//! Scanner for user-defined cleanable paths from config

use super::{calculate_dir_size, get_last_accessed, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};

pub struct CustomPathsScanner {
    allowed_categories: Vec<Category>,
}

impl CustomPathsScanner {
    pub fn new(allowed_categories: Vec<Category>) -> Self {
        Self { allowed_categories }
    }

    fn is_allowed(&self, category: Category) -> bool {
        self.allowed_categories.contains(&category)
    }

    fn expand_path(path: &str, home: &Path, base_path: &Path) -> PathBuf {
        if path == "~" {
            return home.to_path_buf();
        }

        if let Some(stripped) = path.strip_prefix("~/") {
            return home.join(stripped);
        }

        let candidate = PathBuf::from(path);
        if candidate.is_relative() {
            return base_path.join(candidate);
        }

        candidate
    }
}

impl Scanner for CustomPathsScanner {
    fn name(&self) -> &'static str {
        "Custom Paths Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();

        if config.custom_paths.is_empty() {
            return Ok(results);
        }

        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return Ok(results),
        };
        let base_path = config.get_base_path();

        for entry in &config.custom_paths {
            let category = match entry.category.to_category() {
                Some(c) => c,
                None => continue,
            };

            if !self.is_allowed(category) {
                continue;
            }

            let path = Self::expand_path(&entry.path, &home, &base_path);

            if !path.exists() {
                continue;
            }

            if !path.starts_with(&base_path) {
                continue;
            }

            if config.is_excluded(&path) {
                continue;
            }

            let size = if path.is_dir() {
                calculate_dir_size(&path)
            } else {
                path.metadata().map(|m| m.len()).unwrap_or(0)
            };

            let min_size_mb = entry.min_size_mb.unwrap_or(1);
            if size < min_size_mb * 1024 * 1024 {
                continue;
            }

            let last_accessed = get_last_accessed(&path).unwrap_or_else(Utc::now);
            let reason = entry
                .description
                .clone()
                .unwrap_or_else(|| format!("Custom {}", category.display_name()));

            let is_directory = path.is_dir();
            results.push(CleanableFile {
                path,
                size,
                category,
                last_accessed,
                reason,
                is_directory,
            });
        }

        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}
