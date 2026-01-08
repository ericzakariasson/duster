//! Duplicate files scanner using blake3 hashing

use super::{get_last_accessed, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct DuplicatesScanner;

impl DuplicatesScanner {
    pub fn new() -> Self {
        Self
    }

    /// Directories to skip when scanning for duplicates
    fn should_skip_dir(path: &Path) -> bool {
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy(),
            None => return false,
        };

        matches!(
            name.as_ref(),
            "node_modules"
                | "target"
                | ".git"
                | ".svn"
                | ".hg"
                | "Library"
                | ".Trash"
                | ".cache"
                | "Caches"
        )
    }

    /// Compute blake3 hash of a file
    fn hash_file(path: &Path) -> Option<String> {
        let file = File::open(path).ok()?;
        let mut reader = BufReader::with_capacity(1024 * 1024, file);
        let mut hasher = blake3::Hasher::new();

        let mut buffer = [0u8; 65536]; // 64KB buffer
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    hasher.update(&buffer[..n]);
                }
                Err(_) => return None,
            }
        }

        Some(hasher.finalize().to_hex().to_string())
    }
}

impl Default for DuplicatesScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for DuplicatesScanner {
    fn name(&self) -> &'static str {
        "Duplicates Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let base_path = config.get_base_path();

        // Minimum size for duplicate detection (skip small files)
        let min_size = 1024 * 1024; // 1MB

        // Step 1: Collect files and group by size
        let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();

        for entry in WalkDir::new(&base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                if e.file_type().is_dir() {
                    return !Self::should_skip_dir(e.path());
                }
                true
            })
            .filter_map(|e| e.ok())
        {
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

            // Skip small files
            if size < min_size {
                continue;
            }

            size_groups
                .entry(size)
                .or_default()
                .push(path.to_path_buf());
        }

        // Step 2: For files with matching sizes, compute hashes
        let potential_duplicates: Vec<_> = size_groups
            .into_iter()
            .filter(|(_, paths)| paths.len() > 1)
            .collect();

        // Compute hashes in parallel
        let hash_results: Vec<(PathBuf, u64, Option<String>)> = potential_duplicates
            .into_par_iter()
            .flat_map(|(size, paths)| {
                paths
                    .into_par_iter()
                    .map(move |path| {
                        let hash = Self::hash_file(&path);
                        (path, size, hash)
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        // Step 3: Group by hash
        let mut hash_groups: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();

        for (path, size, hash) in hash_results {
            if let Some(h) = hash {
                hash_groups.entry(h).or_default().push((path, size));
            }
        }

        // Step 4: Create cleanable files from duplicates (keep the oldest one)
        let mut results = Vec::new();

        for (_hash, mut files) in hash_groups {
            if files.len() < 2 {
                continue;
            }

            // Sort by modification time (oldest first)
            files.sort_by(|a, b| {
                let time_a = get_last_accessed(&a.0).unwrap_or_else(Utc::now);
                let time_b = get_last_accessed(&b.0).unwrap_or_else(Utc::now);
                time_a.cmp(&time_b)
            });

            // Keep the first (oldest) file, mark the rest as duplicates
            let (original_path, _) = &files[0];
            let original_name = original_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            for (path, size) in files.into_iter().skip(1) {
                let last_accessed = get_last_accessed(&path).unwrap_or_else(Utc::now);

                results.push(CleanableFile {
                    path,
                    size,
                    category: Category::Duplicate,
                    last_accessed,
                    reason: format!("Duplicate of: {}", original_name),
                    is_directory: false,
                });
            }
        }

        // Sort by size descending
        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}
