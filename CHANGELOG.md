# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-01-19

### Added

- `duster space` command to report total and free disk space for the filesystem containing a path (default: home directory). Supports `--path <PATH>` and `--json` for machine-readable output.

## [0.1.0]

### Added

- Initial release with `scan`, `clean`, `analyze`, and `config` commands.
- Categories: cache, trash, temp, downloads, build artifacts, large files, duplicates, old files.
