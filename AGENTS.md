# Agents

## Cloud-specific instructions

**Duster** is a Rust CLI tool for disk cleanup — it has no web UI, no database, and no long-running services.

### Rust toolchain

The VM ships with Rust 1.83 pinned as default. This is too old for some transitive dependencies (e.g. `blake3` requires `edition2024`). The update script runs `rustup default stable` to ensure the latest stable toolchain is active before `cargo build`.

### Build / Test / Lint / Run

All standard Cargo commands; see `README.md` for CLI usage examples.

| Task | Command |
|------|---------|
| Build | `cargo build` |
| Test | `cargo test` |
| Lint | `cargo clippy` |
| Format check | `cargo fmt --check` |
| Run (example) | `cargo run -- space` |
| Run scan | `cargo run -- scan --path <dir>` |

### Notes

- `cargo clippy -- -D warnings` fails on the current codebase due to pre-existing warnings (derivable_impls, collapsible_if, redundant_closure). Use `cargo clippy` (without `-D warnings`) for a non-blocking lint check.
- `cargo fmt --check` shows formatting diffs in the existing code. These are pre-existing and not caused by agent changes.
- The tool scans the home directory by default. Use `--path` to scope scans to a specific directory during testing.
- The `clean` subcommand prompts interactively; pass `-y` to auto-confirm in non-interactive contexts.
