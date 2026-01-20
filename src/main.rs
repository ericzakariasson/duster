//! Duster - A developer-focused CLI tool to clean up unused files and free disk space

use anyhow::Result;
use clap::Parser;
use colored::*;

mod analyzer;
mod cleaner;
mod cli;
mod config;
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

            // Run scan
            let result = analyzer::run_scan(&options.scan, &config)?;

            if result.files.is_empty() {
                ui::print_info("No cleanable files found.");
                return Ok(());
            }

            // Preview what will be deleted
            cleaner::preview_deletion(&result.files);

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
            let cleanup_result = cleaner::delete_files(&result.files, None)?;
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
