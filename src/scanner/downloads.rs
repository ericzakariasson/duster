//! Old downloads scanner

use super::{get_last_accessed, was_accessed_within_days, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct DownloadsScanner;

impl DownloadsScanner {
    pub fn new() -> Self {
        Self
    }

    /// Get the downloads directory
    fn get_downloads_dir(&self) -> Option<PathBuf> {
        dirs::download_dir()
    }
}

impl Default for DownloadsScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for DownloadsScanner {
    fn name(&self) -> &'static str {
        "Downloads Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();

        let downloads_dir = match self.get_downloads_dir() {
            Some(d) if d.exists() => d,
            _ => return Ok(results),
        };

        let age_threshold = config.download_age_days;

        // Walk the downloads directory (shallow - only top level)
        for entry in WalkDir::new(&downloads_dir)
            .max_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path().to_path_buf();

            // Skip the downloads directory itself
            if path == downloads_dir {
                continue;
            }

            // Skip if excluded
            if config.is_excluded(&path) {
                continue;
            }

            // Skip hidden files
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }

            // Skip recently accessed files
            if was_accessed_within_days(&path, age_threshold) {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = if metadata.is_dir() {
                super::calculate_dir_size(&path)
            } else {
                metadata.len()
            };

            let is_dir = metadata.is_dir();
            let last_accessed = get_last_accessed(&path).unwrap_or_else(Utc::now);

            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            // Calculate age in days
            let age_days = (Utc::now() - last_accessed).num_days();

            results.push(CleanableFile {
                path,
                size,
                category: Category::Downloads,
                last_accessed,
                reason: format!("Download not accessed in {} days: {}", age_days, name),
                is_directory: is_dir,
            });
        }

        // Sort by size descending (prioritize large files)
        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}
