# External Integrations

**Analysis Date:** 2026-01-23

## APIs & External Services

**None** - This is a pure Rust library with no external API integrations.

## Data Storage

**Databases:**
- Not applicable - Library only

**File Storage:**
- Not applicable - Library only

**Caching:**
- In-memory only via Rust's `Rc<RefCell<T>>` (stack-local) or `Arc<RwLock<T>>` (with `sync` feature)
- No external caching service

## Authentication & Identity

**Auth Provider:**
- Not applicable - Library only

## Monitoring & Observability

**Error Tracking:**
- None configured

**Logs:**
- Debug output via Criterion during benchmarking
- Library itself has no logging (uses silent fail patterns per TypeScript spec)

**Benchmarks:**
- Criterion 0.5 generates HTML reports in `target/criterion/`
- Run: `cargo bench`
- Reports location: `target/criterion/report/index.html`

## CI/CD & Deployment

**Hosting:**
- Repository: `https://github.com/RLabs-Inc/spark-signals` (metadata in Cargo.toml)
- Local development only at present

**CI Pipeline:**
- Not yet configured (no GitHub Actions, CI config files found)
- Manual testing via `cargo bench` and `cargo test`

## Environment Configuration

**Required env vars:**
- None

**Development env vars:**
- None required (all configuration in Cargo.toml and source code)

**Secrets location:**
- Not applicable - No credentials needed

## Crates.io Publishing

**Package Metadata (Cargo.toml):**
- name: `spark-signals`
- version: `0.1.0`
- license: `MIT`
- description: "A standalone reactive signals library for Rust - fine-grained reactivity for any application"
- repository: `https://github.com/RLabs-Inc/spark-signals`
- keywords: `["reactive", "signals", "state-management", "fine-grained"]`
- categories: `["data-structures", "rust-patterns"]`

## Webhooks & Callbacks

**Incoming:**
- Not applicable

**Outgoing:**
- Not applicable

## Transitive Integration Stack (via Criterion dev-dependency)

While the library has **zero production dependencies**, Criterion (dev-only) transitively depends on:

**Data Processing:**
- serde 1.0.228 - Serialization framework
- serde_json 1.0.149 - JSON serialization for benchmark data
- serde_derive 1.0.228 - Derive macros

**Parsing/Regex:**
- regex 1.12.2 - Pattern matching (for output parsing)
- aho-corasick 1.1.4 - String search
- memchr 2.7.6 - Memory search

**Plotting/Visualization:**
- plotters 0.3.7 - Generating benchmark charts
- plotters-svg 0.3.7 - SVG output for plots

**Parallelism:**
- rayon 1.11.0 - Data parallelism for Criterion analysis

**Utilities:**
- clap 4.5.54 - CLI argument parsing
- walkdir 2.5.0 - Directory traversal
- tinytemplate 1.2.1 - Template rendering for HTML reports
- once_cell 1.21.3 - Lazy static initialization

**WASM Support (for browser benchmarks):**
- wasm-bindgen 0.2.108 - Rust-WASM boundary bindings
- js-sys 0.3.85 - JavaScript sys bindings
- web-sys 0.3.85 - Web API bindings

**These are dev-only and not included in production deployments.**

---

*Integration audit: 2026-01-23*
