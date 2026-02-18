//! Filesystem disk space helpers (total / free) for a given path.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use sysinfo::Disks;

#[derive(Debug, Clone)]
pub(crate) struct FsSpace {
    pub(crate) total_bytes: u64,
    pub(crate) free_bytes: u64,
    pub(crate) mount_point: PathBuf,
}

pub(crate) fn resolve_target_path(path: Option<&PathBuf>) -> Result<PathBuf> {
    let path = if let Some(p) = path {
        p.clone()
    } else if let Some(home) = dirs::home_dir() {
        home
    } else {
        std::env::current_dir().context("Could not determine current directory")?
    };

    path.canonicalize()
        .with_context(|| format!("Path does not exist: {}", path.display()))
}

pub(crate) fn fs_space_for_path(target: &Path) -> Result<FsSpace> {
    let disks = Disks::new_with_refreshed_list();

    let mut matching: Vec<_> = disks
        .list()
        .iter()
        .filter(|disk| target.starts_with(disk.mount_point()))
        .map(|disk| (disk.mount_point().to_path_buf(), disk))
        .collect();

    // Longest mount point first (handles nested mounts like / vs /home).
    matching.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));

    let (mount_point, disk) = matching
        .into_iter()
        .next()
        .context("No disk found containing the given path")?;

    Ok(FsSpace {
        total_bytes: disk.total_space(),
        free_bytes: disk.available_space(),
        mount_point,
    })
}

pub(crate) fn used_bytes(space: &FsSpace) -> u64 {
    space.total_bytes.saturating_sub(space.free_bytes)
}

pub(crate) fn used_percent(space: &FsSpace) -> u16 {
    if space.total_bytes == 0 {
        return 0;
    }
    let used = used_bytes(space) as f64;
    let total = space.total_bytes as f64;
    let pct = (used / total) * 100.0;
    pct.round().clamp(0.0, 100.0) as u16
}

