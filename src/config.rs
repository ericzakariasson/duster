//! Configuration management with file-based and CLI override support

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::cli::ScanOptions;
use crate::scanner::Category;

/// Application configuration with sensible defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Files older than this are considered "old" (default: 30 days)
    #[serde(default = "default_min_age_days")]
    pub min_age_days: u32,

    /// Files larger than this are considered "large" (default: 100 MB)
    #[serde(default = "default_min_large_size_mb")]
    pub min_large_size_mb: u64,

    /// Projects accessed within this period are considered "recent" (default: 14 days)
    #[serde(default = "default_project_recent_days")]
    pub project_recent_days: u32,

    /// Downloads older than this are candidates for cleanup (default: 30 days)
    #[serde(default = "default_download_age_days")]
    pub download_age_days: u32,

    /// Paths to always exclude from scanning
    #[serde(default)]
    pub excluded_paths: Vec<String>,

    /// Additional cache paths to scan beyond system defaults
    #[serde(default)]
    pub cache_paths: Vec<String>,

    /// Custom cleanable paths with categories and optional metadata
    #[serde(default)]
    pub custom_paths: Vec<CustomCleanPath>,

    /// File extensions to ignore during scanning
    #[serde(default)]
    pub ignored_extensions: Vec<String>,

    /// When set, only include files with these extensions
    #[serde(default)]
    pub only_extensions: Vec<String>,

    /// Base path for scanning (default: home directory)
    #[serde(skip)]
    pub base_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCleanPath {
    /// Path to clean (absolute or ~/ relative)
    pub path: String,

    /// Category for this path (cache, build, etc.)
    #[serde(default)]
    pub category: CustomPathCategory,

    /// Optional reason shown in reports
    #[serde(default)]
    pub description: Option<String>,

    /// Minimum size (MB) before showing this entry (default: 1MB)
    #[serde(default)]
    pub min_size_mb: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustomPathCategory {
    Cache,
    Trash,
    Temp,
    Downloads,
    Build,
    Large,
    Duplicates,
    Old,
    #[serde(other)]
    Unknown,
}

impl Default for CustomPathCategory {
    fn default() -> Self {
        Self::Cache
    }
}

impl CustomPathCategory {
    pub fn to_category(self) -> Option<Category> {
        match self {
            CustomPathCategory::Cache => Some(Category::Cache),
            CustomPathCategory::Trash => Some(Category::Trash),
            CustomPathCategory::Temp => Some(Category::Temp),
            CustomPathCategory::Downloads => Some(Category::Downloads),
            CustomPathCategory::Build => Some(Category::BuildArtifact),
            CustomPathCategory::Large => Some(Category::LargeFile),
            CustomPathCategory::Duplicates => Some(Category::Duplicate),
            CustomPathCategory::Old => Some(Category::OldFile),
            CustomPathCategory::Unknown => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CustomPathCategory::Cache => "cache",
            CustomPathCategory::Trash => "trash",
            CustomPathCategory::Temp => "temp",
            CustomPathCategory::Downloads => "downloads",
            CustomPathCategory::Build => "build",
            CustomPathCategory::Large => "large",
            CustomPathCategory::Duplicates => "duplicates",
            CustomPathCategory::Old => "old",
            CustomPathCategory::Unknown => "unknown",
        }
    }
}

fn default_min_age_days() -> u32 {
    30
}

fn default_min_large_size_mb() -> u64 {
    100
}

fn default_project_recent_days() -> u32 {
    14
}

fn default_download_age_days() -> u32 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_age_days: default_min_age_days(),
            min_large_size_mb: default_min_large_size_mb(),
            project_recent_days: default_project_recent_days(),
            download_age_days: default_download_age_days(),
            excluded_paths: Vec::new(),
            cache_paths: Vec::new(),
            custom_paths: Vec::new(),
            ignored_extensions: Vec::new(),
            only_extensions: Vec::new(),
            base_path: None,
        }
    }
}

