# Spark Signals Rust - Architecture Document

## TypeScript Source Analysis

The TypeScript implementation at `/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals` is a complete reactive signals library based on Svelte 5's reactivity system. This document captures everything needed to port it to Rust.

---

## Complete API Surface

### Core Primitives

| Function | Description | Returns |
|----------|-------------|---------|
| `signal<T>(initial, options?)` | Create writable signal | `WritableSignal<T>` |
| `source<T>(initial, options?)` | Low-level signal (internal) | `Source<T>` |
| `mutableSource<T>(initial)` | Signal with safe equality (NaN handling) | `Source<T>` |
| `derived<T>(fn, options?)` | Lazy computed value | `DerivedSignal<T>` |
| `derived.by<T>(fn, options?)` | Alias for derived() | `DerivedSignal<T>` |
| `effect(fn)` | Async effect (microtask batched) | `DisposeFn` |
| `effect.sync(fn)` | Synchronous effect | `DisposeFn` |
| `effect.pre(fn)` | Deprecated alias for effect.sync | `DisposeFn` |
| `effect.root(fn)` | Root effect scope | `DisposeFn` |
| `effect.tracking()` | Check if inside reactive context | `boolean` |

### Advanced Primitives

| Function | Description | Returns |
|----------|-------------|---------|
| `bind<T>(source)` | Reactive pointer to signal/value | `Binding<T>` |
| `bindReadonly<T>(source)` | Read-only binding | `ReadonlyBinding<T>` |
| `signals<T>(initial)` | Create multiple signals from object | `{ [K]: WritableSignal<T[K]> }` |
| `slot<T>(initial?)` | Stable reactive cell | `Slot<T>` |
| `slotArray<T>(default?)` | Array of slots | `SlotArray<T>` |
| `trackedSlotArray<T>(default, dirtySet)` | Slots with dirty tracking | `SlotArray<T>` |
| `linkedSignal<D>(config)` | Writable that resets on source change | `WritableSignal<D>` |
| `createSelector<T, U>(source, fn?)` | O(2) list selection optimization | `SelectorFn<T, U>` |
| `effectScope(detached?)` | Group effects for batch disposal | `EffectScope` |
| `getCurrentScope()` | Get active scope | `EffectScope \| null` |
| `onScopeDispose(fn)` | Register cleanup on scope | `void` |
| `reactiveProps<T>(rawProps)` | Normalize props to reactive | `ReactiveProps<T>` |

### Deep Reactivity

| Function | Description | Returns |
|----------|-------------|---------|
| `state<T>(initial)` | Deeply reactive object (no .value) | `T` (proxied) |
| `stateRaw<T>(initial)` | Signal holding object (no deep) | `WritableSignal<T>` |
| `proxy<T>(value)` | Create reactive proxy | `T` (proxied) |
| `toRaw<T>(value)` | Get original from proxy | `T` |
| `isReactive(value)` | Check if proxied | `boolean` |

### Collections

| Class | Description |
|-------|-------------|
| `ReactiveMap<K, V>` | Map with per-key reactivity |
| `ReactiveSet<T>` | Set with per-item reactivity |
| `ReactiveDate` | Date with reactive accessors |

### Batching & Utilities

| Function | Description | Returns |
|----------|-------------|---------|
| `batch<T>(fn)` | Group updates into single reaction | `T` |
| `untrack<T>(fn)` | Read without creating dependencies | `T` |
| `peek<T>(fn)` | Alias for untrack | `T` |
| `flushSync<T>(fn?)` | Synchronously flush effects | `T \| undefined` |
| `tick()` | Wait for next update cycle | `Promise<void>` |

### Equality Functions

| Function | Description |
|----------|-------------|
| `equals(a, b)` | Strict equality (Object.is) - default for signals |
| `safeEquals(a, b)` | Handles NaN and objects |
| `shallowEquals(a, b)` | One level deep comparison |
| `deepEquals(a, b)` | Full structural comparison - default for derived |
| `neverEquals(a, b)` | Always returns false (always trigger) |
| `alwaysEquals(a, b)` | Always returns true (never trigger) |
| `createEquals(fn)` | Create custom equality |

