# Roadmap: Spark Signals (Rust)

## Overview

Port the TypeScript @rlabs-inc/signals library to Rust, solving three hard problems (type erasure, circular dependencies, borrow rules) incrementally across phases. Each phase proves a hard problem before building on it. Starting with core foundation, progressing through basic reactivity, then adding derived, effects, and finally advanced primitives and collections.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3...): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

- [ ] **Phase 1: Core Foundation** - Flags, types, context (proves type erasure infrastructure)
- [ ] **Phase 2: Basic Reactivity** - Signal + get/set (proves type erasure end-to-end)
- [ ] **Phase 3: Dependency Tracking** - markReactions, graph traversal (proves borrow scoping)
- [ ] **Phase 4: Derived** - MAYBE_DIRTY optimization (proves circular dep injection)
- [ ] **Phase 5: Effects & Scheduling** - Complete reactive loop
- [ ] **Phase 6: Batching & Utilities** - batch(), untrack(), flushSync()
- [ ] **Phase 7: Bindings & Linked Signals** - Two-way bindings, external sync
- [ ] **Phase 8: Scopes & Slots** - Effect scopes, storage primitives
- [ ] **Phase 9: Deep Reactivity** - Proxy, nested objects, arrays
- [ ] **Phase 10: Collections** - ReactiveMap, ReactiveSet, ReactiveVec
- [ ] **Phase 11: Advanced Primitives** - Selectors, tracked slots, reactive props
- [ ] **Phase 12: API Polish** - Two API surfaces, Rust ergonomics, final integration

## Phase Details

### Phase 1: Core Foundation
**Goal**: Establish type system foundation that enables heterogeneous signal storage
**Depends on**: Nothing (first phase)
**Requirements**: CNST-01, CNST-02, CNST-03, CNST-04, RUST-05
**Success Criteria** (what must be TRUE):
  1. Type flags (DERIVED, EFFECT, etc.) and status flags (CLEAN, DIRTY, MAYBE_DIRTY) are defined as constants
  2. AnySource and AnyReaction traits compile and can be implemented by concrete types
  3. Thread-local ReactiveContext exists and can be accessed via with_context pattern
  4. A Vec<Rc<dyn AnySource>> can hold signals of different T types
**Plans**: TBD

Plans:
- [ ] 01-01: Core constants and flags
- [ ] 01-02: Type-erased traits (AnySource, AnyReaction)
- [ ] 01-03: Thread-local context

### Phase 2: Basic Reactivity
**Goal**: Working signal primitive with read/write and type-erased storage
**Depends on**: Phase 1
**Requirements**: PRIM-01, PRIM-02, PRIM-06, LOWL-01, LOWL-02, RUST-06, RUST-07, RUST-08
**Success Criteria** (what must be TRUE):
  1. User can create signal with `signal(value)`, read with `.get()`, write with `.set()`
  2. Signal<i32> and Signal<String> can be stored in same Vec<Rc<dyn AnySource>>
  3. `.try_get()`, `.with(f)`, `.update(f)` combinators work
  4. Equality checking prevents write when value unchanged
**Plans**: TBD

Plans:
- [ ] 02-01: SourceInner<T> implementing AnySource
- [ ] 02-02: Signal<T> public API (get, set, try_get, with, update)
- [ ] 02-03: Basic get/set in tracking module

### Phase 3: Dependency Tracking
**Goal**: Reactive graph with dependency registration and dirty propagation
**Depends on**: Phase 2
**Requirements**: LOWL-03, LOWL-04, LOWL-05, LOWL-06, LOWL-07, LOWL-08
**Success Criteria** (what must be TRUE):
  1. Reading a signal inside a reaction registers the signal as a dependency
  2. Writing to a signal marks all dependent reactions as DIRTY
  3. `isDirty(reaction)` correctly reports dirty state
  4. `removeReactions(reaction, start)` cleans up old dependencies
  5. No RefCell borrow panics during cascade updates (borrow scoping proven)
**Plans**: TBD

Plans:
- [ ] 03-01: Dependency registration in get()
- [ ] 03-02: markReactions with proper borrow scoping
- [ ] 03-03: Graph cleanup utilities

### Phase 4: Derived
**Goal**: Lazy computed signals with MAYBE_DIRTY optimization
**Depends on**: Phase 3
**Requirements**: DERV-01, DERV-02, DERV-03, DERV-04, DERV-05
**Success Criteria** (what must be TRUE):
  1. User can create derived with `derived(|| computation)`
  2. Derived caches value, only recomputes when dependencies change
  3. MAYBE_DIRTY optimization prevents unnecessary recomputation in chains
  4. Diamond dependency patterns work correctly (A->B, A->C, B->D, C->D)
  5. Circular dependency injection works (tracking calls derived update)
