//! System and application cache scanner

use super::{calculate_dir_size, get_last_accessed, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

pub struct CacheScanner;

impl CacheScanner {
    pub fn new() -> Self {
        Self
    }

    /// Get cache directories to scan based on the platform
    fn get_cache_dirs(&self, config: &Config) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        if let Some(home) = dirs::home_dir() {
            // macOS
            #[cfg(target_os = "macos")]
            {
                let library_caches = home.join("Library").join("Caches");
                if library_caches.exists() {
                    dirs.push(library_caches);
                }
            }

            // Linux / fallback
            let cache_dir = home.join(".cache");
            if cache_dir.exists() {
                dirs.push(cache_dir);
            }
        }

        // Add any custom cache paths from config
        for path in &config.cache_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                dirs.push(p);
            }
        }

        dirs
    }
}

impl Default for CacheScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for CacheScanner {
    fn name(&self) -> &'static str {
        "Cache Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();
        let cache_dirs = self.get_cache_dirs(config);

        for cache_dir in cache_dirs {
            // Scan top-level directories in cache
            let entries = match std::fs::read_dir(&cache_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();

                // Skip if excluded
                if config.is_excluded(&path) {
                    continue;
                }

                // Calculate size
                let size = if path.is_dir() {
                    calculate_dir_size(&path)
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };

                // Skip very small cache entries (less than 1MB)
                if size < 1024 * 1024 {
                    continue;
                }

                let last_accessed = get_last_accessed(&path).unwrap_or_else(Utc::now);

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                results.push(CleanableFile {
                    path: path.clone(),
                    size,
                    category: Category::Cache,
                    last_accessed,
                    reason: format!("Cache directory: {}", name),
                    is_directory: path.is_dir(),
                });
            }
        }

        // Sort by size descending
        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}

/// Scan for specific application caches that are known to be safe to delete
pub struct KnownCacheScanner;

impl KnownCacheScanner {
    pub fn new() -> Self {
        Self
    }

    /// List of known cache directories relative to home that are safe to clean
    fn known_caches() -> Vec<(&'static str, &'static str)> {
        vec![
            // Package managers
            ("Library/Caches/Homebrew", "Homebrew downloads cache"),
            (".npm/_cacache", "npm cache"),
            (".yarn/cache", "Yarn cache"),
            (".pnpm-store", "pnpm cache"),
            (".cargo/registry/cache", "Cargo registry cache"),
            (".gradle/caches", "Gradle cache"),
            (".m2/repository", "Maven cache"),
            (".nuget/packages", "NuGet cache"),
            (".cache/pip", "pip cache"),
            (".cache/go-build", "Go build cache"),
            // IDEs and editors
            ("Library/Caches/com.apple.dt.Xcode", "Xcode cache"),
            ("Library/Caches/JetBrains", "JetBrains IDEs cache"),
            ("Library/Caches/com.microsoft.VSCode", "VS Code cache"),
            (".vscode-server", "VS Code Server"),
            // Browsers
            (
                "Library/Caches/com.google.Chrome",
                "Chrome browser cache",
            ),
            (
                "Library/Caches/com.brave.Browser",
                "Brave browser cache",
            ),
            (
                "Library/Caches/org.mozilla.firefox",
                "Firefox browser cache",
            ),
            ("Library/Caches/com.apple.Safari", "Safari browser cache"),
            // Apps
            (
                "Library/Caches/com.spotify.client",
                "Spotify cache",
            ),
            (
                "Library/Caches/com.docker.docker",
                "Docker cache",
            ),
            ("Library/Caches/Slack", "Slack cache"),
        ]
    }
}

impl Default for KnownCacheScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for KnownCacheScanner {
    fn name(&self) -> &'static str {
        "Known Cache Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();

        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return Ok(results),
        };

        for (rel_path, description) in Self::known_caches() {
            let path = home.join(rel_path);

            if !path.exists() {
                continue;
            }

            if config.is_excluded(&path) {
                continue;
            }

            let size = calculate_dir_size(&path);
            let last_accessed = get_last_accessed(&path).unwrap_or_else(Utc::now);

            // Only include if it's at least 10MB
            if size >= 10 * 1024 * 1024 {
                results.push(CleanableFile {
                    path,
                    size,
                    category: Category::Cache,
                    last_accessed,
                    reason: description.to_string(),
                    is_directory: true,
                });
            }
        }

        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}