### Low-Level API (for advanced use)

| Function | Description |
|----------|-------------|
| `get<T>(signal)` | Read with dependency tracking |
| `set<T>(signal, value)` | Write with dirty propagation |
| `isDirty(reaction)` | Check if needs update |
| `setSignalStatus(signal, status)` | Set status flags |
| `markReactions(signal, status)` | Propagate dirty state |
| `updateReaction(reaction)` | Run reaction with tracking |
| `removeReactions(reaction, start)` | Clean up stale deps |
| `disconnectSource(source)` | Break from graph |
| `disconnectDerived(derived)` | Disconnect derived |
| `disconnectBinding(binding)` | Disconnect binding |

---

## Type System

### Core Interfaces (from types.ts)

```typescript
interface Signal {
  f: number       // Flags bitmask
  wv: number      // Write version
}

interface Source<T> extends Signal {
  v: T                          // Current value
  equals: Equals<T>             // Equality function
  reactions: Reaction[] | null  // Dependents
  rv: number                    // Read version (deduplication)
}

interface Reaction extends Signal {
  fn: Function | null           // Function to execute
  deps: Source[] | null         // Dependencies
}

interface Derived<T> extends Source<T>, Reaction {
  fn: () => T                   // Computation function
  effects: Effect[] | null      // Child effects
  parent: Effect | Derived | null
}

interface Effect extends Reaction {
  fn: EffectFn | null
  teardown: CleanupFn | null    // Cleanup from last run
  parent: Effect | null         // Parent in effect tree
  first: Effect | null          // First child
  last: Effect | null           // Last child
  prev: Effect | null           // Previous sibling
  next: Effect | null           // Next sibling
}
```

### Public API Types

```typescript
interface ReadableSignal<T> {
  readonly value: T
}

interface WritableSignal<T> extends ReadableSignal<T> {
  value: T
}

interface DerivedSignal<T> extends ReadableSignal<T> {}

interface Binding<T> {
  get value(): T
  set value(v: T)
}

interface Slot<T> {
  readonly value: T
  source: T | Signal<T> | (() => T)
  set(value: T): void
  peek(): T
}

interface EffectScope {
  readonly active: boolean
  readonly paused: boolean
  run<R>(fn: () => R): R | undefined
  stop(): void
  pause(): void
  resume(): void
}
```

---

## Flag Constants (from constants.ts)

```typescript
// Signal type flags
DERIVED      = 1 << 1   // Is a derived
EFFECT       = 1 << 2   // Is an effect
RENDER_EFFECT = 1 << 3  // Runs synchronously
ROOT_EFFECT  = 1 << 4   // Root effect scope
BRANCH_EFFECT = 1 << 5  // If/each block
USER_EFFECT  = 1 << 6   // User-created effect
BLOCK_EFFECT = 1 << 7   // Block effect

// Status flags
CLEAN        = 1 << 10  // Up-to-date
DIRTY        = 1 << 11  // Needs update
MAYBE_DIRTY  = 1 << 12  // Check dependencies
REACTION_IS_UPDATING = 1 << 13
DESTROYED    = 1 << 14
INERT        = 1 << 15  // Paused
EFFECT_RAN   = 1 << 16  // Has run once
EFFECT_PRESERVED = 1 << 17

// Derived-specific
UNOWNED      = 1 << 8   // No owner
DISCONNECTED = 1 << 9   // No reactions

// Sentinel values (use Rust Option/enum instead)
UNINITIALIZED, STALE_REACTION
```

---

## Global State (from globals.ts)

Thread-local state needed:

```rust
struct ReactiveContext {
    // Reaction tracking
    active_reaction: Option<WeakReaction>,
    active_effect: Option<WeakReaction>,
    untracking: bool,

    // Version counters
    write_version: u64,      // Starts at 1
    read_version: u64,       // Starts at 0

    // Dependency tracking (during reaction execution)
    new_deps: Option<Vec<RcSource>>,
    skipped_deps: usize,
    untracked_writes: Option<Vec<RcSource>>,

    // Batching
    batch_depth: u32,
    pending_reactions: Vec<WeakReaction>,
    queued_root_effects: Vec<WeakReaction>,
    is_flushing_sync: bool,

    // Update cycle tracking
    update_cycle_id: u64,
}
```