**Plans**: TBD

Plans:
- [ ] 04-01: DerivedInner implementing both AnySource and AnyReaction
- [ ] 04-02: Derived<T> public API
- [ ] 04-03: MAYBE_DIRTY propagation and version checking
- [ ] 04-04: Dependency injection setup for updateDerived

### Phase 5: Effects & Scheduling
**Goal**: Side effects with automatic dependency tracking and scheduling
**Depends on**: Phase 4
**Requirements**: EFCT-01, EFCT-02, EFCT-03, EFCT-04, EFCT-05, EFCT-06, EFCT-07, EFCT-08, EFCT-09, RUST-01, RUST-04
**Success Criteria** (what must be TRUE):
  1. User can create effect with `effect(|| side_effect)` that runs on dependency change
  2. Effects support cleanup functions (returned value is called before re-run)
  3. `effect.sync()` runs immediately without scheduling
  4. `effect.root()` creates unparented effect
  5. Drop-based cleanup disposes effects automatically (RAII)
  6. Infinite loop detection prevents self-invalidating effects
**Plans**: TBD

Plans:
- [ ] 05-01: EffectInner implementing AnyReaction
- [ ] 05-02: Effect scheduling queue and flush
- [ ] 05-03: Effect public API (effect, effect.sync, effect.root, effect.tracking)
- [ ] 05-04: Cleanup function support
- [ ] 05-05: RAII disposal

### Phase 6: Batching & Utilities
**Goal**: Batch updates, untrack reads, synchronous flush
**Depends on**: Phase 5
**Requirements**: UTIL-01, UTIL-02, UTIL-03, UTIL-04, UTIL-05
**Success Criteria** (what must be TRUE):
  1. `batch(|| { multiple writes })` only triggers effects once
  2. `untrack(|| signal.get())` reads without creating dependency
  3. `peek(signal)` is shorthand for untrack read
  4. `flushSync()` immediately runs all pending effects
  5. `tick()` awaits next update cycle
**Plans**: TBD

Plans:
- [ ] 06-01: Batch depth tracking and deferred flush
- [ ] 06-02: Untrack flag and peek helper
- [ ] 06-03: flushSync and tick

### Phase 7: Bindings & Linked Signals
**Goal**: Two-way bindings and externally-synced signals
**Depends on**: Phase 6
**Requirements**: BIND-01, BIND-02, BIND-03, BIND-04, BIND-05, BIND-06, LINK-01, LINK-02, LINK-03
**Success Criteria** (what must be TRUE):
  1. `bind(signal)` creates two-way binding that syncs in both directions
  2. `bindReadonly(signal)` creates one-way binding
  3. `isBinding()`, `unwrap()`, `signals()` utilities work
  4. `linkedSignal(options)` creates signal that syncs with external source
  5. Bindings can be disconnected from graph
**Plans**: TBD

Plans:
- [ ] 07-01: Binding types and bind/bindReadonly
- [ ] 07-02: Binding utilities (isBinding, unwrap, signals, disconnect)
- [ ] 07-03: LinkedSignal implementation

### Phase 8: Scopes & Slots
**Goal**: Effect lifecycle grouping and typed storage primitives
**Depends on**: Phase 7
**Requirements**: SCOP-01, SCOP-02, SCOP-03, SCOP-04, SLOT-01, SLOT-02, SLOT-03, SLOT-04
**Success Criteria** (what must be TRUE):
  1. `effectScope(fn)` groups effects for collective disposal
  2. `getCurrentScope()` returns active scope
  3. `onScopeDispose(fn)` registers cleanup callback
  4. Disposing scope disposes all contained effects
  5. `slot<T>()` creates typed storage slot
  6. `slotArray<T>()` creates growable slot array
**Plans**: TBD

Plans:
- [ ] 08-01: EffectScope implementation
- [ ] 08-02: Scope utilities (getCurrentScope, onScopeDispose)
- [ ] 08-03: Slot and SlotArray primitives

### Phase 9: Deep Reactivity
**Goal**: Recursive reactive proxies for nested objects and arrays
**Depends on**: Phase 8
**Requirements**: DEEP-01, DEEP-02, DEEP-03, DEEP-04, DEEP-05, PRIM-04, PRIM-05
**Success Criteria** (what must be TRUE):
  1. `proxy(object)` creates recursively reactive proxy
  2. `toRaw(proxy)` returns original un-proxied object
  3. `isReactive(value)` detects reactive proxies
  4. Nested objects become reactive automatically
  5. Arrays are fully reactive (push, pop, splice, etc.)
  6. `state(value)` creates deep reactive signal using proxy
