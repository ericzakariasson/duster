//! Old files scanner for files not accessed in a long time

use super::{get_last_accessed, was_accessed_within_days, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::path::Path;
use walkdir::WalkDir;

pub struct OldFilesScanner;

impl OldFilesScanner {
    pub fn new() -> Self {
        Self
    }

    /// Directories to specifically scan for old files
    fn user_data_dirs() -> Vec<&'static str> {
        vec!["Documents", "Desktop", "Pictures", "Movies", "Music"]
    }

    /// Directories to skip
    fn should_skip_dir(path: &Path) -> bool {
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy(),
            None => return false,
        };

        // Skip dotfiles/directories
        if name.starts_with('.') {
            return true;
        }

        // Skip common non-user directories
        matches!(
            name.as_ref(),
            "node_modules"
                | "target"
                | "Library"
                | "Applications"
                | ".Trash"
                | "Volumes"
                | "System"
                | "bin"
                | "lib"
                | "include"
                | "share"
        )
    }

    /// File extensions that are typically system/config files
    fn is_system_file(path: &Path) -> bool {
        let ext = match path.extension() {
            Some(e) => e.to_string_lossy().to_lowercase(),
            None => return false,
        };

        matches!(
            ext.as_ref(),
            "plist" | "dylib" | "so" | "dll" | "sys" | "kext" | "bundle"
        )
    }
}

impl Default for OldFilesScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for OldFilesScanner {
    fn name(&self) -> &'static str {
        "Old Files Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();

        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return Ok(results),
        };

        let min_age_days = config.min_age_days;

        // Scan user data directories
        for dir_name in Self::user_data_dirs() {
            let dir_path = home.join(dir_name);

            if !dir_path.exists() {
                continue;
            }

            for entry in WalkDir::new(&dir_path)
                .follow_links(false)
                .max_depth(5) // Don't go too deep
                .into_iter()
                .filter_entry(|e| {
                    if e.file_type().is_dir() {
                        return !Self::should_skip_dir(e.path());
                    }
                    true
                })
                .filter_map(|e| e.ok())
            {
                // Only look at files
                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path();

                // Skip if excluded
                if config.is_excluded(path) {
                    continue;
                }

                // Skip hidden files
                if let Some(name) = path.file_name() {
                    if name.to_string_lossy().starts_with('.') {
                        continue;
                    }
                }

                // Skip system files
                if Self::is_system_file(path) {
                    continue;
                }

                // Skip recently accessed files
                if was_accessed_within_days(path, min_age_days) {
                    continue;
                }

                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let size = metadata.len();

                // Skip very small files (less than 10KB)
                if size < 10 * 1024 {
                    continue;
                }

                let last_accessed = get_last_accessed(path).unwrap_or_else(Utc::now);

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let age_days = (Utc::now() - last_accessed).num_days();

                results.push(CleanableFile {
                    path: path.to_path_buf(),
                    size,
                    category: Category::OldFile,
                    last_accessed,
                    reason: format!("Not accessed in {} days: {}", age_days, name),
                    is_directory: false,
                });
            }
        }

        // Sort by last accessed (oldest first) then by size
        results.sort_by(|a, b| {
            a.last_accessed
                .cmp(&b.last_accessed)
                .then(b.size.cmp(&a.size))
        });

        // Limit results to avoid overwhelming output
        results.truncate(200);

        Ok(results)
    }
}
