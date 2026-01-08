# duster

Disk cleanup CLI for developers. Finds and removes build artifacts, caches, and other space hogs.

## Install

```bash
cargo install --path .
```

## Commands

```bash
duster scan              # Find cleanable files (dry-run)
duster clean             # Delete files (with confirmation)
duster clean -y          # Delete without confirmation
duster analyze           # Detailed breakdown by category
duster config            # Show current settings
```

## Categories

```bash
--cache       # App/system caches (~/.cache, ~/Library/Caches)
--trash       # Trash bin
--temp        # Temp files older than 1 day
--downloads   # Old files in ~/Downloads
--build       # Build artifacts from inactive projects (node_modules, target/, etc.)
--large       # Files over 100MB
--duplicates  # Duplicate files (by hash)
--old         # Files not accessed in 30+ days
--all, -a     # All categories (default if none specified)
```

## Options

```bash
--min-age <DAYS>      # Age threshold for old files (default: 30)
--min-size <SIZE>     # Size threshold for large files (default: 100MB)
--project-age <DAYS>  # Projects inactive for this long are cleanable (default: 14)
--path <PATH>         # Scan path (default: home directory)
--exclude <PATTERN>   # Exclude matching paths (repeatable)
--json                # Output as JSON
```

## Examples

```bash
# Quick cache cleanup
duster clean --cache --trash -y

# Find build artifacts from old projects
duster scan --build --project-age 30

# Large files over 500MB
duster scan --large --min-size 500MB

# Everything as JSON
duster scan --json
```

## Config File

Optional: `~/.config/duster/config.toml`

```toml
min_age_days = 30
min_large_size_mb = 100
project_recent_days = 14
download_age_days = 30
excluded_paths = ["important-project/node_modules"]
```

## How Build Detection Works

Build artifacts (`node_modules`, `target/`, `.gradle`, etc.) are only flagged if the parent project hasn't been modified within `--project-age` days. This protects active projects.