---

## Core Algorithms

### get() - Read with Dependency Tracking

```
1. If active_reaction exists and not untracking:
   a. If REACTION_IS_UPDATING flag set:
      - Version-based deduplication: only add if signal.rv < read_version
      - Optimization: if deps in same order, increment skipped_deps
      - Otherwise: add to new_deps
   b. Else (after await):
      - Add directly to deps
      - Register reaction with signal

2. If signal is DERIVED:
   - Update derived chain iteratively

3. Return value
```

### set() - Write with Dirty Propagation

```
1. Check not inside a derived (throw error)
2. Compare with equality function
3. If changed:
   - Update value
   - Increment write_version
   - markReactions(signal, DIRTY)
   - Track untracked writes for self-invalidation
4. Return value
```

### markReactions() - Propagate Dirty State (ITERATIVE)

```
Use explicit stack to avoid recursion:

stack = [(signal, status)]
while stack not empty:
    (signal, status) = stack.pop()
    for reaction in signal.reactions:
        if not DIRTY:
            setSignalStatus(reaction, status)
        if reaction is DERIVED:
            stack.push((reaction, MAYBE_DIRTY))
        else if not was DIRTY:
            scheduleEffect(reaction)
```

### updateDerivedChain() - Update Derived Dependencies (ITERATIVE)

```
1. Quick check: if clean, return
2. Start new update cycle
3. Collect all deriveds from target to sources:
   chain = [target]
   for each in chain:
       for dep in current.deps:
           if dep is DERIVED and (DIRTY or MAYBE_DIRTY):
               chain.push(dep)

4. Update from deepest back to target:
   for i in (chain.length-1..=0):
       if DIRTY: updateDerived()
       else if MAYBE_DIRTY:
           check if any dep.wv > current.wv
           if yes: updateDerived()
           else: mark CLEAN
```

### updateReaction() - Run with Dependency Tracking

```
1. Save previous tracking state
2. Set up: new_deps = null, skipped_deps = 0, active_reaction = self
3. Increment read_version
4. Set REACTION_IS_UPDATING flag
5. Execute fn()
6. Handle new dependencies:
   - Remove old deps not in new set (swap-and-pop)
   - Install new deps
   - Register with new deps
7. Handle self-invalidation (effect wrote to its deps)
8. Restore previous state
```

### Effect Scheduling

```
scheduleEffect(reaction):
1. Add to pending_reactions
2. If batch_depth > 0: return
3. Walk up to root effect
4. Add root to queued_root_effects
5. If sync (RENDER_EFFECT): flushSync()
   Else: queue callback (equivalent to microtask)
```

---

## Memory Management

### TypeScript Approach
- FinalizationRegistry breaks cycles on GC
- Weak references for reaction->signal links

### Rust Approach
- `Rc<RefCell<T>>` for sources
- `Weak<RefCell<T>>` for back-references
- Drop trait for cleanup
- No FinalizationRegistry needed - Rust's RAII handles it

### Cleanup Patterns

1. **Effect disposal**: destroyEffect() nullifies all links
2. **Derived disconnection**: removeReactions(), clear deps
3. **Source disconnection**: clear reactions array
4. **Binding disconnection**: disconnect internal source

---

## Rust Implementation Strategy

### Type Hierarchy

```rust
// Type-erased traits
trait AnySource { ... }
trait AnyReaction { ... }

// Concrete types
struct SourceInner<T> { ... }
struct DerivedInner<T> { ... }
struct EffectInner { ... }

// Public handles
struct Signal<T>(Rc<RefCell<SourceInner<T>>>);
struct Derived<T>(Rc<RefCell<DerivedInner<T>>>);
struct Effect(Rc<RefCell<EffectInner>>);
```

### Handling Type Erasure

Sources and reactions need to be stored in heterogeneous collections:

