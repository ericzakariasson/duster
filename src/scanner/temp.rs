//! Temporary files scanner

use super::{get_last_accessed, was_modified_within_days, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::env;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct TempScanner;

impl TempScanner {
    pub fn new() -> Self {
        Self
    }

    /// Get temp directories to scan
    fn get_temp_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // Standard temp directories
        dirs.push(PathBuf::from("/tmp"));
        dirs.push(PathBuf::from("/var/tmp"));

        // TMPDIR environment variable (often set on macOS)
        if let Ok(tmpdir) = env::var("TMPDIR") {
            let p = PathBuf::from(&tmpdir);
            if p.exists() && !dirs.contains(&p) {
                dirs.push(p);
            }
        }

        // User-specific temp on macOS
        if let Some(home) = dirs::home_dir() {
            let user_tmp = home.join("Library").join("Caches").join("TemporaryItems");
            if user_tmp.exists() {
                dirs.push(user_tmp);
            }
        }

        dirs
    }
}

impl Default for TempScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for TempScanner {
    fn name(&self) -> &'static str {
        "Temp Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();
        let temp_dirs = self.get_temp_dirs();

        // Only scan files older than 1 day to avoid active temp files
        let min_age_days = 1;

        for temp_dir in temp_dirs {
            if !temp_dir.exists() {
                continue;
            }

            // Walk the temp directory (limit depth to avoid going too deep)
            for entry in WalkDir::new(&temp_dir)
                .max_depth(3)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path().to_path_buf();

                // Skip the root temp directory itself
                if path == temp_dir {
                    continue;
                }

                // Skip if excluded
                if config.is_excluded(&path) {
                    continue;
                }

                // Skip recently modified files (they might be in use)
                if was_modified_within_days(&path, min_age_days) {
                    continue;
                }

                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                // Skip if we don't have read permissions
                if metadata.permissions().readonly() {
                    continue;
                }

                let size = metadata.len();
                let is_dir = metadata.is_dir();

                // Skip small files and directories
                if size < 1024 && !is_dir {
                    continue;
                }

                // Skip directories in deeper walks (we handle top-level only for dirs)
                if is_dir && entry.depth() > 1 {
                    continue;
                }

                let last_accessed = get_last_accessed(&path).unwrap_or_else(Utc::now);

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                results.push(CleanableFile {
                    path,
                    size,
                    category: Category::Temp,
                    last_accessed,
                    reason: format!("Temp file: {}", name),
                    is_directory: is_dir,
                });
            }
        }

        // Sort by size descending
        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}
