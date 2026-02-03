//! Duster - A developer-focused CLI tool to clean up unused files and free disk space

use anyhow::Result;
use clap::Parser;
use colored::*;

mod analyzer;
mod cleaner;
mod cli;
mod config;
mod scan_cache;
mod scanner;
mod space;
mod ui;

use cli::{Cli, Command};
use config::Config;

fn main() -> Result<()> {
    // Set up Ctrl+C handler
    ctrlc_handler();

    let cli = Cli::parse();

    // Load configuration
    let mut config = Config::load()?;

    match cli.command {
        Command::Scan(options) => {
            // Apply CLI options to config
            config.apply_cli_options(&options);

            // Run scan
            let result = analyzer::run_scan(&options, &config)?;

            if result.files.is_empty() {
                ui::print_info("No cleanable files found.");
                return Ok(());
            }

            // Cache result for clean to reuse if run within 5 minutes
            let _ = scan_cache::save(&result, &options);

            // Print report
            if options.json {
                analyzer::print_json_report(&result)?;
            } else {
                analyzer::print_report(&result);
            }
        }

        Command::Clean(options) => {
            // Apply CLI options to config
            config.apply_cli_options(&options.scan);

            // Use cached scan result if a scan was run within the last 5 minutes with same options
            let result = match scan_cache::load_if_recent_default(&options.scan) {
                Some(cached) => {
                    ui::print_info("Using recent scan result (scan was run within 5 minutes).");
                    cached
                }
                None => analyzer::run_scan(&options.scan, &config)?,
            };

            if result.files.is_empty() {
                ui::print_info("No cleanable files found.");
                return Ok(());
            }

            let files_to_delete = if options.yes {
                result.files.clone()
            } else {
                let selected_categories = cleaner::select_categories(&result.files);
                if selected_categories.is_empty() {
                    ui::print_info("No categories selected.");
                    return Ok(());
                }
                result
                    .files
                    .iter()
                    .filter(|f| selected_categories.contains(&f.category))
                    .cloned()
                    .collect()
            };

            // Preview what will be deleted
            cleaner::preview_deletion(&files_to_delete);

            // Get confirmation
            let should_delete = if options.yes {
                true
            } else {
                println!();
                ui::confirm("Proceed with deletion?")
            };

            if !should_delete {
                ui::print_info("Cleanup cancelled.");
                return Ok(());
            }

            // Delete files
            let cleanup_result = cleaner::delete_files(&files_to_delete, None)?;
            cleaner::print_cleanup_result(&cleanup_result);
        }

        Command::Analyze(options) => {
            // Apply CLI options to config
            config.apply_cli_options(&options.scan);

            // Run scan
            let result = analyzer::run_scan(&options.scan, &config)?;

            if result.files.is_empty() {
                ui::print_info("No cleanable files found.");
                return Ok(());
            }

            // Print detailed report
            if options.scan.json {
                analyzer::print_json_report(&result)?;
            } else {
                analyzer::print_detailed_report(&result);
            }
        }

        Command::Space(options) => {
            space::run(&options)?;
        }

        Command::Config => {
            show_config(&config)?;
        }
    }

    Ok(())
}

/// Show current configuration
fn show_config(config: &Config) -> Result<()> {
    ui::print_header("Current Configuration");

    println!("{:<25} {}", "Min age (days):".bold(), config.min_age_days);
    println!(
        "{:<25} {} MB",
        "Min large size:".bold(),
        config.min_large_size_mb
    );
    println!(
        "{:<25} {}",
        "Project recent (days):".bold(),
        config.project_recent_days
    );
    println!(
        "{:<25} {}",
        "Download age (days):".bold(),
        config.download_age_days
    );

    if !config.excluded_paths.is_empty() {
        println!();
        println!("{}", "Excluded paths:".bold());
        for path in &config.excluded_paths {
            println!("  - {}", path);
        }
    }

    if !config.cache_paths.is_empty() {
        println!();
        println!("{}", "Additional cache paths:".bold());
        for path in &config.cache_paths {
            println!("  - {}", path);
        }
    }

    if !config.custom_paths.is_empty() {
        println!();
        println!("{}", "Custom clean paths:".bold());
        for entry in &config.custom_paths {
            let mut line = format!("  - {} ({})", entry.path, entry.category.as_str());
            if let Some(min_size) = entry.min_size_mb {
                line.push_str(&format!(", min_size={}MB", min_size));
            }
            if let Some(description) = &entry.description {
                line.push_str(&format!(" - {}", description));
            }
            println!("{}", line);
        }
    }

    println!();
    if let Some(config_path) = Config::config_path() {
        if config_path.exists() {
            println!(
                "{} {}",
                "Config file:".dimmed(),
                config_path.display()
            );
        } else {
            println!(
                "{} {} (not created yet)",
                "Config file:".dimmed(),
                config_path.display()
            );
            println!();
            println!(
                "{}",
                "To customize settings, create this file with your preferences.".dimmed()
            );
        }
    }

    println!();
    println!("{}", "Example config.toml:".dimmed());
    println!("{}", "─".repeat(40).dimmed());
    println!(
        "{}",
        r#"min_age_days = 30
min_large_size_mb = 100
project_recent_days = 14
download_age_days = 30
excluded_paths = [
    "important-project/node_modules"
]
custom_paths = [
    { path = "~/Library/Application Support/Cursor Nightly", category = "cache", description = "Cursor Nightly app data" },
    { path = "~/Library/Caches/co.anysphere.cursor.nightly", category = "cache", description = "Cursor Nightly cache" },
    { path = "~/Library/Caches/co.anysphere.cursor.nightly.ShipIt", category = "cache", description = "Cursor Nightly updater cache" },
    { path = "~/dev/everysphere/anyrun/target", category = "build", description = "Anyrun build artifacts" }
]"#
        .dimmed()
    );

    Ok(())
}

/// Set up Ctrl+C handler for graceful shutdown
fn ctrlc_handler() {
    ctrlc::set_handler(move || {
        println!();
        ui::print_warning("Interrupted. Exiting...");
        std::process::exit(130);
    })
    .expect("Error setting Ctrl+C handler");
}
