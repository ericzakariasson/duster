//! Build artifacts scanner with smart "recently used" detection

use super::{calculate_dir_size, get_last_modified, was_modified_within_days, Category, CleanableFile, Scanner};
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use std::path::Path;
use walkdir::WalkDir;

pub struct BuildArtifactsScanner;

impl BuildArtifactsScanner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BuildArtifactsScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Build artifact patterns to scan for
struct ArtifactPattern {
    /// Directory name to look for
    dir_name: &'static str,
    /// Project file that indicates an active project
    project_file: &'static str,
    /// Description of the artifact
    description: &'static str,
}

const ARTIFACT_PATTERNS: &[ArtifactPattern] = &[
    ArtifactPattern {
        dir_name: "node_modules",
        project_file: "package.json",
        description: "Node.js dependencies",
    },
    ArtifactPattern {
        dir_name: "target",
        project_file: "Cargo.toml",
        description: "Rust build artifacts",
    },
    ArtifactPattern {
        dir_name: "__pycache__",
        project_file: "",
        description: "Python bytecode cache",
    },
    ArtifactPattern {
        dir_name: ".pytest_cache",
        project_file: "",
        description: "pytest cache",
    },
    ArtifactPattern {
        dir_name: ".gradle",
        project_file: "build.gradle",
        description: "Gradle cache",
    },
    ArtifactPattern {
        dir_name: "build",
        project_file: "build.gradle",
        description: "Gradle build output",
    },
    ArtifactPattern {
        dir_name: ".next",
        project_file: "next.config.js",
        description: "Next.js build cache",
    },
    ArtifactPattern {
        dir_name: ".nuxt",
        project_file: "nuxt.config.js",
        description: "Nuxt.js build cache",
    },
    ArtifactPattern {
        dir_name: "dist",
        project_file: "package.json",
        description: "Build distribution",
    },
    ArtifactPattern {
        dir_name: "vendor",
        project_file: "composer.json",
        description: "PHP Composer dependencies",
    },
    ArtifactPattern {
        dir_name: "Pods",
        project_file: "Podfile",
        description: "CocoaPods dependencies",
    },
    ArtifactPattern {
        dir_name: ".tox",
        project_file: "tox.ini",
        description: "tox virtual environments",
    },
    ArtifactPattern {
        dir_name: "venv",
        project_file: "",
        description: "Python virtual environment",
    },
    ArtifactPattern {
        dir_name: ".venv",
        project_file: "",
        description: "Python virtual environment",
    },
];

/// Check if a project was recently used by examining project files
fn is_project_recently_used(project_root: &Path, days: u32) -> bool {
    // Check common project files for recent modifications
    let project_files = [
        "package.json",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "Cargo.toml",
        "Cargo.lock",
        "requirements.txt",
        "pyproject.toml",
        "build.gradle",
        "pom.xml",
        "go.mod",
        "composer.json",
        "Gemfile",
        "Podfile",
        ".git/HEAD",
        ".git/index",
    ];

    for file in &project_files {
        let path = project_root.join(file);
        if path.exists() && was_modified_within_days(&path, days) {
            return true;
        }
    }

    // Also check if any source files were modified recently
    let source_extensions = ["rs", "js", "ts", "tsx", "jsx", "py", "go", "java", "rb", "php"];
    
    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if source_extensions.contains(&ext.to_string_lossy().as_ref()) {
                    if was_modified_within_days(&path, days) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

impl Scanner for BuildArtifactsScanner {
    fn name(&self) -> &'static str {
        "Build Artifacts Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();

        let base_path = config.get_base_path();

        // Walk the directory tree looking for build artifacts
        for entry in WalkDir::new(&base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories (except specific ones we care about)
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') {
                    // Allow specific hidden dirs we want to scan
                    return matches!(
                        name.as_ref(),
                        ".next" | ".nuxt" | ".gradle" | ".tox" | ".venv" | ".pytest_cache"
                    );
                }
                // Skip node_modules subdirectories (we handle the whole dir)
                if e.path().components().any(|c| c.as_os_str() == "node_modules") 
                    && e.file_name() != "node_modules" {
                    return false;
                }
                true
            })
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            // Only look at directories
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir_name = match path.file_name() {
                Some(n) => n.to_string_lossy(),
                None => continue,
            };

            // Check if this matches any artifact pattern
            for pattern in ARTIFACT_PATTERNS {
                if dir_name != pattern.dir_name {
                    continue;
                }

                let parent = match path.parent() {
                    Some(p) => p,
                    None => continue,
                };

                // Skip if excluded
                if config.is_excluded(path) {
                    continue;
                }

                // Check if the project file exists (if required)
                if !pattern.project_file.is_empty() {
                    let project_file = parent.join(pattern.project_file);
                    if !project_file.exists() {
                        continue;
                    }
                }

                // Check if project was recently used
                if is_project_recently_used(parent, config.project_recent_days) {
                    continue;
                }

                let size = calculate_dir_size(path);
                let last_modified = get_last_modified(path).unwrap_or_else(Utc::now);

                // Skip small directories (less than 1MB)
                if size < 1024 * 1024 {
                    continue;
                }

                let project_name = parent
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                results.push(CleanableFile {
                    path: path.to_path_buf(),
                    size,
                    category: Category::BuildArtifact,
                    last_accessed: last_modified,
                    reason: format!("{} in project '{}'", pattern.description, project_name),
                    is_directory: true,
                });

                break; // Don't match multiple patterns for the same directory
            }
        }

        // Sort by size descending
        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}

/// Scanner for global package manager caches
pub struct GlobalCacheScanner;

impl GlobalCacheScanner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlobalCacheScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for GlobalCacheScanner {
    fn name(&self) -> &'static str {
        "Global Cache Scanner"
    }

    fn scan(&self, config: &Config) -> Result<Vec<CleanableFile>> {
        let mut results = Vec::new();

        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return Ok(results),
        };

        // Global caches that can be cleaned
        let global_caches = [
            (".cargo/registry/cache", "Cargo registry cache"),
            (".cargo/git/checkouts", "Cargo git checkouts"),
            (".rustup/tmp", "Rustup temp files"),
            (".npm/_cacache", "npm cache"),
            (".yarn/cache", "Yarn cache"),
            (".pnpm-store", "pnpm store"),
            (".gradle/caches", "Gradle caches"),
            (".m2/repository", "Maven repository"),
            (".cache/pip", "pip cache"),
            (".cache/go-build", "Go build cache"),
        ];

        for (rel_path, description) in &global_caches {
            let path = home.join(rel_path);
            
            if !path.exists() {
                continue;
            }

            if config.is_excluded(&path) {
                continue;
            }

            let size = calculate_dir_size(&path);
            let last_modified = get_last_modified(&path).unwrap_or_else(Utc::now);

            // Only include if it's significant (>10MB)
            if size < 10 * 1024 * 1024 {
                continue;
            }

            results.push(CleanableFile {
                path,
                size,
                category: Category::BuildArtifact,
                last_accessed: last_modified,
                reason: description.to_string(),
                is_directory: true,
            });
        }

        results.sort_by(|a, b| b.size.cmp(&a.size));

        Ok(results)
    }
}
