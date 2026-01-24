# Requirements: Spark Signals (Rust)

**Defined:** 2026-01-23
**Core Value:** Complete feature parity with TypeScript @rlabs-inc/signals, with Rust-idiomatic secondary API

## v1 Requirements

Requirements for initial release. Every TypeScript feature, plus Rust additions.

### Core Primitives

- [ ] **PRIM-01**: `signal<T>(value)` creates writable reactive signal
- [ ] **PRIM-02**: `source<T>(value, options)` creates internal source (low-level)
- [ ] **PRIM-03**: `mutableSource<T>(value)` creates mutable source variant
- [ ] **PRIM-04**: `state<T>(value)` creates deep reactive signal (uses proxy)
- [ ] **PRIM-05**: `stateRaw<T>()` creates raw state accessor
- [ ] **PRIM-06**: Signal equality checking prevents unnecessary updates
- [ ] **PRIM-07**: Signal supports custom equality functions

### Derived Signals

- [ ] **DERV-01**: `derived(fn)` creates lazy computed signal
- [ ] **DERV-02**: Derived caches value until dependencies change
- [ ] **DERV-03**: MAYBE_DIRTY optimization prevents unnecessary recomputation
- [ ] **DERV-04**: `createDerived(fn, options)` low-level API
- [ ] **DERV-05**: `disconnectDerived(d)` removes from dependency graph

### Effects

- [ ] **EFCT-01**: `effect(fn)` creates side effect with dependency tracking
- [ ] **EFCT-02**: Effects re-run when dependencies change
- [ ] **EFCT-03**: Effects support cleanup functions (return value)
- [ ] **EFCT-04**: `effect.sync(fn)` runs effect synchronously (no scheduling)
- [ ] **EFCT-05**: `effect.root(fn)` creates root effect without parent
- [ ] **EFCT-06**: `effect.tracking()` returns true if currently tracking
- [ ] **EFCT-07**: `createEffect(fn)` low-level API
- [ ] **EFCT-08**: `updateEffect(e)` manually triggers effect update
- [ ] **EFCT-09**: `destroyEffect(e)` removes effect from graph

### Bindings

- [ ] **BIND-01**: `bind(signal)` creates two-way binding
- [ ] **BIND-02**: `bindReadonly(signal)` creates one-way (read) binding
- [ ] **BIND-03**: `isBinding(value)` type guard for bindings
- [ ] **BIND-04**: `unwrap(binding)` gets underlying signal
- [ ] **BIND-05**: `signals(binding)` returns all signals in binding
- [ ] **BIND-06**: `disconnectBinding(b)` removes binding from graph

### Linked Signals

- [ ] **LINK-01**: `linkedSignal(options)` creates signal with external sync
- [ ] **LINK-02**: `isLinkedSignal(value)` type guard
- [ ] **LINK-03**: Linked signals support source/equal options

### Selectors

- [ ] **SELC-01**: `createSelector(source, fn)` creates optimized selector
- [ ] **SELC-02**: Selector returns function that tracks specific key

### Effect Scopes

- [ ] **SCOP-01**: `effectScope(fn)` groups effects for lifecycle management
- [ ] **SCOP-02**: `getCurrentScope()` returns active scope
- [ ] **SCOP-03**: `onScopeDispose(fn)` registers cleanup callback
- [ ] **SCOP-04**: Disposing scope disposes all contained effects

### Slots (Storage Primitives)

- [ ] **SLOT-01**: `slot<T>()` creates typed storage slot
- [ ] **SLOT-02**: `slotArray<T>()` creates growable slot array
- [ ] **SLOT-03**: `isSlot(value)` type guard
- [ ] **SLOT-04**: `hasSlot(entity, slot)` checks if entity has slot value

### TrackedSlotArray (ECS/Game Primitive)

- [ ] **TSLOT-01**: `trackedSlotArray<T>()` creates reactive slot array
- [ ] **TSLOT-02**: Array tracks changes per-index (fine-grained)
- [ ] **TSLOT-03**: Efficient iteration with reactive tracking
- [ ] **TSLOT-04**: Supports the "father state pattern" (parallel arrays)

### Reactive Props

- [ ] **PROP-01**: `reactiveProps(props)` creates reactive object from props
- [ ] **PROP-02**: Each property becomes independently trackable

### Deep Reactivity

- [ ] **DEEP-01**: `proxy(object)` creates recursively reactive proxy
- [ ] **DEEP-02**: `toRaw(proxy)` returns original un-proxied object
- [ ] **DEEP-03**: `isReactive(value)` checks if value is reactive proxy
- [ ] **DEEP-04**: Nested objects become reactive automatically
- [ ] **DEEP-05**: Arrays are fully reactive (push, pop, splice, etc.)

### Batching & Utilities

