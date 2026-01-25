# Spark Signals - Progress

## Current Status: Phase 9 Complete ✓

### Phase 1: Core Foundation - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ Type flags and status flags defined as constants
2. ✓ AnySource and AnyReaction traits compile and can be implemented
3. ✓ Thread-local ReactiveContext with `with_context` pattern
4. ✓ `Vec<Rc<dyn AnySource>>` can hold signals of different T types

**What was built:**
- `core/constants.rs` - 19 flag constants
- `core/types.rs` - AnySource trait, SourceInner<T>
- `core/context.rs` - Thread-local ReactiveContext

---

### Phase 2: Basic Reactivity - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ User can create signal with `signal(value)`, read with `.get()`, write with `.set()`
2. ✓ Signal<i32> and Signal<String> can be stored in same Vec<Rc<dyn AnySource>>
3. ✓ `.try_get()`, `.with(f)`, `.update(f)` combinators work
4. ✓ Equality checking prevents write when value unchanged

**What was built:**
- `primitives/signal.rs` - Signal<T> wrapper, signal() function, combinators
- `primitives/mod.rs` - Module re-exports

---

### Phase 3: Dependency Tracking - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ Reading a signal inside a reaction registers the signal as a dependency
2. ✓ Writing to a signal marks all dependent reactions as DIRTY
3. ✓ `isDirty(reaction)` correctly reports dirty state
4. ✓ `removeReactions(reaction, start)` cleans up old dependencies
5. ✓ **No RefCell borrow panics during cascade updates** (borrow scoping proven!)

**What was built:**
- `reactivity/tracking.rs` - Core tracking functions:
  - `track_read(source)` - Dependency registration with version-based deduplication
  - `notify_write(source)` - Triggers markReactions after value change
  - `mark_reactions(source, status)` - Iterative dirty propagation (collect-then-mutate pattern)
  - `is_dirty(reaction)` - Dirty state checking
  - `remove_reactions(reaction, start)` - Dependency cleanup
  - `install_dependencies(reaction, skipped)` - Wires up deps after reaction execution
- `reactivity/mod.rs` - Module re-exports

**Key Pattern Proven:**
The "collect-then-mutate" pattern for borrow safety.

---

### Phase 4: Derived - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ User can create derived with `derived(|| computation)`
2. ✓ Derived caches value, only recomputes when dependencies change
3. ✓ MAYBE_DIRTY optimization prevents unnecessary recomputation in chains
4. ✓ Diamond dependency patterns work correctly (A->B, A->C, B->D, C->D)
5. ✓ Cascade propagation works via as_derived_source() / as_derived_reaction()

**What was built:**
- `primitives/derived.rs`:
  - `DerivedInner<T>` - Implements BOTH AnySource AND AnyReaction
  - `Derived<T>` - Public wrapper with `.get()` method
  - `derived()` - Public constructor function
  - `update_derived_chain()` - Iterative MAYBE_DIRTY optimization algorithm
  - Self-referential pattern for `as_derived_source()` / `as_derived_reaction()`
- Updated `core/types.rs`:
  - Added `as_derived_reaction()` method to AnySource trait

**Key Patterns Proven:**
- Dual-trait implementation: DerivedInner implements both AnySource and AnyReaction
- Self-referential Rc: Store `Weak<Self>` to enable trait object conversion
- MAYBE_DIRTY chain: Walk deps, update from deepest to shallowest

**Tests:** 71 passing + 5 doctests

---

### Phase 5: Effects & Scheduling - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ User can create effect with `effect(|| side_effect)` that runs on dependency change
2. ✓ Effects support cleanup functions (returned value is called before re-run)
3. ✓ `effect_sync()` runs immediately without scheduling
4. ✓ `effect_root()` creates unparented effect
5. ✓ Drop-based cleanup disposes effects automatically (dispose function)
6. ✓ Infinite loop detection prevents self-invalidating effects (MAX_ITERATIONS = 1000)

**What was built:**
- `primitives/effect.rs`:
  - `EffectInner` - Implements AnyReaction with effect tree structure (parent/first/last/prev/next)
  - `Effect` - Public wrapper with disposal
  - `update_effect()` - Runs effect with dependency tracking and teardown
  - `destroy_effect()` - Cleanup including children and teardown
  - Public API: `effect()`, `effect_sync()`, `effect_root()`, `effect_tracking()`
  - `effect_with_cleanup()`, `effect_sync_with_cleanup()` - With cleanup support
- `reactivity/scheduling.rs`:
  - `schedule_effect()` - Queue and flush effects
  - `flush_pending_effects()` - Process pending effects with loop detection
  - `flush_sync()` - Synchronous flush
- Updated `reactivity/tracking.rs`:
  - `mark_reactions()` now schedules effects when marking them dirty

**Key Design Decisions:**
- Rust has no microtasks, so all effects flush synchronously unless batched
- Effect tree uses Weak refs to avoid cycles (parent, last_child, prev_sibling)
- Dependencies installed AFTER effect function runs (same as TypeScript)
- Infinite loop detection at 1000 iterations

**Tests:** 94 passing + 5 doctests

---

