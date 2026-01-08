//! Disk usage analysis and reporting

use crate::cli::{ScanCategory, ScanOptions};
use crate::config::Config;
use crate::scanner::{
    build_artifacts::{BuildArtifactsScanner, GlobalCacheScanner},
    cache::{CacheScanner, KnownCacheScanner},
    downloads::DownloadsScanner,
    duplicates::DuplicatesScanner,
    large_files::LargeFilesScanner,
    old_files::OldFilesScanner,
    temp::TempScanner,
    trash::TrashScanner,
    Category, CleanableFile, ScanResult, Scanner,
};
use crate::ui;
use anyhow::Result;
use colored::*;
use rayon::prelude::*;
use std::collections::HashMap;

/// Run all enabled scanners and aggregate results
pub fn run_scan(options: &ScanOptions, config: &Config) -> Result<ScanResult> {
    let mut result = ScanResult::new();
    let mut scanners: Vec<Box<dyn Scanner>> = Vec::new();

    // Build list of scanners based on options
    if options.should_scan(ScanCategory::Cache) {
        scanners.push(Box::new(CacheScanner::new()));
        scanners.push(Box::new(KnownCacheScanner::new()));
    }

    if options.should_scan(ScanCategory::Trash) {
        scanners.push(Box::new(TrashScanner::new()));
    }

    if options.should_scan(ScanCategory::Temp) {
        scanners.push(Box::new(TempScanner::new()));
    }

    if options.should_scan(ScanCategory::Downloads) {
        scanners.push(Box::new(DownloadsScanner::new()));
    }

    if options.should_scan(ScanCategory::Build) {
        scanners.push(Box::new(BuildArtifactsScanner::new()));
        scanners.push(Box::new(GlobalCacheScanner::new()));
    }

    if options.should_scan(ScanCategory::Large) {
        scanners.push(Box::new(LargeFilesScanner::new()));
    }

    if options.should_scan(ScanCategory::Duplicates) {
        scanners.push(Box::new(DuplicatesScanner::new()));
    }

    if options.should_scan(ScanCategory::Old) {
        scanners.push(Box::new(OldFilesScanner::new()));
    }

    // Show progress
    let spinner = ui::create_spinner("Scanning for cleanable files...");

    // Run scanners in parallel
    let scan_results: Vec<(String, Result<Vec<CleanableFile>>)> = scanners
        .par_iter()
        .map(|scanner| {
            let name = scanner.name().to_string();
            let files = scanner.scan(config);
            (name, files)
        })
        .collect();

    // Aggregate results
    for (name, files_result) in scan_results {
        match files_result {
            Ok(files) => {
                result.add_files(files);
            }
            Err(e) => {
                result.add_error(format!("{}: {}", name, e));
            }
        }
    }

    spinner.finish_and_clear();

    // Deduplicate results (same path shouldn't appear twice)
    let mut seen_paths = std::collections::HashSet::new();
    result.files.retain(|f| seen_paths.insert(f.path.clone()));

    Ok(result)
}

/// Print a summary report of scan results
pub fn print_report(result: &ScanResult) {
    let by_category = result.by_category();

    // Calculate category totals
    let mut category_stats: Vec<(Category, usize, u64)> = by_category
        .iter()
        .map(|(cat, files)| {
            let count = files.len();
            let size: u64 = files.iter().map(|f| f.size).sum();
            (*cat, count, size)
        })
        .collect();

    // Sort by size descending
    category_stats.sort_by(|a, b| b.2.cmp(&a.2));

    // Print header
    ui::print_header("Scan Results");

    // Print category breakdown
    println!(
        "{:<20} {:>10} {:>12}",
        "Category".bold(),
        "Files".bold(),
        "Size".bold()
    );
    ui::print_table_separator(44);

    for (category, count, size) in &category_stats {
        println!(
            "{:<20} {:>10} {:>12}",
            category.display_name(),
            ui::format_number(*count as u64),
            ui::format_size(*size)
        );
    }

    ui::print_table_separator(44);

    // Print total
    println!(
        "{:<20} {:>10} {:>12}",
        "Total".bold(),
        ui::format_number(result.total_count() as u64).bold(),
        ui::format_size(result.total_size()).yellow().bold()
    );

    // Print any errors
    if !result.errors.is_empty() {
        println!();
        ui::print_warning(&format!("{} scanner(s) encountered errors:", result.errors.len()));
        for error in &result.errors {
            println!("  {}", error.dimmed());
        }
    }
}

/// Print detailed breakdown of scan results
pub fn print_detailed_report(result: &ScanResult) {
    let by_category = result.by_category();

    // Sort categories by total size
    let mut categories: Vec<_> = by_category.iter().collect();
    categories.sort_by(|a, b| {
        let size_a: u64 = a.1.iter().map(|f| f.size).sum();
        let size_b: u64 = b.1.iter().map(|f| f.size).sum();
        size_b.cmp(&size_a)
    });

    ui::print_header("Detailed Analysis");

    for (category, files) in categories {
        if files.is_empty() {
            continue;
        }

        let total_size: u64 = files.iter().map(|f| f.size).sum();
        ui::print_category_header(category.display_name(), total_size, files.len());

        // Show top 5 largest items
        let mut sorted_files: Vec<_> = files.iter().collect();
        sorted_files.sort_by(|a, b| b.size.cmp(&a.size));

        for file in sorted_files.iter().take(5) {
            ui::print_file_entry(&file.path, file.size, 1);
        }

        if files.len() > 5 {
            println!(
                "  {} {} more items...",
                "...and".dimmed(),
                files.len() - 5
            );
        }
    }

    ui::print_summary(result.total_count(), result.total_size());
}

/// Print JSON output of scan results
pub fn print_json_report(result: &ScanResult) -> Result<()> {
    let output = serde_json::json!({
        "summary": {
            "total_files": result.total_count(),
            "total_size": result.total_size(),
            "total_size_formatted": ui::format_size(result.total_size()),
        },
        "by_category": result.by_category().iter().map(|(cat, files)| {
            let size: u64 = files.iter().map(|f| f.size).sum();
            serde_json::json!({
                "category": cat.display_name(),
                "count": files.len(),
                "size": size,
                "size_formatted": ui::format_size(size),
            })
        }).collect::<Vec<_>>(),
        "files": result.files.iter().map(|f| {
            serde_json::json!({
                "path": f.path.display().to_string(),
                "size": f.size,
                "size_formatted": ui::format_size(f.size),
                "category": f.category.display_name(),
                "reason": f.reason,
                "is_directory": f.is_directory,
            })
        }).collect::<Vec<_>>(),
        "errors": result.errors,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

/// Group files by category for interactive selection
pub fn group_by_category(files: &[CleanableFile]) -> HashMap<Category, Vec<&CleanableFile>> {
    let mut groups: HashMap<Category, Vec<&CleanableFile>> = HashMap::new();

    for file in files {
        groups.entry(file.category).or_default().push(file);
    }

    groups
}