- [ ] **UTIL-01**: `batch(fn)` batches multiple writes into single notification
- [ ] **UTIL-02**: `untrack(fn)` reads signals without creating dependencies
- [ ] **UTIL-03**: `peek(signal)` reads value without tracking (shorthand)
- [ ] **UTIL-04**: `flushSync(fn?)` immediately runs all pending effects
- [ ] **UTIL-05**: `tick()` waits for next update cycle

### Equality Functions

- [ ] **EQLS-01**: `equals(a, b)` default equality (Object.is)
- [ ] **EQLS-02**: `deepEquals(a, b)` structural deep equality
- [ ] **EQLS-03**: `safeEquals(a, b)` handles NaN and object references
- [ ] **EQLS-04**: `safeNotEqual(a, b)` inverse of safeEquals
- [ ] **EQLS-05**: `shallowEquals(a, b)` shallow object comparison
- [ ] **EQLS-06**: `createEquals(fn)` wraps custom equality function
- [ ] **EQLS-07**: `neverEquals()` always returns false (for fn-valued signals)
- [ ] **EQLS-08**: `alwaysEquals()` always returns true (never update)

### Reactive Collections

- [ ] **COLL-01**: `ReactiveMap<K, V>` with reactive get/set/delete/has
- [ ] **COLL-02**: `ReactiveSet<T>` with reactive add/delete/has
- [ ] **COLL-03**: `ReactiveDate` wrapper with reactive getters/setters
- [ ] **COLL-04**: `ReactiveVec<T>` (Rust addition) with push/pop/insert/remove

### Low-Level Tracking API

- [ ] **LOWL-01**: `get(source)` reads value with tracking
- [ ] **LOWL-02**: `set(source, value)` writes value with notification
- [ ] **LOWL-03**: `isDirty(reaction)` checks if reaction needs update
- [ ] **LOWL-04**: `setSignalStatus(signal, status)` sets flag state
- [ ] **LOWL-05**: `markReactions(source, status)` propagates dirty flags
- [ ] **LOWL-06**: `updateReaction(reaction)` runs reaction update
- [ ] **LOWL-07**: `removeReactions(reaction, start)` cleans up deps
- [ ] **LOWL-08**: `disconnectSource(source)` removes from graph

### Constants

- [ ] **CNST-01**: Type flags: DERIVED, EFFECT, RENDER_EFFECT, ROOT_EFFECT, etc.
- [ ] **CNST-02**: Status flags: CLEAN, DIRTY, MAYBE_DIRTY, DESTROYED, etc.
- [ ] **CNST-03**: Sentinel values: UNINITIALIZED, STALE_REACTION, etc.
- [ ] **CNST-04**: Symbols: STATE_SYMBOL, BINDING_SYMBOL, SLOT_SYMBOL, etc.

### Rust Table Stakes

- [ ] **RUST-01**: Drop-based cleanup (RAII) instead of manual dispose
- [ ] **RUST-02**: Clone trait implementations where applicable
- [ ] **RUST-03**: Debug trait implementations for all public types
- [ ] **RUST-04**: `#[must_use]` attributes on effect/scope returns
- [ ] **RUST-05**: `Send + Sync` documentation (explicitly !Send, !Sync by default)
- [ ] **RUST-06**: `.try_get()` / `.try_set()` for Option-based access
- [ ] **RUST-07**: `.with(f)` combinator for borrowing without clone
- [ ] **RUST-08**: `.update(f)` combinator for in-place mutation

### Two API Surfaces

- [ ] **API-01**: TypeScript-like API as primary surface (signal(), derived(), etc.)
- [ ] **API-02**: Rust-idiomatic API as secondary (Signal::new(), Derived::new())
- [ ] **API-03**: Both APIs access same underlying implementation

## v2 Requirements

Deferred to future release.

### Thread Safety

- **SYNC-01**: `sync` feature flag for Send + Sync signals
- **SYNC-02**: Arc<RwLock> variants of all primitives
- **SYNC-03**: Thread-safe effect scheduling

### Performance Optimizations

- **PERF-01**: Arena allocation option (Leptos-style Copy signals)
- **PERF-02**: SIMD-accelerated batch operations (from Bun patterns)

### Async Integration

- **ASYNC-01**: `Resource<T>` for async data fetching
- **ASYNC-02**: Async effect support

## Out of Scope