### Phase 6: Batching & Utilities - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ `batch(|| { multiple writes })` only triggers effects once
2. ✓ `untrack(|| signal.get())` reads without creating dependency
3. ✓ `peek(signal)` is shorthand for untrack read
4. ✓ `flushSync()` immediately runs all pending effects
5. ✓ `tick()` awaits next update cycle

**What was built:**
- `reactivity/batching.rs`:
  - `batch()` - Groups updates, only flushes effects once on exit
  - `untrack()` - Read signals without creating dependencies
  - `peek()` - Alias for untrack
  - `tick()` - Ensure all effects have flushed
  - Guard pattern for panic safety in batch/untrack
- Already had: `flush_sync()` in scheduling.rs

**Tests:** 110 passing + 10 doctests

---

### Phase 7: Bindings & Linked Signals - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ `bind(signal)` creates two-way binding that syncs in both directions
2. ✓ `bindReadonly(signal)` creates one-way binding
3. ✓ `isBinding()`, `unwrap()` utilities work (signals() deferred to Phase 12)
4. ✓ `linkedSignal(options)` creates signal that syncs with external source
5. ✓ Bindings can be disconnected from graph

**What was built:**
- `primitives/bind.rs`:
  - `Binding<T>` - Writable two-way binding (Forward/Chain/Static variants)
  - `ReadonlyBinding<T>` - Read-only one-way binding
  - `bind()`, `bind_chain()`, `bind_value()`, `bind_static()` - Writable binding constructors
  - `bind_readonly()`, `bind_readonly_from()`, `bind_getter()`, `bind_readonly_static()` - Read-only constructors
  - `disconnect_binding()`, `disconnect_source()` - Manual graph cleanup
  - `is_binding()`, `unwrap_binding()`, `unwrap_readonly()` - Utilities
- `primitives/linked.rs`:
  - `LinkedSignal<T>` - Angular-style linked signal (resets on source change, can be manually overridden)
  - `linked_signal()` - Simple form (getter function)
  - `linked_signal_full()` - Full form with separate source/computation and previous value context
  - `is_linked_signal()` - Type check
- Bug fix in `effect_sync_with_cleanup()` - Was missing EFFECT flag, causing sync effects not to schedule

**Key Design Decisions:**
- Binding uses enum variants for different source types (Forward/Chain/Static)
- LinkedSignal uses effect_sync + derived for source tracking
- Previous value for computation reads actual current value (handles manual overrides)
- RAII for disposal (Rc<dyn Fn()> for dispose functions)

**Tests:** 137 passing + 22 doctests

---

### Phase 8: Scopes & Slots - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ `effectScope(fn)` groups effects for collective disposal
2. ✓ `getCurrentScope()` returns active scope
3. ✓ `onScopeDispose(fn)` registers cleanup callback
4. ✓ Disposing scope disposes all contained effects
5. ✓ `slot<T>()` creates typed storage slot
6. ✓ `slotArray<T>()` creates growable slot array

**What was built:**
- `primitives/scope.rs`:
  - `EffectScope` - Groups effects for batch disposal
  - `EffectScopeInner` - Internal with effects list, cleanups, children
  - `effect_scope(detached)` - Creates new scope
  - `get_current_scope()` - Returns active scope if any
  - `on_scope_dispose(fn)` - Registers cleanup callback
  - `register_effect_with_scope(effect)` - Called from effect creation
  - Pause/resume support (marks effects INERT)
  - Nested scopes with parent tracking
  - Detached scopes (opt out of parent collection)
- `primitives/slot.rs`:
  - `Slot<T>` - Reactive cell pointing to static/signal/getter
  - `SlotInner<T>` - Internal source tracking with SourceInner<Option<T>>
  - `slot(initial)`, `slot_with_value(value)` - Constructors
  - `Slot::get()` / `peek()` - Read with/without tracking
  - `Slot::set_value(v)` - Set static value
  - `Slot::set_signal(&sig)` - Point to signal (write-through)
  - `Slot::set_getter(fn)` - Point to getter (read-only)
  - `Slot::set(v)` - Write through to source
  - `SlotArray<T>` - Growable slot array with auto-expansion
  - `slot_array(default)` - Constructor
  - `SlotWriteError` - Error type for write failures
- Updated `primitives/effect.rs`:
  - Added `register_effect_with_scope()` call in `create_effect()`

**Key Design Decisions:**
- Scopes use thread-local active_scope (similar to active_reaction)
- Effects register with scope at creation time
- Slots use SourceInner<Option<T>> for optional values
- Slots track BOTH slot version AND underlying source
- SlotArray auto-expands on index access

**Tests:** 169 passing + 26 doctests

---

## Current Structure

