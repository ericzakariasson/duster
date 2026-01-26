//! Cache of recent scan results so clean can reuse them when run shortly after scan.

use crate::cli::ScanOptions;
use crate::scanner::ScanResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_MAX_AGE_SECS: u64 = 300; // 5 minutes

#[derive(Debug, Serialize, Deserialize)]
struct CacheEnvelope {
    timestamp_secs: u64,
    options_key: String,
    result: ScanResult,
}

/// Build a deterministic key from scan options so we can match cached scans.
fn options_fingerprint(options: &ScanOptions) -> String {
    let path = options
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut exclude = options.exclude.clone();
    exclude.sort();
    format!(
        "path={} all={} cache={} trash={} temp={} downloads={} build={} large={} duplicates={} old={} min_age={:?} min_size={:?} project_age={:?} exclude={:?}",
        path,
        options.all,
        options.cache,
        options.trash,
        options.temp,
        options.downloads,
        options.build,
        options.large,
        options.duplicates,
        options.old,
        options.min_age,
        options.min_size,
        options.project_age,
        exclude,
    )
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("duster").join("last_scan.json"))
}

/// Save a scan result for potential reuse by clean.
pub fn save(result: &ScanResult, options: &ScanOptions) -> Result<()> {
    let path = match cache_path() {
        Some(p) => p,
        None => return Ok(()),
    };

    let timestamp_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let envelope = CacheEnvelope {
        timestamp_secs,
        options_key: options_fingerprint(options),
        result: result.clone(),
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache dir: {}", parent.display()))?;
    }

    let data = serde_json::to_string_pretty(&envelope).context("Failed to serialize scan cache")?;
    fs::write(&path, data).with_context(|| format!("Failed to write cache: {}", path.display()))?;

    Ok(())
}

/// Load cached scan result if it exists, is no older than max_age_secs, and options match.
pub fn load_if_recent(options: &ScanOptions, max_age_secs: u64) -> Option<ScanResult> {
    let path = cache_path()?;
    let data = fs::read_to_string(&path).ok()?;
    let envelope: CacheEnvelope = serde_json::from_str(&data).ok()?;

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())?;
    let age_secs = now_secs.saturating_sub(envelope.timestamp_secs);
    if age_secs > max_age_secs {
        return None;
    }

    if envelope.options_key != options_fingerprint(options) {
        return None;
    }

    Some(envelope.result)
}

/// Load cached scan result if it exists, is no older than 5 minutes, and options match.
pub fn load_if_recent_default(options: &ScanOptions) -> Option<ScanResult> {
    load_if_recent(options, CACHE_MAX_AGE_SECS)
}
