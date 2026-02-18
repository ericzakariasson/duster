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
    let used = total.saturating_sub(free);
    let used_ratio = used_ratio(total, free);
    let used_percent = used_ratio * 100.0;
    let free_percent = (1.0 - used_ratio) * 100.0;

    let used_text = format!(
        "Used: {} ({used_percent:.1}%)",
        ui::format_size(used)
    );
    let used_text = if used_ratio >= 0.90 {
        used_text.red()
    } else if used_ratio >= 0.75 {
        used_text.yellow()
    } else {
        used_text.green()
    };

    println!(
        "{}  |  {}  |  {}",
        format!("Total: {}", ui::format_size(total)).yellow(),
        used_text,
        format!("Free: {} ({free_percent:.1}%)", ui::format_size(free)).green()
    );

    let bar_width = 30;
    let (filled, empty) = bar_segments(used_ratio, bar_width);
    let filled_str = "#".repeat(filled);
    let empty_str = "-".repeat(empty);
    let filled_str = if used_ratio >= 0.90 {
        filled_str.red()
    } else if used_ratio >= 0.75 {
        filled_str.yellow()
    } else {
        filled_str.green()
    };

    println!();
    println!(
        "{} [{}{}] {}",
        "Usage:".dimmed(),
        filled_str,
        empty_str.dimmed(),
        format!("{used_percent:.1}% used").bold()
    );
    println!();
    println!("{} {}", "Mount point:".dimmed(), mount_point.display());
}

fn used_ratio(total: u64, free: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let used = total.saturating_sub(free);
    (used as f64 / total as f64).clamp(0.0, 1.0)
}

fn bar_segments(used_ratio: f64, width: usize) -> (usize, usize) {
    if width == 0 {
        return (0, 0);
    }
    let ratio = used_ratio.clamp(0.0, 1.0);
    let filled = ((ratio * width as f64).round() as usize).min(width);
    (filled, width - filled)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_ratio_is_clamped() {
        assert_eq!(used_ratio(0, 0), 0.0);
        assert_eq!(used_ratio(100, 150), 0.0);
        assert_eq!(used_ratio(100, 0), 1.0);
        assert!((used_ratio(100, 25) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn bar_segments_handles_width_and_rounding() {
        assert_eq!(bar_segments(0.0, 10), (0, 10));
        assert_eq!(bar_segments(1.0, 10), (10, 0));
        assert_eq!(bar_segments(-1.0, 10), (0, 10));
        assert_eq!(bar_segments(2.0, 10), (10, 0));
        assert_eq!(bar_segments(0.75, 20), (15, 5));
        assert_eq!(bar_segments(0.0, 0), (0, 0));
    }
}
