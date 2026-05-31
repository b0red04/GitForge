# Contributing to GitForge

Thank you for your interest in contributing to GitForge! This guide will help you get started.

## Development Setup

### Prerequisites

- Rust 1.85+ (edition 2024)
- System libraries: see README.md for your distro

### Building

```bash
cargo build -p gitforge-app
cargo test --workspace
```

### Running

```bash
cargo run -p gitforge-app
```

## Code Style

- Follow standard Rust conventions (`cargo fmt`)
- No warnings from `cargo clippy`
- No unnecessary comments — code should be self-documenting
- Use `tracing` for logging, not `println!`

## Architecture

GitForge uses a panel-based architecture:

- **GitForgeApp** — Main view, owns all state, delegates to panels
- **GraphPanel** — Commit graph rendering and selection
- **DiffPanel** — Diff content rendering
- **StatusPanel** — Staging area and commit workflow
- **Sidebar** — Ref tree (branches, remotes, tags)

Panels are plain structs (not GPUI entities), owned by `GitForgeApp`. Each panel has a `render()` method that produces GPUI elements.

### Async Pattern

All blocking git I/O uses `tokio::spawn_blocking`. Results are sent back to the main thread via `this.update(cx, ...)`.

### Theme System

All colors flow from the `Theme` JSON → `AppColors` (hex→RGBA) → rendering. Never hardcode colors in render methods.

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `cargo fmt`, `cargo clippy`, `cargo test`
5. Submit a PR with a clear description

## Reporting Issues

Please use GitHub Issues and include:
- Your OS and desktop environment
- GitForge version (commit hash or release)
- Steps to reproduce
- Expected vs actual behavior