```
src/
├── lib.rs              # Crate root with re-exports
├── core/
│   ├── mod.rs
│   ├── constants.rs    # All flags (19 constants)
│   ├── types.rs        # AnySource, AnyReaction traits, SourceInner<T>
│   └── context.rs      # Thread-local ReactiveContext
├── primitives/
│   ├── mod.rs
│   ├── signal.rs       # Signal<T>, signal(), combinators
│   ├── derived.rs      # Derived<T>, derived(), DerivedInner<T>
│   ├── effect.rs       # Effect, EffectInner, effect(), effect_sync(), effect_root()
│   ├── bind.rs         # Binding<T>, ReadonlyBinding<T>, bind(), bind_readonly()
│   ├── linked.rs       # LinkedSignal<T>, linked_signal(), linked_signal_full()
│   ├── scope.rs        # EffectScope, effect_scope(), on_scope_dispose()
│   └── slot.rs         # Slot<T>, SlotArray<T>, slot(), slot_array()
├── collections/
│   ├── mod.rs          # Module exports
│   ├── map.rs          # ReactiveMap<K, V> - per-key reactivity
│   ├── set.rs          # ReactiveSet<T> - per-item reactivity
│   └── vec.rs          # ReactiveVec<T> - per-index reactivity
└── reactivity/
    ├── mod.rs
    ├── tracking.rs     # track_read, notify_write, mark_reactions, disconnect_source
    ├── scheduling.rs   # schedule_effect, flush_pending_effects, flush_sync
    └── batching.rs     # batch, untrack, peek, tick
```

---

### Phase 9: Reactive Collections - COMPLETE ✓

**Success Criteria (all verified):**
1. ✓ `ReactiveMap<K, V>` with per-key tracking
2. ✓ `ReactiveSet<T>` with per-item tracking
3. ✓ `ReactiveVec<T>` with per-index tracking
4. ✓ Iteration triggers full collection dependency (version signal)
5. ✓ `.get()` / `.contains()` only trigger on specific key/item change

**What was built:**
- `collections/mod.rs` - Module exports
- `collections/map.rs`:
  - `ReactiveMap<K, V>` - HashMap with per-key reactivity
  - Three signals: key_signals (per-key), version (structural), size
  - Methods: get, get_tracked, contains_key, insert, remove, clear
  - Iteration methods track version signal
- `collections/set.rs`:
  - `ReactiveSet<T>` - HashSet with per-item reactivity
  - Three signals: item_signals (per-item), version (structural), size
  - Methods: contains, contains_tracked, insert, remove, clear
  - Set operations: is_subset, is_superset, is_disjoint
- `collections/vec.rs`:
  - `ReactiveVec<T>` - Vec with per-index reactivity (Rust-specific)
  - Three signals: index_signals (per-index), version (structural), length
  - Methods: get, get_tracked, set, push, pop, insert, remove, swap_remove
  - Utility: truncate, retain, extend, append, reverse, sort

**Key Design Decisions:**
- Use composition not inheritance (Rust can't extend HashMap/HashSet like TypeScript)
- Per-key/item/index signals for fine-grained tracking
- Version signal for structural changes (add/remove)
- Size/length signal for count changes
- `batch()` needed when mutating inside effects (RefCell re-entrancy)

**Tests:** 211 passing + 29 doctests

---

## Next: Phase 10 - Deep Reactivity (Optional)

**Goal:** Recursive reactive proxies for nested objects and arrays

**Note:** This phase may be skipped - Rust's ownership model makes JS-style proxies impractical. The collections we built provide most of the needed functionality.

**Success Criteria (if implemented):**
1. `proxy(object)` creates recursively reactive proxy
2. `toRaw(proxy)` returns original un-proxied object
3. `isReactive(value)` detects reactive proxies

**Alternative:** Phase 11 (Advanced Primitives) or Phase 12 (API Polish) may be more valuable

---

## History

### Previous Attempts (Before GSD)

Each attempt got better at *hiding* incompleteness rather than *solving* it:
1. Explicit TODOs everywhere
2. "Simplified for now" comments
3. Placeholder functions
4. Nice documentation masking hollow code

### The Breakthrough

Using structured planning with:
- Clear success criteria that can't be faked
- Tests that prove criteria are met
- Phase-by-phase progression that builds on proven foundations
- **Phase 3: Proved the borrow rules work** - the critical milestone
- **Phase 4: Proved the dual-trait pattern works** - deriveds are the heart of reactivity
- **Phase 7: Effect flag bug found and fixed** - sync effects weren't being scheduled

---

## Reference: TypeScript Source

```
/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals
```

Key files:
- `src/core/types.ts` - The interfaces
- `src/core/constants.ts` - The flags
- `src/core/globals.ts` - Thread-local state
- `src/reactivity/tracking.ts` - THE CORE ← **PORTED!**
- `src/primitives/derived.ts` - Derived ← **PORTED!**
- `src/primitives/effect.ts` - Effect ← **PORTED!**
- `src/primitives/bind.ts` - Bindings ← **PORTED!**
- `src/primitives/linked.ts` - LinkedSignal ← **PORTED!**
- `src/primitives/scope.ts` - EffectScope ← **PORTED!**
- `src/primitives/slot.ts` - Slot & SlotArray ← **PORTED!**
- `src/collections/map.ts` - ReactiveMap ← **PORTED!**
- `src/collections/set.ts` - ReactiveSet ← **PORTED!**
- (no vec.ts - ReactiveVec is Rust-specific)

---
*Last updated: 2026-01-24 after Phase 9 completion*
