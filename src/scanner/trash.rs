//! Trash bin scanner

use super::{calculate_dir_size, get_last_accessed, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

pub struct TrashScanner;

impl TrashScanner {
    pub fn new() -> Self {
        Self
    }

    /// Get trash directories based on platform
    fn get_trash_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        if let Some(home) = dirs::home_dir() {
            // macOS
            #[cfg(target_os = "macos")]
            {
                let trash = home.join(".Trash");
                if trash.exists() {
                    dirs.push(trash);
                }
            }

            // Linux
            #[cfg(target_os = "linux")]
            {
                let trash = home.join(".local/share/Trash/files");
                if trash.exists() {
                    dirs.push(trash);
                }
            }
        }

        dirs
    }
}

impl Default for TrashScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for TrashScanner {
    fn name(&self) -> &'static str {
        "Trash Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();
        let trash_dirs = self.get_trash_dirs();

        for trash_dir in trash_dirs {
            let entries = match std::fs::read_dir(&trash_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();

                // Skip if excluded
                if config.is_excluded(&path) {
                    continue;
                }

                let is_dir = path.is_dir();
                let size = if is_dir {
                    calculate_dir_size(&path)
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };

                let last_accessed = get_last_accessed(&path).unwrap_or_else(Utc::now);

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                results.push(CleanableFile {
                    path,
                    size,
                    category: Category::Trash,
                    last_accessed,
                    reason: format!("Trashed item: {}", name),
                    is_directory: is_dir,
                });
            }
        }

        // Sort by size descending
        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}
