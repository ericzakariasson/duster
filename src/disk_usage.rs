//! Directory-level disk usage scanning for the TUI.
//!
//! This is intentionally simple (file length sums). It does not attempt to
//! account for filesystem block sizes or sparse files.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone)]
pub(crate) struct ChildUsage {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) kind: EntryKind,
    pub(crate) size_bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct DirectoryUsage {
    pub(crate) path: PathBuf,
    pub(crate) total_bytes: u64,
    pub(crate) children: Vec<ChildUsage>,
    pub(crate) error_count: usize,
}

pub(crate) fn scan_directory_children(path: &Path, show_hidden: bool) -> Result<DirectoryUsage> {
    let meta = std::fs::metadata(path)
        .with_context(|| format!("Failed to stat path: {}", path.display()))?;
    if !meta.is_dir() {
        anyhow::bail!("Not a directory: {}", path.display());
    }

    let mut children: Vec<ChildUsage> = Vec::new();
    let mut index_by_name: HashMap<OsString, usize> = HashMap::new();
    let mut error_count: usize = 0;

    let dir_entries =
        std::fs::read_dir(path).with_context(|| format!("Failed to read {}", path.display()))?;

    for entry in dir_entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                error_count += 1;
                continue;
            }
        };

        let name_os = entry.file_name();
        if !show_hidden && is_hidden_name(&name_os) {
            continue;
        }

        let child_path = entry.path();
        let meta = match std::fs::symlink_metadata(&child_path) {
            Ok(m) => m,
            Err(_) => {
                error_count += 1;
                continue;
            }
        };
        let ft = meta.file_type();

        let kind = if ft.is_dir() {
            EntryKind::Directory
        } else if ft.is_file() {
            EntryKind::File
        } else if ft.is_symlink() {
            EntryKind::Symlink
        } else {
            EntryKind::Other
        };

        let size = match kind {
            EntryKind::File | EntryKind::Symlink => meta.len(),
            EntryKind::Directory | EntryKind::Other => 0,
        };

        let idx = children.len();
        children.push(ChildUsage {
            name: name_os.to_string_lossy().to_string(),
            path: child_path,
            kind,
            size_bytes: size,
        });
        index_by_name.insert(name_os, idx);
    }

    // Walk the subtree once and bucket file sizes by the first path component.
    let walker = WalkDir::new(path)
        .follow_links(false)
        .min_depth(1)
        .same_file_system(true)
        .into_iter()
        .filter_entry(|e| show_hidden || !is_hidden_path(e.path()));

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                error_count += 1;
                continue;
            }
        };

        // We already recorded depth=1 files via read_dir; only count files below children.
        if entry.depth() <= 1 {
            continue;
        }
        if !(entry.file_type().is_file() || entry.file_type().is_symlink()) {
            continue;
        }

        let size = match std::fs::symlink_metadata(entry.path()) {
            Ok(m) => m.len(),
            Err(_) => {
                error_count += 1;
                continue;
            }
        };

        let Some(first) = first_component(path, entry.path()) else {
            continue;
        };
        let Some(idx) = index_by_name.get(&first) else {
            continue;
        };
        if let Some(child) = children.get_mut(*idx) {
            child.size_bytes = child.size_bytes.saturating_add(size);
        }
    }

    // Sort for display (largest first).
    children.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.name.cmp(&b.name)));

    let total_bytes: u64 = children.iter().map(|c| c.size_bytes).sum();
    Ok(DirectoryUsage {
        path: path.to_path_buf(),
        total_bytes,
        children,
        error_count,
    })
}

fn is_hidden_name(name: &OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .map(is_hidden_name)
        .unwrap_or(false)
}

fn first_component(root: &Path, full_path: &Path) -> Option<OsString> {
    let rel = full_path.strip_prefix(root).ok()?;
    let mut components = rel.components();
    let first = components.next()?;
    match first {
        std::path::Component::Normal(os) => Some(os.to_os_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("{}_{}_{}", prefix, std::process::id(), nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scan_buckets_sizes_by_immediate_child() {
        let root = temp_dir("duster_disk_usage");
        let _cleanup = Cleanup(root.clone());

        std::fs::write(root.join("a.txt"), b"abc").unwrap(); // 3 bytes
        std::fs::create_dir_all(root.join("dir1")).unwrap();
        std::fs::write(root.join("dir1").join("b.bin"), vec![0u8; 10]).unwrap();
        std::fs::create_dir_all(root.join("dir2")).unwrap();

        let usage = scan_directory_children(&root, true).unwrap();

        let mut by_name: HashMap<String, u64> = HashMap::new();
        for child in usage.children {
            by_name.insert(child.name, child.size_bytes);
        }

        assert_eq!(by_name.get("a.txt").copied(), Some(3));
        assert_eq!(by_name.get("dir1").copied(), Some(10));
        assert_eq!(by_name.get("dir2").copied(), Some(0));
    }

    #[test]
    fn scan_respects_hidden_toggle() {
        let root = temp_dir("duster_disk_usage_hidden");
        let _cleanup = Cleanup(root.clone());

        std::fs::write(root.join(".hidden_file"), vec![0u8; 5]).unwrap();
        std::fs::create_dir_all(root.join(".hidden_dir")).unwrap();
        std::fs::write(root.join(".hidden_dir").join("x"), vec![0u8; 7]).unwrap();
        std::fs::write(root.join("visible"), vec![0u8; 2]).unwrap();

        let usage = scan_directory_children(&root, false).unwrap();
        let names: Vec<_> = usage.children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"visible"));
        assert!(!names.contains(&".hidden_file"));
        assert!(!names.contains(&".hidden_dir"));
    }

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

