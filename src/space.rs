//! Disk space reporting (total / free) for a given path's filesystem

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};
use sysinfo::Disks;

use crate::cli::SpaceOptions;
use crate::ui;

/// Run the space command: resolve path, find disk, print total/free.
pub fn run(options: &SpaceOptions) -> Result<()> {
    let path = resolve_target_path(options)?;
    let (total, free, mount_point) = find_disk_for_path(&path)?;

    if options.json {
        print_json(total, free, &mount_point)?;
    } else {
        print_human(total, free, &mount_point);
    }

    Ok(())
}

fn resolve_target_path(options: &SpaceOptions) -> Result<PathBuf> {
    let path = if let Some(ref p) = options.path {
        p.clone()
    } else if let Some(home) = dirs::home_dir() {
        home
    } else {
        std::env::current_dir().context("Could not determine current directory")?
    };

    let canonical = path
        .canonicalize()
        .with_context(|| format!("Path does not exist: {}", path.display()))?;
    Ok(canonical)
}

fn find_disk_for_path(target: &Path) -> Result<(u64, u64, PathBuf)> {
    let disks = Disks::new_with_refreshed_list();

    let mut matching: Vec<_> = disks
        .list()
        .iter()
        .filter(|disk| target.starts_with(disk.mount_point()))
        .map(|disk| (disk.mount_point().to_path_buf(), disk))
        .collect();

    // Longest mount point first (handles nested mounts like / vs /home)
    matching.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));

    let (mount_point, disk) = matching
        .into_iter()
        .next()
        .context("No disk found containing the given path")?;

    let total = disk.total_space();
    let free = disk.available_space();
    Ok((total, free, mount_point))
}

fn print_human(total: u64, free: u64, mount_point: &Path) {
    ui::print_header("Disk space");
    println!(
        "{}  |  {}",
        format!("Total: {}", ui::format_size(total)).yellow(),
        format!("Free: {}", ui::format_size(free)).green()
    );
    println!();
    println!("{} {}", "Mount point:".dimmed(), mount_point.display());
}

fn print_json(total: u64, free: u64, mount_point: &Path) -> Result<()> {
    let output = serde_json::json!({
        "total_bytes": total,
        "free_bytes": free,
        "total_formatted": ui::format_size(total),
        "free_formatted": ui::format_size(free),
        "mount_point": mount_point.display().to_string(),
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
