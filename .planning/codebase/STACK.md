# Technology Stack

**Analysis Date:** 2026-01-23

## Languages

**Primary:**
- Rust 1.91.1 - Core library implementation and all source code

**Specification Reference:**
- TypeScript (@rlabs-inc/signals) - The canonical spec being ported; not a runtime dependency but a design reference at `/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals`

## Runtime

**Environment:**
- Rust compiler 1.91.1 (Homebrew)
- Cargo 1.91.1 (Homebrew) - Package and build manager

**Edition:**
- Rust Edition 2024 (Cargo.toml specifies `edition = "2024"`)

**Minimum Supported Rust Version (MSRV):**
- Rust 1.85+ (specified in Cargo.toml as `rust-version = "1.85"`)

**Package Manager:**
- Cargo
- Lockfile: `Cargo.lock` present at `/Users/rusty/Documents/Projects/TUI/tui-rust/crates/spark-signals/Cargo.lock`

## Frameworks

**Core:**
- None - This is a library crate with zero production dependencies

**Testing:**
- No built-in test framework specified in dependencies (Rust's built-in `#[test]` would be used)

**Build/Dev:**
- Criterion 0.5 (with `html_reports` feature) - Statistical benchmarking framework
  - Location: `dev-dependencies` in `Cargo.toml`
  - Run: `cargo bench`
  - Benchmark definition: `/Users/rusty/Documents/Projects/TUI/tui-rust/crates/spark-signals/benches/signals.rs`

## Key Dependencies

**Production Dependencies:**
- **NONE** - The library has zero production dependencies intentionally

**Development Dependencies:**
- **Criterion 0.5** - Benchmarking framework with HTML report generation
  - Includes: rayon (1.11.0), serde (1.0.228), serde_json (1.0.149), regex (1.12.2), plotters (0.3.7)
  - These are transitive dependencies only for benchmarking

## Features

**Feature Flags:**
- `default` - Empty (no features enabled by default)
- `sync` - Thread-safe signals using `Arc<RwLock<T>>` instead of `Rc<RefCell<T>>` (not yet implemented)

## Configuration

**Environment:**
- No environment variables required for library functionality
- No external configuration files needed

**Build:**
- `Cargo.toml` at `/Users/rusty/Documents/Projects/TUI/tui-rust/crates/spark-signals/Cargo.toml`
- `Cargo.lock` for reproducible builds

## Platform Requirements

**Development:**
- macOS (Darwin 25.2.0 confirmed, but Rust is cross-platform)
- Rust 1.85 or later installed via Homebrew or rustup

**Compilation Targets:**
- Default: native platform (x86_64-unknown-linux-gnu, x86_64-apple-darwin, etc.)
- WASM support: Not currently configured but Criterion has WASM support available

**Production:**
- Any platform Rust supports (Linux, macOS, Windows, WASM, embedded)
- No native dependencies - pure Rust

## Build Artifacts

**Output:**
- Library crate name: `spark_signals`
- Generated: `target/debug/` and `target/release/` directories
- Benchmarks: `target/criterion/` (criterion.rs benchmark results with HTML reports)

## Workspace Structure

**Type:** Standalone crate (not part of a larger workspace)
- Config: `[workspace]` section in `Cargo.toml` is empty
- Location: `/Users/rusty/Documents/Projects/TUI/tui-rust/crates/spark-signals/`

---

*Stack analysis: 2026-01-23*
