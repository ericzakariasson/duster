//! Configuration management with file-based and CLI override support

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::cli::ScanOptions;

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

    /// Base path for scanning (default: home directory)
    #[serde(skip)]
    pub base_path: Option<PathBuf>,
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

    /// Check if a path should be excluded
    pub fn is_excluded(&self, path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();
        self.excluded_paths.iter().any(|pattern| {
            // Simple glob-style matching
            if pattern.contains('*') {
                // Convert glob pattern to simple matching
                let parts: Vec<&str> = pattern.split('*').collect();
                if parts.len() == 2 {
                    let (prefix, suffix) = (parts[0], parts[1]);
                    return path_str.starts_with(prefix) && path_str.ends_with(suffix);
                }
            }
            path_str.contains(pattern)
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
}