```rust
type RcSource = Rc<RefCell<dyn AnySource>>;
type WeakReaction = Weak<RefCell<dyn AnyReaction>>;

// Signal implements AnySource
impl<T: 'static> AnySource for SourceInner<T> { ... }

// This allows storing mixed-type signals in deps/reactions lists
```

### Two APIs Example

**TypeScript-like (with macros):**
```rust
use spark_signals::prelude::*;

let count = signal!(0);
let doubled = derived!(count.get() * 2);
effect!(|| println!("{}", doubled.get()));
count.set(5);
```

**Idiomatic Rust:**
```rust
use spark_signals::prelude::*;

let count = Signal::new(0);
let doubled = Derived::new(|| count.get() * 2);
Effect::new(|| println!("{}", doubled.get()));
count.set(5);
```

---

## Deep Reactivity (Proxy Replacement)

TypeScript uses Proxy for deep reactivity. Rust doesn't have proxies, so we use:

### Option 1: Derive Macro
```rust
#[derive(Reactive)]
struct User {
    name: String,
    age: u32,
}

// Generates:
struct UserReactive {
    name: Signal<String>,
    age: Signal<u32>,
}
```

### Option 2: Wrapper Types
```rust
let user = Reactive::new(User { name: "Alice".into(), age: 30 });
user.name.set("Bob".into());
```

---

## Slots (Stable Reactive Cells)

Slots are stable identities that can point to different sources:

```rust
struct Slot<T> {
    source_type: SourceType,  // Static, Signal, or Getter
    inner: Rc<RefCell<SlotInner<T>>>,
}

enum SourceType {
    Static,
    Signal,
    Getter,
}
```

Key feature: When you read `slot.value`, it:
1. Tracks the slot itself
2. Tracks through to the underlying source

---

## Collections

### ReactiveVec (Rust-specific, replaces array proxy)
```rust
struct ReactiveVec<T> {
    items: Vec<Rc<RefCell<SourceInner<T>>>>,
    length: Signal<usize>,
    version: Signal<u64>,
}
```

### ReactiveMap
```rust
struct ReactiveMap<K, V> {
    inner: HashMap<K, V>,
    key_signals: HashMap<K, Signal<u64>>,  // Per-key version
    version: Signal<u64>,                   // Structural changes
    size: Signal<usize>,
}
```

### ReactiveSet
```rust
struct ReactiveSet<T> {
    inner: HashSet<T>,
    item_signals: HashMap<T, Signal<bool>>,  // Per-item presence
    version: Signal<u64>,
    size: Signal<usize>,
}
```

---

## Effect Scheduling (No Microtasks)

Rust doesn't have built-in microtasks. Options:

1. **Callback queue**: User calls `flush()` manually
2. **Async runtime integration**: Use tokio/async-std spawn
3. **Immediate for sync effects**: effect.sync() runs immediately

Current approach:
- Sync effects run immediately
- Async effects queue and require explicit flush or async runtime

---

## Testing Strategy

1. **Unit tests**: Each primitive in isolation
2. **Integration tests**: Reactive graph behavior
3. **Port TS tests**: Translate existing test cases
4. **Property-based tests**: Edge cases, cycles, deep graphs
5. **Benchmarks**: Compare with TS version

---

## Implementation Order

Starting fresh with GSD approach. See PROGRESS.md for history of previous attempts.

The phases below will be refined during GSD roadmap creation:

### Phase 1: Core Foundation
- [ ] flags.rs - Flag constants
- [ ] types.rs - Type traits and definitions
- [ ] globals.rs - Thread-local context

### Phase 2: Reactivity Engine
- [ ] tracking.rs - get(), set(), dependency tracking
- [ ] scheduling.rs - Effect scheduling
- [ ] batching.rs - batch(), untrack()

### Phase 3: Core Primitives
- [ ] signal.rs - Signal primitive
- [ ] derived.rs - Computed values with MAYBE_DIRTY
- [ ] effect.rs - Side effects

### Phase 4: Advanced Primitives
- [ ] bind.rs, slot.rs, linked.rs, selector.rs, scope.rs

### Phase 5: Collections
- [ ] ReactiveVec, ReactiveMap, ReactiveSet

### Phase 6+: Deep Reactivity, Macros, Polish
