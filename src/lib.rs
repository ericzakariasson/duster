//! Duster - A developer-focused CLI tool to clean up unused files and free disk space
//!
//! This library provides the core functionality for scanning, analyzing, and cleaning
//! various types of files that are safe to remove from a developer's system.

pub mod analyzer;
pub mod cleaner;
pub mod cli;
pub mod config;
pub mod scanner;
pub mod ui;
