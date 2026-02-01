# AGENTS.md

## Project overview
Zeterm is a terminal emulator built with Rust and GPUI (Zed's UI framework). It uses alacritty_terminal for terminal emulation and supports SSH connections via russh.

**Architecture**: Workspace-based monorepo with 5 crates:
- `zeterm` - Main application (GUI + bin)
- `zeterm-core` - Core terminal logic and file watching
- `zeterm-ssh` - SSH client functionality
- `zeterm-storage` - SQLite persistence layer
- `zeterm-mock` - Test mocks

## Build and test commands
```bash
# Build entire workspace
cargo build

# Build release binary
cargo build --release

# Run tests for all crates
cargo test --workspace

# Run tests for specific crate
cargo test -p zeterm-core

# Check code without building
cargo check

# Run Clippy lints
cargo clippy --workspace

# Format code
cargo fmt

# Clean build artifacts
cargo clean
```

## Code style guidelines
- **Rust edition**: 2024 (requires Rust 1.92+)
- **Formatter**: Use `rustfmt.toml` config (max_width: 100, 4 spaces)
- **Linter**: Use `clippy.toml` config
- **Imports**: Group as StdExternalCrate, granularity: Module
- **Async**: Use `tokio::time::sleep` instead of `std::thread::sleep`
- **Mutex**: Use `parking_lot::Mutex` or `tokio::sync::Mutex`, not `std::sync::Mutex`
- **Wildcard imports**: Allowed for `gpui::*` and `gpui::prelude::*`

## Testing instructions
- Run `cargo test --workspace` before committing
- Check lints: `cargo clippy --workspace -- -D warnings`
- Format check: `cargo fmt -- --check`
- CI runs on: Windows x64, Linux x64, macOS x64, macOS ARM64
- Find CI config in `.github/workflows/build.yml`

## Security considerations
- SSH keys handled via russh library (supports ECDSA P-256/P-384/P-521)
- Credentials stored via `keyring` crate (OS keychain)
- SQLite database for session storage

## Dev environment tips
- **Linux deps**: `libxcb-*`, `libwayland-dev`, `libvulkan-dev`, `libssl-dev`
- **macOS deps**: `cmake` (via brew)
- **Windows deps**: `cmake` (via chocolatey)
- Use `cargo run -p zeterm` to run the GUI app
- Enable logging: `RUST_LOG=debug cargo run`

## Dependencies notes
- GPUI pulled from Zed's git repo (not crates.io)
- `patch.crates-io` redirects gpui to git version for ashpd compatibility