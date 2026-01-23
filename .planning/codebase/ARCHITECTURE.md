# Architecture

**Analysis Date:** 2026-01-23

## Pattern Overview

**Overall:** Fine-Grained Reactivity System (Svelte 5 / CompilerDOM Pattern)

**Key Characteristics:**
- **Signal-based reactivity**: Core primitive is the `Source<T>` (a reactive value holder with get/set)
- **Two-level tracking**: Dependency tracking at read-time via `activeReaction`, write-time triggering via `reactions` lists
- **Lazy computed values**: `Derived<T>` signals compute on-demand, cache results, only recompute when dependencies change
- **Side effects**: `Effect` functions re-run automatically when their dependencies change
- **MAYBE_DIRTY optimization**: Intermediate deriveds marked as MAYBE_DIRTY prevent unnecessary cascading updates
- **Flag-based state**: All state (DIRTY, CLEAN, MAYBE_DIRTY, DERIVED, EFFECT, etc.) encoded in bitmask flags for performance
- **No lifetime parameters**: Uses `Rc<RefCell<T>>` internally (via `.borrow()` / `.borrow_mut()`) to avoid Rust lifetime complexity
- **Type erasure ready**: Design supports storing heterogeneous `Source<T>` in collections via trait objects

## Layers

**Core Layer (`core/`):**
- Purpose: Type definitions and state constants
- Location: Mirrors TypeScript at `/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals/src/core/`
- Contains:
  - `types.ts`: `Signal`, `Source<T>`, `Reaction`, `Derived<T>`, `Effect` interfaces
  - `constants.ts`: Bitmask flags (DIRTY, CLEAN, MAYBE_DIRTY, DERIVED, EFFECT, etc.)
  - `globals.ts`: Thread-local state (activeReaction, activeEffect, readVersion, writeVersion, pendingReactions)
- Depends on: Nothing (foundational)
- Used by: Everything

**Reactivity Layer (`reactivity/`):**
- Purpose: The core engine - dependency tracking, scheduling, batching
- Location: Mirrors TypeScript implementation
- Contains:
  - `tracking.ts`: `get()` and `set()` functions with version-based deduplication and MAYBE_DIRTY logic
  - `scheduling.ts`: Effect scheduler, `flushSync()`, microtask-based batching
  - `batching.ts`: `batch()`, `untrack()`, `peek()` for controlling reactivity
  - `equality.ts`: Equality comparisons (default, deep, safe, shallow)
- Depends on: Core layer
- Used by: Primitives layer

**Primitives Layer (`primitives/`):**
- Purpose: User-facing API functions - signal creation, derived computation, effect registration
- Location: Mirrors TypeScript structure
- Contains:
  - `signal.ts`: `signal()` and `source()` - writable reactive values
  - `derived.ts`: `derived()` - lazy computed values (both Source AND Reaction)
  - `effect.ts`: `effect()` and `effect.sync()` - side effects
  - `bind.ts`: `bind()` - two-way bindings
  - `linked.ts`: `linkedSignal()` - signals with external sync
  - `selector.ts`: `createSelector()` - optimized signal selector
  - `scope.ts`: `effectScope()` - effect grouping/lifecycle
  - `slot.ts`, `tracked-slot.ts`: Storage primitives
  - `props.ts`: `reactiveProps()` - reactive object properties
- Depends on: Core and Reactivity layers
- Used by: Collections layer and user code

**Collections Layer (`collections/`):**
- Purpose: Reactive data structures
- Location: Mirrors TypeScript
- Contains:
  - `map.ts`: `ReactiveMap<K, V>` - reactive Map implementation
  - `set.ts`: `ReactiveSet<T>` - reactive Set implementation
  - `date.ts`: `ReactiveDate` - reactive Date wrapper
  - (Rust addition) `vec.ts`: Would add `ReactiveVec<T>` for Rust-style reactive vectors
- Depends on: Primitives layer (uses signals/effects internally)
- Used by: User code

**Deep Reactivity Layer (`deep/`):**
- Purpose: Recursive reactive proxies for objects
- Location: Mirrors TypeScript
- Contains:
  - `proxy.ts`: `proxy()`, `toRaw()`, `isReactive()` - recursive object/array tracking
- Depends on: Core, Reactivity, Primitives
- Used by: `signal()` for `.state()` variant

## Data Flow

**Reading a Value (get):**

1. User calls `signal.get()` or reads derived in a reaction
2. `get(source)` function in `tracking.rs` called
3. If `activeReaction` exists:
   - Version-based dedup: check if source already read this cycle (rv < readVersion)
   - If new dependency: add to `newDeps` array
   - Register reaction in source's `reactions` list
4. If source is `Derived` and DIRTY/MAYBE_DIRTY:
   - Call `updateDerivedChain()` to recompute it
5. Return source value

**Writing a Value (set):**

1. User calls `signal.set(newValue)`
2. Equality check: if `equals(oldValue, newValue)` → skip (no change)
3. Update value, increment `writeVersion`
4. Mark all `reactions` as DIRTY
5. If not in batch: schedule effect flushes
6. Dependent deriveds marked DIRTY, effects marked DIRTY
7. On next reaction run: cascading MAYBE_DIRTY optimization checks if intermediate values changed

