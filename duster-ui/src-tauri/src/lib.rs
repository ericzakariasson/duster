//! Tauri commands for duster UI

use duster::cli::ScanOptions;
use duster::config::Config;
use duster::scanner::{Category, ScanResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Serializable scan result for the frontend
#[derive(Debug, Serialize)]
pub struct UiScanResult {
    pub files: Vec<UiCleanableFile>,
    pub errors: Vec<String>,
    pub total_size: u64,
    pub total_count: usize,
    pub by_category: Vec<UiCategoryStats>,
}

#[derive(Debug, Serialize)]
pub struct UiCleanableFile {
    pub path: String,
    pub size: u64,
    pub size_formatted: String,
    pub category: String,
    pub category_key: String,
    pub last_accessed: String,
    pub reason: String,
    pub is_directory: bool,
}

#[derive(Debug, Serialize)]
pub struct UiCategoryStats {
    pub key: String,
    pub name: String,
    pub description: String,
    pub count: usize,
    pub size: u64,
    pub size_formatted: String,
}

#[derive(Debug, Serialize)]
pub struct UiConfig {
    pub min_age_days: u32,
    pub min_large_size_mb: u64,
    pub project_recent_days: u32,
    pub download_age_days: u32,
    pub excluded_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UiScanOptions {
    pub categories: Vec<String>,
    pub min_age: Option<u32>,
    pub min_size_mb: Option<u64>,
    pub project_age: Option<u32>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UiCleanupResult {
    pub deleted_count: usize,
    pub freed_bytes: u64,
    pub freed_formatted: String,
    pub errors: Vec<String>,
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn category_to_key(cat: &Category) -> String {
    match cat {
        Category::Cache => "cache".to_string(),
        Category::Trash => "trash".to_string(),
        Category::Temp => "temp".to_string(),
        Category::Downloads => "downloads".to_string(),
        Category::BuildArtifact => "build".to_string(),
        Category::LargeFile => "large".to_string(),
        Category::Duplicate => "duplicates".to_string(),
        Category::OldFile => "old".to_string(),
    }
}


fn convert_scan_result(result: &ScanResult) -> UiScanResult {
    let by_category = result.by_category();
    
    let category_stats: Vec<UiCategoryStats> = by_category
        .iter()
        .map(|(cat, files)| {
            let size: u64 = files.iter().map(|f| f.size).sum();
            UiCategoryStats {
                key: category_to_key(cat),
                name: cat.display_name().to_string(),
                description: cat.description().to_string(),
                count: files.len(),
                size,
                size_formatted: format_size(size),
            }
        })
        .collect();

    let files: Vec<UiCleanableFile> = result
        .files
        .iter()
        .map(|f| UiCleanableFile {
            path: f.path.display().to_string(),
            size: f.size,
            size_formatted: format_size(f.size),
            category: f.category.display_name().to_string(),
            category_key: category_to_key(&f.category),
            last_accessed: f.last_accessed.format("%Y-%m-%d %H:%M").to_string(),
            reason: f.reason.clone(),
            is_directory: f.is_directory,
        })
        .collect();

    UiScanResult {
        total_size: result.total_size(),
        total_count: result.total_count(),
        files,
        errors: result.errors.clone(),
        by_category: category_stats,
    }
}

/// Get the current configuration
#[tauri::command]
async fn get_config() -> Result<UiConfig, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    
    Ok(UiConfig {
        min_age_days: config.min_age_days,
        min_large_size_mb: config.min_large_size_mb,
        project_recent_days: config.project_recent_days,
        download_age_days: config.download_age_days,
        excluded_paths: config.excluded_paths,
    })
}

/// Run a scan with the specified options
#[tauri::command]
async fn run_scan(options: UiScanOptions) -> Result<UiScanResult, String> {
    let mut config = Config::load().map_err(|e| e.to_string())?;
    
    // Apply options
    if let Some(min_age) = options.min_age {
        config.min_age_days = min_age;
    }
    if let Some(min_size_mb) = options.min_size_mb {
        config.min_large_size_mb = min_size_mb;
    }
    if let Some(project_age) = options.project_age {
        config.project_recent_days = project_age;
    }
    if let Some(ref path) = options.path {
        config.base_path = Some(PathBuf::from(path));
    }
    
    // Build scan options
    let scan_options = ScanOptions {
        all: options.categories.is_empty(),
        cache: options.categories.contains(&"cache".to_string()),
        trash: options.categories.contains(&"trash".to_string()),
        temp: options.categories.contains(&"temp".to_string()),
        downloads: options.categories.contains(&"downloads".to_string()),
        build: options.categories.contains(&"build".to_string()),
        large: options.categories.contains(&"large".to_string()),
        duplicates: options.categories.contains(&"duplicates".to_string()),
        old: options.categories.contains(&"old".to_string()),
        min_age: options.min_age,
        min_size: options.min_size_mb.map(|m| format!("{}MB", m)),
        project_age: options.project_age,
        path: options.path.map(PathBuf::from),
        exclude: vec![],
        json: false,
    };
    
    let result = duster::analyzer::run_scan(&scan_options, &config)
        .map_err(|e| e.to_string())?;
    
    Ok(convert_scan_result(&result))
}

/// Delete specified files
#[tauri::command]
async fn delete_files(paths: Vec<String>) -> Result<UiCleanupResult, String> {
    let mut result = UiCleanupResult {
        deleted_count: 0,
        freed_bytes: 0,
        freed_formatted: "0 B".to_string(),
        errors: vec![],
    };
    
    for path_str in paths {
        let path = std::path::Path::new(&path_str);
        
        let size = if path.is_dir() {
            duster::scanner::calculate_dir_size(path)
        } else {
            path.metadata().map(|m| m.len()).unwrap_or(0)
        };
        
        let delete_result = if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        
        match delete_result {
            Ok(_) => {
                result.deleted_count += 1;
                result.freed_bytes += size;
            }
            Err(e) => {
                result.errors.push(format!("{}: {}", path_str, e));
            }
        }
    }
    
    result.freed_formatted = format_size(result.freed_bytes);
    Ok(result)
}

/// Get home directory path
#[tauri::command]
async fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.display().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

/// Get available categories
#[tauri::command]
async fn get_categories() -> Vec<UiCategoryStats> {
    vec![
        UiCategoryStats {
            key: "cache".to_string(),
            name: "System Cache".to_string(),
            description: "Cached data from applications and system".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
        UiCategoryStats {
            key: "trash".to_string(),
            name: "Trash".to_string(),
            description: "Files in the trash bin".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
        UiCategoryStats {
            key: "temp".to_string(),
            name: "Temp Files".to_string(),
            description: "Temporary files from /tmp and similar".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
        UiCategoryStats {
            key: "downloads".to_string(),
            name: "Old Downloads".to_string(),
            description: "Old files in Downloads folder".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
        UiCategoryStats {
            key: "build".to_string(),
            name: "Build Artifacts".to_string(),
            description: "Build outputs and dependencies (node_modules, target, etc.)".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
        UiCategoryStats {
            key: "large".to_string(),
            name: "Large Files".to_string(),
            description: "Large files that may not be needed".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
        UiCategoryStats {
            key: "duplicates".to_string(),
            name: "Duplicates".to_string(),
            description: "Duplicate files wasting space".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
        UiCategoryStats {
            key: "old".to_string(),
            name: "Old Files".to_string(),
            description: "Files not accessed for a long time".to_string(),
            count: 0,
            size: 0,
            size_formatted: "0 B".to_string(),
        },
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_config,
            run_scan,
            delete_files,
            get_home_dir,
            get_categories,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
