//! Deletion logic with confirmation and progress

use crate::scanner::{Category, CleanableFile};
use crate::ui;
use anyhow::{Context, Result};
use colored::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Result of a cleanup operation
#[derive(Debug)]
pub struct CleanupResult {
    /// Number of files/directories successfully deleted
    pub deleted_count: usize,
    /// Total bytes freed
    pub freed_bytes: u64,
    /// Errors encountered during deletion
    pub errors: Vec<String>,
}

impl CleanupResult {
    pub fn new() -> Self {
        Self {
            deleted_count: 0,
            freed_bytes: 0,
            errors: Vec::new(),
        }
    }
}

impl Default for CleanupResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Preview what will be deleted
pub fn preview_deletion(files: &[CleanableFile]) {
    let mut by_category: HashMap<Category, Vec<&CleanableFile>> = HashMap::new();

    for file in files {
        by_category.entry(file.category).or_default().push(file);
    }

    // Sort categories by total size
    let mut categories: Vec<_> = by_category.iter().collect();
    categories.sort_by(|a, b| {
        let size_a: u64 = a.1.iter().map(|f| f.size).sum();
        let size_b: u64 = b.1.iter().map(|f| f.size).sum();
        size_b.cmp(&size_a)
    });

    println!();
    println!("{}", "Files to delete:".bold());

    for (category, cat_files) in categories {
        let total_size: u64 = cat_files.iter().map(|f| f.size).sum();

        println!();
        println!(
            "{} ({}):",
            category.display_name().bold(),
            ui::format_size(total_size).yellow()
        );

        // Show top items
        let mut sorted: Vec<_> = cat_files.iter().collect();
        sorted.sort_by(|a, b| b.size.cmp(&a.size));

        for file in sorted.iter().take(3) {
            println!(
                "  {} ({})",
                ui::format_path(&file.path),
                ui::format_size(file.size).dimmed()
            );
        }

        if cat_files.len() > 3 {
            println!("  {} and {} more", "...".dimmed(), cat_files.len() - 3);
        }
    }

    let total_size: u64 = files.iter().map(|f| f.size).sum();
    ui::print_summary(files.len(), total_size);
    ui::print_deletion_warning();
}

/// Interactively select which categories to clean
pub fn select_categories(files: &[CleanableFile]) -> Vec<Category> {
    let mut by_category: HashMap<Category, Vec<&CleanableFile>> = HashMap::new();

    for file in files {
        by_category.entry(file.category).or_default().push(file);
    }

    // Build selection items
    let mut items: Vec<(Category, String)> = by_category
        .iter()
        .map(|(cat, cat_files)| {
            let total_size: u64 = cat_files.iter().map(|f| f.size).sum();
            let label = format!(
                "{} ({} files, {})",
                cat.display_name(),
                cat_files.len(),
                ui::format_size(total_size)
            );
            (*cat, label)
        })
        .collect();

    // Sort by size
    items.sort_by(|a, b| {
        let size_a: u64 = by_category[&a.0].iter().map(|f| f.size).sum();
        let size_b: u64 = by_category[&b.0].iter().map(|f| f.size).sum();
        size_b.cmp(&size_a)
    });

    let labels: Vec<String> = items.iter().map(|(_, label)| label.clone()).collect();
    let selected = ui::multi_select("Select categories to clean:", &labels);

    selected.into_iter().map(|i| items[i].0).collect()
}

/// Delete files in the specified categories
pub fn delete_files(
    files: &[CleanableFile],
    categories: Option<&[Category]>,
) -> Result<CleanupResult> {
    let mut result = CleanupResult::new();

    // Filter files by category if specified
    let files_to_delete: Vec<&CleanableFile> = if let Some(cats) = categories {
        files.iter().filter(|f| cats.contains(&f.category)).collect()
    } else {
        files.iter().collect()
    };

    if files_to_delete.is_empty() {
        return Ok(result);
    }

    let progress = ui::create_progress_bar(files_to_delete.len() as u64, "Deleting files...");

    for file in files_to_delete {
        let delete_result = if file.is_directory {
            delete_directory(&file.path)
        } else {
            delete_file(&file.path)
        };

        match delete_result {
            Ok(_) => {
                result.deleted_count += 1;
                result.freed_bytes += file.size;
            }
            Err(e) => {
                result.errors.push(format!("{}: {}", file.path.display(), e));
            }
        }

        progress.inc(1);
    }

    progress.finish_and_clear();

    Ok(result)
}

/// Delete a single file
fn delete_file(path: &Path) -> Result<()> {
    // Safety check: don't delete outside home directory
    if !is_safe_to_delete(path) {
        anyhow::bail!("Refusing to delete path outside home directory");
    }

    fs::remove_file(path).with_context(|| format!("Failed to delete file: {}", path.display()))
}

/// Delete a directory recursively
fn delete_directory(path: &Path) -> Result<()> {
    // Safety check: don't delete outside home directory
    if !is_safe_to_delete(path) {
        anyhow::bail!("Refusing to delete path outside home directory");
    }

    fs::remove_dir_all(path)
        .with_context(|| format!("Failed to delete directory: {}", path.display()))
}

/// Check if a path is safe to delete
fn is_safe_to_delete(path: &Path) -> bool {
    // Must be within home directory
    if let Some(home) = dirs::home_dir() {
        if path.starts_with(&home) {
            // Don't delete direct children of home
            if path.parent() == Some(&home) {
                // Only allow specific directories
                let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                return matches!(
                    name.as_deref(),
                    Some(".Trash")
                        | Some(".cache")
                        | Some("Library/Caches")
                );
            }
            return true;
        }
    }

    // Allow temp directories
    if path.starts_with("/tmp") || path.starts_with("/var/tmp") || path.starts_with("/var/folders") {
        return true;
    }

    false
}

/// Print cleanup results
pub fn print_cleanup_result(result: &CleanupResult) {
    println!();

    if result.deleted_count > 0 {
        ui::print_success(&format!(
            "Cleaned {} items, freed {}",
            ui::format_number(result.deleted_count as u64),
            ui::format_size(result.freed_bytes)
        ));
    } else {
        ui::print_info("No files were deleted.");
    }

    if !result.errors.is_empty() {
        println!();
        ui::print_warning(&format!(
            "{} item(s) could not be deleted:",
            result.errors.len()
        ));
        for error in result.errors.iter().take(5) {
            println!("  {}", error.dimmed());
        }
        if result.errors.len() > 5 {
            println!("  ... and {} more errors", result.errors.len() - 5);
        }
    }
}
