# Duster UI

A beautiful desktop UI for duster disk cleanup, built with Tauri + Svelte.

![Duster UI](https://img.shields.io/badge/Built%20with-Tauri%20%2B%20Svelte-orange)

## Prerequisites

1. **Rust** (for Tauri backend)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Node.js** (v18+ recommended)
   ```bash
   # macOS with Homebrew
   brew install node
   ```

3. **Tauri CLI**
   ```bash
   cargo install tauri-cli
   ```

## Development

```bash
# Navigate to duster-ui directory
cd duster-ui

# Install npm dependencies
npm install

# Run in development mode (opens app with hot-reload)
cargo tauri dev
```

## Building

```bash
# Build for production
cargo tauri build
```

The built app will be in `src-tauri/target/release/bundle/`.

## Project Structure

```
duster-ui/
├── src/                    # Svelte frontend
│   ├── App.svelte          # Main application component
│   └── main.js             # Entry point
├── src-tauri/              # Tauri/Rust backend
│   ├── src/
│   │   ├── lib.rs          # Tauri commands (bridges to duster library)
│   │   └── main.rs         # App entry point
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
├── public/                 # Static assets
└── package.json            # Node dependencies
```

## Features

- 🔍 **Scan** - Find cleanable files across multiple categories
- 📊 **Analyze** - See detailed breakdown by category
- 🧹 **Clean** - Remove selected files with one click
- 🎨 **Beautiful UI** - Modern, dark-themed interface with smooth animations

## How It Works

The UI integrates with the existing `duster` Rust library:

1. **Frontend** (Svelte) handles the user interface
2. **Tauri Commands** bridge frontend to Rust functions
3. **Duster Library** performs actual scanning and cleaning

The app uses the same scanner logic as the CLI, so results are identical.