**Effect Execution:**

1. Effect created or marked DIRTY
2. If not in batch: scheduled via `scheduleEffect()`
3. Scheduler runs via microtask (or sync if `effect.sync()`)
4. Active effect set to this effect, `readVersion` incremented
5. Effect function runs, reads signals (tracked as dependencies)
6. Dependencies stored, teardown stored
7. Next write to any dependency marks this effect DIRTY
8. On next flush: re-run if still DIRTY

**MAYBE_DIRTY Cascade:**

1. Signal A changes → Derived B (depends on A) marked **DIRTY**
2. Derived C (depends on B) marked **MAYBE_DIRTY** (expensive recomputation deferred)
3. Effect E (depends on C) marked **MAYBE_DIRTY**
4. When E scheduled to run:
   - Read C → it's MAYBE_DIRTY, triggers check
   - C reads B → it's DIRTY, recomputes B
   - If B value unchanged: C becomes CLEAN (skipped), E doesn't run
   - If B value changed: C recomputes, checks if C changed, etc.

**State Management:**

- Global mutable state in `globals.rs`: activeReaction, activeEffect, readVersion, writeVersion, pendingReactions
- Each Signal/Reaction has flags bitmask `f` and write version `wv`
- Source has `reactions` list (dependents) and `rv` (read version for dedup)
- Reaction has `deps` list (dependencies)
- Batch depth counter prevents effect flushing until batch complete

## Key Abstractions

**Signal (Base):**
- Purpose: Root reactive value container
- Examples: `Source<T>` in `signal.ts`, `Derived<T>` in `derived.ts`
- Pattern: Struct with flags `f`, value `v`, reactions list, metadata (wv, rv)

**Source<T>:**
- Purpose: Basic writable reactive value
- Examples: Created by `signal()`, represents mutable state
- Pattern: Stores value, equality function, reactions that depend on it

**Reaction (Base):**
- Purpose: Something that reads signals (effect or derived)
- Pattern: Has flags, function to execute, dependencies list, metadata

**Derived<T> (Dual Nature):**
- Purpose: Computed value that is BOTH a Source (readable) AND a Reaction (tracks dependencies)
- Examples: `derived()` in `derived.ts`
- Pattern: Extends both Source and Reaction, has computation function `fn`, caches result in `v`, tracks deps
- Critical: This dual nature enables MAYBE_DIRTY - dependents can treat it like a Source even when it's recomputing

**Effect:**
- Purpose: Side effect runner with cleanup
- Examples: `effect()` in `effect.ts`, has parent/child tree structure
- Pattern: Stores function, dependencies, teardown function, parent/sibling pointers, nested effect support

**WritableSignal<T> (Public API):**
- Purpose: User-facing wrapper around Source<T>
- Pattern: Proxy object with `value` property getter/setter that calls underlying `get()`/`set()`
- Storage: Uses Symbol to access internal Source from wrapper, FinalizationRegistry for cleanup

## Entry Points

**`lib.rs`:**
- Location: `/Users/rusty/Documents/Projects/TUI/tui-rust/crates/spark-signals/src/lib.rs`
- Triggers: Crate import
- Responsibilities: Currently minimal - intentionally left empty for GSD-driven implementation

**Benchmark Entry:**
- Location: `/Users/rusty/Documents/Projects/TUI/tui-rust/crates/spark-signals/benches/signals.rs`
- Triggers: `cargo bench` command
- Responsibilities: Test signal creation, get/set, derived chains, effects, batching, stress tests
- Demonstrates: Expected API shape (signal.get(), signal.set(), derived, effect, batch)

**Public API Functions (to be implemented):**
- `signal<T>(value: T) -> WritableSignal<T>` - Create writable signal
- `derived<T>(f: Fn() -> T) -> Signal<T>` - Create computed value
- `effect(f: Fn())` - Create side effect
- `batch(f: Fn())` - Batch multiple writes
- `untrack(f: Fn()) -> T` - Read signals without tracking
- `peek(s: Source) -> T` - Read without tracking in one call

## Error Handling

**Strategy:** Panic-free design with graceful fallbacks

**Patterns:**
- Equality functions return booleans (never panic)
- Circular dependency detection via flags (no unbounded recursion)
- No `.unwrap()` in core tracking logic (use Option, Result carefully)
- User function panics propagate (effects can fail, cleanup still runs)
- Batching depth overflow: unlikely (usize), but guards against infinite nesting

## Cross-Cutting Concerns

**Logging:** Not implemented - use `println!` macros in debug, strip in release

**Validation:**
- Type checking at compile time (Rust generics)
- Flag validation: check for mutually exclusive states (DIRTY+CLEAN, etc.)
- Equality function validation: assume correct, document expectations

**Authentication:** Not applicable - library doesn't handle auth

**Memory Management:**
- `Rc<RefCell<T>>` for interior mutability without lifetime parameters
- `.clone()` to share ownership
- FinalizationRegistry equivalent (Rust drop) for cleanup
- Effects/derived created with explicit cleanup paths

---

*Architecture analysis: 2026-01-23*