**Plans**: TBD

Plans:
- [ ] 09-01: Proxy infrastructure (trait-based delegation)
- [ ] 09-02: Nested object reactivity
- [ ] 09-03: Array reactivity
- [ ] 09-04: state() and stateRaw() primitives

### Phase 10: Collections
**Goal**: Reactive Map, Set, Vec, and Date wrappers
**Depends on**: Phase 9
**Requirements**: COLL-01, COLL-02, COLL-03, COLL-04
**Success Criteria** (what must be TRUE):
  1. `ReactiveMap<K,V>` has reactive get/set/delete/has operations
  2. `ReactiveSet<T>` has reactive add/delete/has operations
  3. `ReactiveDate` wrapper has reactive getters/setters
  4. `ReactiveVec<T>` (Rust addition) has reactive push/pop/insert/remove
**Plans**: TBD

Plans:
- [ ] 10-01: ReactiveMap implementation
- [ ] 10-02: ReactiveSet implementation
- [ ] 10-03: ReactiveVec implementation
- [ ] 10-04: ReactiveDate wrapper

### Phase 11: Advanced Primitives
**Goal**: Selectors, tracked slot arrays, reactive props
**Depends on**: Phase 10
**Requirements**: SELC-01, SELC-02, TSLOT-01, TSLOT-02, TSLOT-03, TSLOT-04, PROP-01, PROP-02, PRIM-03, PRIM-07
**Success Criteria** (what must be TRUE):
  1. `createSelector(source, fn)` creates optimized key-based selector
  2. `trackedSlotArray<T>()` tracks changes per-index (fine-grained)
  3. Tracked slot array supports efficient iteration with reactive tracking
  4. "Father state pattern" (parallel arrays) is supported
  5. `reactiveProps(props)` creates reactive object with independent properties
  6. `mutableSource(value)` creates mutable source variant
  7. Signals support custom equality functions
**Plans**: TBD

Plans:
- [ ] 11-01: createSelector implementation
- [ ] 11-02: TrackedSlotArray for ECS/game patterns
- [ ] 11-03: reactiveProps implementation
- [ ] 11-04: mutableSource and custom equality

### Phase 12: API Polish
**Goal**: Finalize both API surfaces, Rust ergonomics, comprehensive tests
**Depends on**: Phase 11
**Requirements**: API-01, API-02, API-03, RUST-02, RUST-03, EQLS-01, EQLS-02, EQLS-03, EQLS-04, EQLS-05, EQLS-06, EQLS-07, EQLS-08
**Success Criteria** (what must be TRUE):
  1. TypeScript-like API is primary surface: `signal()`, `derived()`, `effect()`
  2. Rust-idiomatic API is secondary: `Signal::new()`, `Derived::new()`
  3. Both APIs access same underlying implementation
  4. All public types implement Clone and Debug
  5. Full equality function library: equals, deepEquals, safeEquals, shallowEquals, etc.
  6. Benchmarks pass (from benches/signals.rs)
**Plans**: TBD

Plans:
- [ ] 12-01: TypeScript-like API module (re-exports and ergonomic wrappers)
- [ ] 12-02: Rust-idiomatic API module
- [ ] 12-03: Equality function library
- [ ] 12-04: Clone/Debug implementations
- [ ] 12-05: Final integration tests and benchmark validation

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10 -> 11 -> 12

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Foundation | 0/3 | Not started | - |
| 2. Basic Reactivity | 0/3 | Not started | - |
| 3. Dependency Tracking | 0/3 | Not started | - |
| 4. Derived | 0/4 | Not started | - |
| 5. Effects & Scheduling | 0/5 | Not started | - |
| 6. Batching & Utilities | 0/3 | Not started | - |
| 7. Bindings & Linked Signals | 0/3 | Not started | - |
| 8. Scopes & Slots | 0/3 | Not started | - |
| 9. Deep Reactivity | 0/4 | Not started | - |
| 10. Collections | 0/4 | Not started | - |
| 11. Advanced Primitives | 0/4 | Not started | - |
| 12. API Polish | 0/5 | Not started | - |

---
*Roadmap created: 2026-01-23*
*Phases: 12 | Depth: comprehensive | Coverage: 91/91 requirements*