impl Config {
    /// Get the config file path
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("duster").join("config.toml"))
    }

    /// Load configuration from file, falling back to defaults
    pub fn load() -> Result<Self> {
        let config_path = match Self::config_path() {
            Some(p) => p,
            None => return Ok(Self::default()),
        };

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path().context("Could not determine config directory")?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }

    /// Apply CLI options to override config values
    pub fn apply_cli_options(&mut self, options: &ScanOptions) {
        if let Some(min_age) = options.min_age {
            self.min_age_days = min_age;
        }

        if let Some(ref min_size) = options.min_size {
            if let Some(size_mb) = parse_size_mb(min_size) {
                self.min_large_size_mb = size_mb;
            }
        }

        if let Some(project_age) = options.project_age {
            self.project_recent_days = project_age;
        }

        if let Some(ref path) = options.path {
            self.base_path = Some(path.clone());
        }

        // Add CLI exclusions to existing ones
        for exclude in &options.exclude {
            if !self.excluded_paths.contains(exclude) {
                self.excluded_paths.push(exclude.clone());
            }
        }

        // Add CLI ignored extensions
        for ext in &options.ignore_ext {
            let ext_lower = ext.to_lowercase();
            if !self.ignored_extensions.contains(&ext_lower) {
                self.ignored_extensions.push(ext_lower);
            }
        }

        // Add CLI only-ext filters
        for ext in &options.only_ext {
            let ext_lower = ext.to_lowercase();
            if !self.only_extensions.contains(&ext_lower) {
                self.only_extensions.push(ext_lower);
            }
        }
    }

    /// Get the base path for scanning
    pub fn get_base_path(&self) -> PathBuf {
        self.base_path
            .clone()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Get minimum large file size in bytes
    pub fn min_large_size_bytes(&self) -> u64 {
        self.min_large_size_mb * 1024 * 1024
    }

    /// Check if a file should be skipped based on extension filters.
    /// Handles both --ignore-ext (exclusion) and --only-ext (inclusion) logic.
    pub fn should_skip_extension(&self, path: &std::path::Path) -> bool {
        if self.ignored_extensions.is_empty() && self.only_extensions.is_empty() {
            return false;
        }

        let ext = match path.extension() {
            Some(e) => e.to_string_lossy().to_lowercase(),
            None => return !self.only_extensions.is_empty(),
        };

        if self.ignored_extensions.iter().any(|ignored| ignored == &ext) {
            return true;
        }

        if !self.only_extensions.is_empty() {
            return self.only_extensions.iter().any(|only| only == &ext);
        }

        false
    }

    /// Check if a path should be excluded
    pub fn is_excluded(&self, path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();
        let home_dir = dirs::home_dir();
        self.excluded_paths.iter().any(|pattern| {
            // Expand ~ prefix to home directory for matching
            let expanded = if let Some(stripped) = pattern.strip_prefix("~/") {
                home_dir
                    .as_ref()
                    .map(|h| h.join(stripped).to_string_lossy().into_owned())
            } else if pattern == "~" {
                home_dir
                    .as_ref()
                    .map(|h| h.to_string_lossy().into_owned())
            } else {
                None
            };
            let effective_pattern = expanded.as_deref().unwrap_or(pattern);

            // Simple glob-style matching
            if effective_pattern.contains('*') {
                // Convert glob pattern to simple matching
                let parts: Vec<&str> = effective_pattern.split('*').collect();
                if parts.len() == 2 {
                    let (prefix, suffix) = (parts[0], parts[1]);
                    return path_str.starts_with(prefix) && path_str.ends_with(suffix);
                }
            }
            path_str.contains(effective_pattern)
        })
    }
}

/// Parse a human-readable size string to megabytes
fn parse_size_mb(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();

    // Try to parse with unit suffix
    if let Some(num_str) = s.strip_suffix("GB") {
        return num_str.trim().parse::<u64>().ok().map(|n| n * 1024);
    }
    if let Some(num_str) = s.strip_suffix("MB") {
        return num_str.trim().parse::<u64>().ok();
    }
    if let Some(num_str) = s.strip_suffix("KB") {
        return num_str.trim().parse::<u64>().ok().map(|n| n / 1024);
    }
    if let Some(num_str) = s.strip_suffix('G') {
        return num_str.trim().parse::<u64>().ok().map(|n| n * 1024);
    }
    if let Some(num_str) = s.strip_suffix('M') {
        return num_str.trim().parse::<u64>().ok();
    }
    if let Some(num_str) = s.strip_suffix('K') {
        return num_str.trim().parse::<u64>().ok().map(|n| n / 1024);
    }

    // Try to parse as plain number (assume MB)
    s.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_mb() {
        assert_eq!(parse_size_mb("100MB"), Some(100));
        assert_eq!(parse_size_mb("1GB"), Some(1024));
        assert_eq!(parse_size_mb("100"), Some(100));
        assert_eq!(parse_size_mb("1G"), Some(1024));
        assert_eq!(parse_size_mb("500M"), Some(500));
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.min_age_days, 30);
        assert_eq!(config.min_large_size_mb, 100);
        assert_eq!(config.project_recent_days, 14);
    }

    #[test]
    fn test_is_excluded_plain_pattern() {
        let mut config = Config::default();
        config.excluded_paths = vec![".local/share/cursor-agent".to_string()];
        let path = PathBuf::from("/Users/test/.local/share/cursor-agent/versions/node");
        assert!(config.is_excluded(&path));
    }

    #[test]
    fn test_is_excluded_tilde_expansion() {
        let mut config = Config::default();
        config.excluded_paths = vec!["~/.local/share/cursor-agent".to_string()];
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".local/share/cursor-agent/versions/node");
            assert!(config.is_excluded(&path));
        }
    }

    #[test]
    fn test_is_excluded_glob() {
        let mut config = Config::default();
        config.excluded_paths = vec!["/tmp/test*artifact".to_string()];
        let path = PathBuf::from("/tmp/test-build-artifact");
        assert!(config.is_excluded(&path));
    }
}
