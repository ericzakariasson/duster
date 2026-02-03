# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-02-02

### Added

- Scan result cache: if you run `duster clean` within 5 minutes of `duster scan` with the same options, the last scan result is reused and no new scan is run. Cache is stored under the platform cache dir (e.g. `~/.cache/duster/last_scan.json`).
- Custom clean paths: define additional cleanable directories in config via `custom_paths` with per-entry category, description, and minimum size threshold.
- Category selection in `duster clean`: interactively pick which categories to delete instead of cleaning everything at once.

### Fixed

- `excluded_paths` now correctly handles `~/`-prefixed patterns by expanding them to the home directory before matching. Previously, patterns like `~/.local/share/cursor-agent` would never match absolute paths.

## [0.1.1] - 2026-01-19

### Added

- `duster space` command to report total and free disk space for the filesystem containing a path (default: home directory). Supports `--path <PATH>` and `--json` for machine-readable output.

## [0.1.0]

### Added

- Initial release with `scan`, `clean`, `analyze`, and `config` commands.
- Categories: cache, trash, temp, downloads, build artifacts, large files, duplicates, old files.
