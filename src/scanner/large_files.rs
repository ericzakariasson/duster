//! Large files scanner

use super::{get_last_accessed, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::path::Path;
use walkdir::WalkDir;

pub struct LargeFilesScanner;

impl LargeFilesScanner {
    pub fn new() -> Self {
        Self
    }

    /// Directories to skip when scanning for large files
    fn should_skip_dir(path: &Path) -> bool {
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy(),
            None => return false,
        };

        // Skip common directories that shouldn't be scanned
        matches!(
            name.as_ref(),
            "node_modules"
                | "target"
                | ".git"
                | ".svn"
                | ".hg"
                | "Library"
                | "Applications"
                | ".Trash"
                | "Volumes"
                | "System"
        )
    }

    /// File extensions that are commonly large but needed
    fn is_common_needed_large_file(path: &Path) -> bool {
        let ext = match path.extension() {
            Some(e) => e.to_string_lossy().to_lowercase(),
            None => return false,
        };

        // Database files in active projects
        if matches!(ext.as_ref(), "db" | "sqlite" | "sqlite3") {
            // Check if it's in an active project directory
            if let Some(parent) = path.parent() {
                if parent.join("package.json").exists() 
                    || parent.join("Cargo.toml").exists()
                    || parent.join(".git").exists() 
                {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for LargeFilesScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for LargeFilesScanner {
    fn name(&self) -> &'static str {
        "Large Files Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();

        let base_path = config.get_base_path();
        let min_size = config.min_large_size_bytes();

        // Walk the directory tree
        for entry in WalkDir::new(&base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip certain directories
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

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = metadata.len();

            // Skip files smaller than threshold
            if size < min_size {
                continue;
            }

            // Skip commonly needed large files
            if Self::is_common_needed_large_file(path) {
                continue;
            }

            let last_accessed = get_last_accessed(path).unwrap_or_else(Utc::now);

            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();

            let file_type = match ext.to_lowercase().as_str() {
                "dmg" => "Disk image",
                "iso" => "ISO image",
                "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => "Archive",
                "pkg" => "Installer package",
                "app" => "Application bundle",
                "mov" | "mp4" | "avi" | "mkv" | "wmv" => "Video file",
                "wav" | "aiff" | "flac" => "Audio file",
                "psd" | "ai" | "sketch" => "Design file",
                "vmdk" | "vdi" | "vhd" => "Virtual disk",
                "log" => "Log file",
                "csv" | "json" | "xml" if size > 100 * 1024 * 1024 => "Data file",
                _ => "Large file",
            };

            results.push(CleanableFile {
                path: path.to_path_buf(),
                size,
                category: Category::LargeFile,
                last_accessed,
                reason: format!("{}: {}", file_type, name),
                is_directory: false,
            });
        }

        // Sort by size descending
        results.sort_by(|a, b| b.size.cmp(&a.size));

        // Limit to top 100 largest files
        results.truncate(100);

        Ok(results)
    }
}