| Feature | Reason |
|---------|--------|
| UI framework features | Not our domain - signals only |
| Routing | Framework feature |
| Server functions / RPC | Framework feature |
| JSX/template macros | Not a UI framework |
| Heavy proc macros | Prefer runtime simplicity |
| Implicit global runtime | Un-Rusty - use explicit context |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CNST-01 | Phase 1 | Pending |
| CNST-02 | Phase 1 | Pending |
| CNST-03 | Phase 1 | Pending |
| CNST-04 | Phase 1 | Pending |
| RUST-05 | Phase 1 | Pending |
| PRIM-01 | Phase 2 | Pending |
| PRIM-02 | Phase 2 | Pending |
| PRIM-06 | Phase 2 | Pending |
| LOWL-01 | Phase 2 | Pending |
| LOWL-02 | Phase 2 | Pending |
| RUST-06 | Phase 2 | Pending |
| RUST-07 | Phase 2 | Pending |
| RUST-08 | Phase 2 | Pending |
| LOWL-03 | Phase 3 | Pending |
| LOWL-04 | Phase 3 | Pending |
| LOWL-05 | Phase 3 | Pending |
| LOWL-06 | Phase 3 | Pending |
| LOWL-07 | Phase 3 | Pending |
| LOWL-08 | Phase 3 | Pending |
| DERV-01 | Phase 4 | Pending |
| DERV-02 | Phase 4 | Pending |
| DERV-03 | Phase 4 | Pending |
| DERV-04 | Phase 4 | Pending |
| DERV-05 | Phase 4 | Pending |
| EFCT-01 | Phase 5 | Pending |
| EFCT-02 | Phase 5 | Pending |
| EFCT-03 | Phase 5 | Pending |
| EFCT-04 | Phase 5 | Pending |
| EFCT-05 | Phase 5 | Pending |
| EFCT-06 | Phase 5 | Pending |
| EFCT-07 | Phase 5 | Pending |
| EFCT-08 | Phase 5 | Pending |
| EFCT-09 | Phase 5 | Pending |
| RUST-01 | Phase 5 | Pending |
| RUST-04 | Phase 5 | Pending |
| UTIL-01 | Phase 6 | Pending |
| UTIL-02 | Phase 6 | Pending |
| UTIL-03 | Phase 6 | Pending |
| UTIL-04 | Phase 6 | Pending |
| UTIL-05 | Phase 6 | Pending |
| BIND-01 | Phase 7 | Pending |
| BIND-02 | Phase 7 | Pending |
| BIND-03 | Phase 7 | Pending |
| BIND-04 | Phase 7 | Pending |
| BIND-05 | Phase 7 | Pending |
| BIND-06 | Phase 7 | Pending |
| LINK-01 | Phase 7 | Pending |
| LINK-02 | Phase 7 | Pending |
| LINK-03 | Phase 7 | Pending |
| SCOP-01 | Phase 8 | Pending |
| SCOP-02 | Phase 8 | Pending |
| SCOP-03 | Phase 8 | Pending |
| SCOP-04 | Phase 8 | Pending |
| SLOT-01 | Phase 8 | Pending |
| SLOT-02 | Phase 8 | Pending |
| SLOT-03 | Phase 8 | Pending |
| SLOT-04 | Phase 8 | Pending |
| DEEP-01 | Phase 9 | Pending |
| DEEP-02 | Phase 9 | Pending |
| DEEP-03 | Phase 9 | Pending |
| DEEP-04 | Phase 9 | Pending |
| DEEP-05 | Phase 9 | Pending |
| PRIM-04 | Phase 9 | Pending |
| PRIM-05 | Phase 9 | Pending |
| COLL-01 | Phase 10 | Pending |
| COLL-02 | Phase 10 | Pending |
| COLL-03 | Phase 10 | Pending |
| COLL-04 | Phase 10 | Pending |
| SELC-01 | Phase 11 | Pending |
| SELC-02 | Phase 11 | Pending |
| TSLOT-01 | Phase 11 | Pending |
| TSLOT-02 | Phase 11 | Pending |
| TSLOT-03 | Phase 11 | Pending |
| TSLOT-04 | Phase 11 | Pending |
| PROP-01 | Phase 11 | Pending |
| PROP-02 | Phase 11 | Pending |
| PRIM-03 | Phase 11 | Pending |
| PRIM-07 | Phase 11 | Pending |
| API-01 | Phase 12 | Pending |
| API-02 | Phase 12 | Pending |
| API-03 | Phase 12 | Pending |
| RUST-02 | Phase 12 | Pending |
| RUST-03 | Phase 12 | Pending |
| EQLS-01 | Phase 12 | Pending |
| EQLS-02 | Phase 12 | Pending |
| EQLS-03 | Phase 12 | Pending |
| EQLS-04 | Phase 12 | Pending |
| EQLS-05 | Phase 12 | Pending |
| EQLS-06 | Phase 12 | Pending |
| EQLS-07 | Phase 12 | Pending |
| EQLS-08 | Phase 12 | Pending |

**Coverage:**
- v1 requirements: 91 total
- Mapped to phases: 91
- Unmapped: 0

---
*Requirements defined: 2026-01-23*
*Last updated: 2026-01-23 after roadmap creation*
