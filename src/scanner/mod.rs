//! Scanner infrastructure and common types

pub mod build_artifacts;
pub mod cache;
pub mod downloads;
pub mod duplicates;
pub mod large_files;
pub mod old_files;
pub mod temp;
pub mod trash;

use crate::config::Config;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a file that can be cleaned up
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanableFile {
    /// Path to the file or directory
    pub path: PathBuf,
    /// Size in bytes
    pub size: u64,
    /// Category of cleanable file
    pub category: Category,
    /// Last access time
    pub last_accessed: DateTime<Utc>,
    /// Human-readable reason why this file is cleanable
    pub reason: String,
    /// Whether this is a directory (for proper deletion)
    pub is_directory: bool,
}

/// Categories of cleanable files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Cache,
    Trash,
    Temp,
    Downloads,
    BuildArtifact,
    LargeFile,
    Duplicate,
    OldFile,
}

impl Category {
    /// Get the display name for this category
    pub fn display_name(&self) -> &'static str {
        match self {
            Category::Cache => "System Cache",
            Category::Trash => "Trash",
            Category::Temp => "Temp Files",
            Category::Downloads => "Old Downloads",
            Category::BuildArtifact => "Build Artifacts",
            Category::LargeFile => "Large Files",
            Category::Duplicate => "Duplicates",
            Category::OldFile => "Old Files",
        }
    }

    /// Get a short description of this category
    pub fn description(&self) -> &'static str {
        match self {
            Category::Cache => "Cached data from applications and system",
            Category::Trash => "Files in the trash bin",
            Category::Temp => "Temporary files from /tmp and similar",
            Category::Downloads => "Old files in Downloads folder",
            Category::BuildArtifact => "Build outputs and dependencies (node_modules, target, etc.)",
            Category::LargeFile => "Large files that may not be needed",
            Category::Duplicate => "Duplicate files wasting space",
            Category::OldFile => "Files not accessed for a long time",
        }
    }
}

/// Trait for file scanners
pub trait Scanner: Send + Sync {
    /// Get the name of this scanner
    fn name(&self) -> &'static str;

    /// Scan for cleanable files
    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>>;
}

/// Calculate the total size of a directory recursively
pub fn calculate_dir_size(path: &std::path::Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Get the last modified time of a file or directory
pub fn get_last_modified(path: &std::path::Path) -> Option<DateTime<Utc>> {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| DateTime::<Utc>::from(t))
}

/// Get the last accessed time of a file
pub fn get_last_accessed(path: &std::path::Path) -> Option<DateTime<Utc>> {
    path.metadata()
        .ok()
        .and_then(|m| m.accessed().ok())
        .map(|t| DateTime::<Utc>::from(t))
}

/// Check if a path was accessed within the given number of days
pub fn was_accessed_within_days(path: &std::path::Path, days: u32) -> bool {
    if let Some(accessed) = get_last_accessed(path) {
        let threshold = Utc::now() - chrono::Duration::days(days as i64);
        return accessed > threshold;
    }
    // If we can't determine access time, assume it was recently accessed (safe default)
    true
}

/// Check if a path was modified within the given number of days
pub fn was_modified_within_days(path: &std::path::Path, days: u32) -> bool {
    if let Some(modified) = get_last_modified(path) {
        let threshold = Utc::now() - chrono::Duration::days(days as i64);
        return modified > threshold;
    }
    // If we can't determine modified time, assume it was recently modified (safe default)
    true
}

/// Aggregate scan results from multiple scanners
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanResult {
    pub files: Vec<CleanableFile>,
    pub errors: Vec<String>,
}

impl ScanResult {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn add_files(&mut self, files: Vec<CleanableFile>) {
        self.files.extend(files);
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    pub fn total_count(&self) -> usize {
        self.files.len()
    }

    /// Group files by category
    pub fn by_category(&self) -> std::collections::HashMap<Category, Vec<&CleanableFile>> {
        let mut map = std::collections::HashMap::new();
        for file in &self.files {
            map.entry(file.category).or_insert_with(Vec::new).push(file);
        }
        map
    }
}

