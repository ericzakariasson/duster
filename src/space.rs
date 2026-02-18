//! Disk space reporting (total / free) for a given path's filesystem

use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::cli::SpaceOptions;
use crate::disk;
use crate::ui;

/// Run the space command: resolve path, find disk, print total/free.
pub fn run(options: &SpaceOptions) -> Result<()> {
    let path = disk::resolve_target_path(options.path.as_ref())?;
    let space = disk::fs_space_for_path(&path)?;
    let total = space.total_bytes;
    let free = space.free_bytes;
    let mount_point = space.mount_point;

    if options.json {
        print_json(total, free, &mount_point)?;
    } else {
        print_human(total, free, &mount_point);
    }

    Ok(())
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
