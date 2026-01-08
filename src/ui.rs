//! Terminal UI helpers for formatting, prompts, and progress indicators

use colored::*;
use dialoguer::{Confirm, MultiSelect};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::time::Duration;

/// Format bytes as human-readable size
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format path, replacing home directory with ~
pub fn format_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(relative) = path.strip_prefix(&home) {
            return format!("~/{}", relative.display());
        }
    }
    path.display().to_string()
}

/// Print a table row with formatting
pub fn print_table_row(columns: &[(&str, usize)]) {
    let formatted: Vec<String> = columns
        .iter()
        .map(|(text, width)| format!("{:<width$}", text, width = width))
        .collect();
    println!("{}", formatted.join("  "));
}

/// Print a table separator line
pub fn print_table_separator(width: usize) {
    println!("{}", "─".repeat(width));
}

/// Print a header for scan results
pub fn print_header(title: &str) {
    println!();
    println!("{}", title.bold().cyan());
    println!();
}

/// Print a success message
pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

/// Print a warning message
pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".yellow().bold(), message);
}

/// Print an error message
pub fn print_error(message: &str) {
    println!("{} {}", "✗".red().bold(), message);
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}

/// Ask for yes/no confirmation
pub fn confirm(message: &str) -> bool {
    Confirm::new()
        .with_prompt(message)
        .default(false)
        .interact()
        .unwrap_or(false)
}

/// Multi-select from a list of items
pub fn multi_select(prompt: &str, items: &[String]) -> Vec<usize> {
    if items.is_empty() {
        return Vec::new();
    }

    MultiSelect::new()
        .with_prompt(prompt)
        .items(items)
        .interact()
        .unwrap_or_default()
}

/// Create a spinner for indeterminate progress
pub fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// Create a progress bar for determinate progress
pub fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{wide_bar:.cyan/blue} {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("█▓▒░"),
    );
    pb.set_message(message.to_string());
    pb
}

/// Format a number with thousand separators
pub fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Print a category header with size
pub fn print_category_header(name: &str, size: u64, count: usize) {
    println!(
        "\n{} {} ({}):",
        name.bold(),
        format!("({} files)", format_number(count as u64)).dimmed(),
        format_size(size).yellow()
    );
}

/// Print a file entry with optional indentation
pub fn print_file_entry(path: &Path, size: u64, indent: usize) {
    let indent_str = "  ".repeat(indent);
    println!(
        "{}{}  {}",
        indent_str,
        format_path(path),
        format_size(size).dimmed()
    );
}

/// Print summary statistics
pub fn print_summary(total_files: usize, total_size: u64) {
    println!();
    print_table_separator(50);
    println!(
        "{}: {} across {} files",
        "Total".bold(),
        format_size(total_size).yellow().bold(),
        format_number(total_files as u64)
    );
}

/// Print deletion warning
pub fn print_deletion_warning() {
    println!();
    println!(
        "{}  {}",
        "⚠️".yellow(),
        "This action is permanent and cannot be undone.".red().bold()
    );
}

/// Format a duration in human-readable form
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1000000), "1,000,000");
        assert_eq!(format_number(42), "42");
    }
}
