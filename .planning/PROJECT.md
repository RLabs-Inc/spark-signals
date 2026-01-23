# Spark Signals (Rust)

## What This Is

A faithful Rust port of `@rlabs-inc/signals` - a fine-grained reactive signals library. The implementation should be so clean and direct that you don't even realize you're reading Rust. Two API surfaces: a TypeScript-like primary API for maximum ergonomics, and a secondary Rust-idiomatic API for developers who prefer native patterns. The simplest reactive API that ever existed.

## Core Value

**Complete feature parity with the TypeScript implementation.** Nothing simplified, nothing missing. Every primitive, every pattern, every optimization (especially MAYBE_DIRTY) must work exactly as the TypeScript version. Can add Rust-specific features, but never remove or simplify existing ones.

## Requirements

### Validated

(None yet — ship to validate)

### Active

**Core Primitives:**
- [ ] `signal<T>()` - Writable reactive values
- [ ] `derived()` - Lazy computed values (both Source AND Reaction)
- [ ] `effect()` - Side effects with automatic dependency tracking
- [ ] `effect.sync()` - Synchronous effects
- [ ] `effect.root()` - Root effects without parent
- [ ] `effect.tracking()` - Check if currently tracking
- [ ] `batch()` - Batch multiple updates
- [ ] `untrack()` - Read without tracking
- [ ] `peek()` - Read signal without tracking (shorthand)

**Advanced Primitives:**
- [ ] `bind()` / `bindReadonly()` - Two-way bindings
- [ ] `linkedSignal()` - Signals with external sync
- [ ] `createSelector()` - Optimized signal selector
- [ ] `effectScope()` - Effect grouping/lifecycle
- [ ] `slot()` / `slotArray()` - Storage primitives
- [ ] `reactiveProps()` - Reactive object properties

**Collections:**
- [ ] `ReactiveMap<K, V>` - Reactive Map
- [ ] `ReactiveSet<T>` - Reactive Set
- [ ] `ReactiveVec<T>` - Reactive Vec (Rust addition)

**Deep Reactivity:**
- [ ] `proxy()` - Recursive reactive proxies
- [ ] `toRaw()` - Get raw value from proxy
- [ ] `isReactive()` - Check if value is reactive

**API Ergonomics:**
- [ ] TypeScript-like API as primary surface (whatever it takes)
- [ ] Rust-idiomatic API as secondary surface (clean, simple)
- [ ] Both APIs access same underlying implementation

### Out of Scope

- Async runtime integration (focus on sync reactivity first) — can add later if needed
- WASM-specific optimizations — pure Rust first, WASM works automatically
- Multi-threaded `sync` feature — design for it, implement later

## Context

**Source Reference:**
The TypeScript implementation at `/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals` is the canonical spec. Port it faithfully.

**Previous Attempts:**
5 failed attempts, each getting better at *hiding* incompleteness:
1. Too much infrastructure before primitives worked
2. Left explicit TODOs everywhere
3. "Simplified for now" comments hiding incompleteness
4. Placeholder functions that looked complete but did nothing
5. Nice documentation describing what code "should do" while actual code was hollow

**Current State:**
Fresh start. `src/lib.rs` is empty stub. Benchmarks defined in `benches/signals.rs` showing expected API but won't compile until implementation exists.

**Three Hard Problems:**
These must be solved BEFORE implementation, not papered over:
1. **Type erasure** - Store `Source<T>` in heterogeneous collections (`Vec<dyn ReactiveNode>`)
2. **Circular dependencies** - tracking ↔ scheduling ↔ derived need each other
3. **Borrow rules in cascade** - `cascade_maybe_dirty()` needs mutable graph traversal

## Constraints

- **Rule Zero**: Write Rust like functional TypeScript. `Rc<RefCell<T>>`, `.clone()`, `.borrow()` - the Rust tax we accept. No lifetime annotations fighting the borrow checker.
- **Zero prod deps**: No production dependencies. Only `criterion` for benchmarks.
- **Edition**: Rust 2024
- **MSRV**: Rust 1.85+

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| `Rc<RefCell<T>>` over lifetimes | Match TypeScript ergonomics, avoid borrow checker complexity | — Pending |
| Two API surfaces | TypeScript-like for ergonomics, Rust-idiomatic for native feel | — Pending |
| GSD approach | 5 failed attempts prove structure and verification needed | — Pending |

---
*Last updated: 2026-01-23 after initialization*
