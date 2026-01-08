//! CLI argument definitions using clap derive

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// A developer-focused CLI tool to clean up unused files and free disk space
#[derive(Parser, Debug)]
#[command(name = "duster")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Find cleanable files (dry-run), show report
    Scan(ScanOptions),

    /// Delete files with interactive confirmation
    Clean(CleanOptions),

    /// Show disk usage breakdown by category
    Analyze(AnalyzeOptions),

    /// Show or edit configuration
    Config,
}

/// Options shared between scan, clean, and analyze commands
#[derive(Parser, Debug, Clone)]
pub struct ScanOptions {
    /// Scan all categories
    #[arg(short, long)]
    pub all: bool,

    /// Include system/app caches
    #[arg(long)]
    pub cache: bool,

    /// Include trash bin
    #[arg(long)]
    pub trash: bool,

    /// Include temp files
    #[arg(long)]
    pub temp: bool,

    /// Include old downloads
    #[arg(long)]
    pub downloads: bool,

    /// Include build artifacts (node_modules, target, etc.)
    #[arg(long)]
    pub build: bool,

    /// Include large files
    #[arg(long)]
    pub large: bool,

    /// Include duplicate files
    #[arg(long)]
    pub duplicates: bool,

    /// Include old unused files
    #[arg(long)]
    pub old: bool,

    /// Minimum age in days for "old" files (default: 30)
    #[arg(long, value_name = "DAYS")]
    pub min_age: Option<u32>,

    /// Minimum size for "large" files (e.g., "100MB", "1GB")
    #[arg(long, value_name = "SIZE")]
    pub min_size: Option<String>,

    /// Consider project "recent" if accessed within X days (default: 14)
    #[arg(long, value_name = "DAYS")]
    pub project_age: Option<u32>,

    /// Custom path to scan (default: home directory)
    #[arg(long, value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Exclude paths matching pattern (can be repeated)
    #[arg(long, value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CleanOptions {
    #[command(flatten)]
    pub scan: ScanOptions,

    /// Skip confirmation prompts
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Parser, Debug)]
pub struct AnalyzeOptions {
    #[command(flatten)]
    pub scan: ScanOptions,
}

impl ScanOptions {
    /// Returns true if no specific category was selected (defaults to all)
    pub fn no_categories_selected(&self) -> bool {
        !self.cache
            && !self.trash
            && !self.temp
            && !self.downloads
            && !self.build
            && !self.large
            && !self.duplicates
            && !self.old
    }

    /// Returns true if a category should be included in the scan
    pub fn should_scan(&self, category: ScanCategory) -> bool {
        if self.all || self.no_categories_selected() {
            return true;
        }

        match category {
            ScanCategory::Cache => self.cache,
            ScanCategory::Trash => self.trash,
            ScanCategory::Temp => self.temp,
            ScanCategory::Downloads => self.downloads,
            ScanCategory::Build => self.build,
            ScanCategory::Large => self.large,
            ScanCategory::Duplicates => self.duplicates,
            ScanCategory::Old => self.old,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanCategory {
    Cache,
    Trash,
    Temp,
    Downloads,
    Build,
    Large,
    Duplicates,
    Old,
}
