# Technology Stack: Rust Reactive Signal Patterns

**Project:** Spark Signals (Rust port of @rlabs-inc/signals)
**Researched:** 2026-01-23
**Overall Confidence:** MEDIUM (based on training data - external verification tools unavailable)

> **Note:** WebSearch, WebFetch, and Context7 were unavailable during this research.
> Findings are based on training knowledge of Leptos, Dioxus, Sycamore, and futures-signals.
> All patterns should be verified against current source code before implementation.

---

## Executive Summary

Rust reactive libraries have converged on several key patterns to solve the same problems this port faces: type erasure, ownership without lifetimes, and ergonomic APIs. The dominant approaches are:

1. **Arena allocation with generational indices** (Leptos, Sycamore) - Avoids `Rc<RefCell<T>>` entirely
2. **Copy-able signal handles** (all libraries) - Signals are just indices/IDs, not owned data
3. **Runtime/Context pattern** (all libraries) - Global or thread-local reactive runtime
4. **Trait object erasure with `Any`** (common pattern) - Type erasure via `dyn Any + 'static`

For this port, **Rc<RefCell<T>> is explicitly chosen per Rule Zero** (match TypeScript ergonomics). This diverges from the arena approach but simplifies the implementation significantly.

---

## Pattern Analysis: How Rust Signal Libraries Work

### Pattern 1: Arena Allocation with Generational Indices

**Used by:** Leptos (reactive_graph), Sycamore
**Confidence:** MEDIUM

#### How It Works

Instead of heap-allocating each signal individually, all signals live in a contiguous arena. Signal handles are just indices into this arena:

```rust
// Conceptual structure (not exact Leptos code)
struct SignalArena {
    // Type-erased storage
    values: Vec<Box<dyn Any + Send + Sync>>,
    // Generation counters to detect stale handles
    generations: Vec<u64>,
}

#[derive(Copy, Clone)]
struct Signal<T> {
    index: usize,
    generation: u64,
    _marker: PhantomData<T>,
}
```

**Key insight:** The `Signal<T>` is just an index with a phantom type. It's `Copy` because it holds no data.

#### Why Libraries Use This

1. **No Rc/RefCell** - Avoids runtime borrow checking overhead
2. **Copy-able handles** - Signals can be freely passed around
3. **Predictable memory layout** - Better cache locality
4. **Generational safety** - Detects use-after-free via generation mismatch

#### Tradeoff for This Port

Arena allocation requires:
- A global/thread-local runtime to own the arena
- More complex signal creation (must register with runtime)
- Careful lifetime management of the arena itself

Per Rule Zero, we're using `Rc<RefCell<T>>` instead, accepting the runtime borrow checking overhead for simpler implementation.

---

### Pattern 2: Thread-Local Runtime Context

**Used by:** Leptos, Dioxus, Sycamore
**Confidence:** HIGH

#### How It Works

All reactive state lives in a thread-local context:

```rust
thread_local! {
    static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::new());
}

struct Runtime {
    // Currently executing reaction
    current_observer: Option<NodeId>,
    // Signals pending notification
    pending_effects: Vec<NodeId>,
    // The reactive graph
    nodes: SlotMap<NodeId, ReactiveNode>,
}

fn with_runtime<R>(f: impl FnOnce(&mut Runtime) -> R) -> R {
    RUNTIME.with(|rt| f(&mut rt.borrow_mut()))
}
```

**Key insight:** This is exactly what the TypeScript `globals.ts` does with module-level variables.

#### Direct Mapping to TypeScript

| TypeScript (`globals.ts`) | Rust Equivalent |
|---------------------------|-----------------|
| `activeReaction` | `runtime.current_observer` |
| `activeEffect` | `runtime.current_effect` |
| `writeVersion` | `runtime.write_version` |
| `readVersion` | `runtime.read_version` |
| `newDeps` / `skippedDeps` | `runtime.dependency_tracker` |
| `pendingReactions` | `runtime.pending_effects` |
| `batchDepth` | `runtime.batch_depth` |

#### Rust Implementation

```rust
use std::cell::RefCell;

thread_local! {
    static CONTEXT: RefCell<ReactiveContext> = RefCell::new(ReactiveContext::default());
}

#[derive(Default)]
struct ReactiveContext {
    active_reaction: Option<WeakReaction>,
    active_effect: Option<WeakReaction>,
    untracking: bool,
    write_version: u64,
    read_version: u64,
    new_deps: Option<Vec<RcSource>>,
    skipped_deps: usize,
    untracked_writes: Option<Vec<RcSource>>,
    batch_depth: u32,
    pending_reactions: Vec<WeakReaction>,
    queued_root_effects: Vec<WeakReaction>,
    is_flushing_sync: bool,
}
```

---

### Pattern 3: Type Erasure via `dyn Any`

**Used by:** All libraries when storing heterogeneous signal types
**Confidence:** HIGH

#### The Problem

TypeScript can store mixed-type signals in arrays easily:

```typescript
interface Source<T> {
  v: T
  reactions: Reaction[]  // This works because TypeScript is structural
}

// deps can contain Source<number>, Source<string>, etc.
deps: Source[]
```

Rust needs explicit type erasure:

```rust
// Can't do this - Vec needs a single concrete type
let deps: Vec<Source<T>> = vec![];  // What is T?
```

#### Solution: Trait Objects with Any

```rust
use std::any::Any;

/// Type-erased source that can be stored in heterogeneous collections
trait AnySource: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    // Methods that don't need the type parameter
    fn flags(&self) -> u32;
    fn set_flags(&mut self, flags: u32);
    fn write_version(&self) -> u64;
    fn read_version(&self) -> u64;
    fn set_read_version(&mut self, rv: u64);
    fn reactions(&self) -> &[WeakReaction];
    fn add_reaction(&mut self, reaction: WeakReaction);
    fn remove_reaction(&mut self, reaction: &WeakReaction);
}

/// Type-erased reaction
trait AnyReaction: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn flags(&self) -> u32;
    fn set_flags(&mut self, flags: u32);
    fn deps(&self) -> &[RcSource];
    fn set_deps(&mut self, deps: Vec<RcSource>);
    fn execute(&mut self) -> Option<Box<dyn Any>>;
}

// Now we can store them
type RcSource = Rc<RefCell<dyn AnySource>>;
type WeakReaction = Weak<RefCell<dyn AnyReaction>>;
```

#### Concrete Implementation Pattern

```rust
struct SourceInner<T: 'static> {
    flags: u32,
    value: T,
    write_version: u64,
    read_version: u64,
    equals: Box<dyn Fn(&T, &T) -> bool>,
    reactions: Vec<WeakReaction>,
}

impl<T: 'static> AnySource for SourceInner<T> {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn flags(&self) -> u32 { self.flags }
    fn set_flags(&mut self, flags: u32) { self.flags = flags }
    // ... etc
}

// Public handle - wraps the type-erased inner
pub struct Signal<T: 'static> {
    inner: Rc<RefCell<dyn AnySource>>,
    _marker: PhantomData<T>,
}

impl<T: 'static> Signal<T> {
    pub fn get(&self) -> T
    where T: Clone
    {
        let inner = self.inner.borrow();
        let concrete = inner.as_any().downcast_ref::<SourceInner<T>>().unwrap();
        // ... tracking logic ...
        concrete.value.clone()
    }
}
```

---

### Pattern 4: Weak References for Back-Edges

**Used by:** All libraries
**Confidence:** HIGH

#### The Problem

Bidirectional graph edges cause reference cycles:

```
Signal A --strong--> Reaction B
         <--strong-- (deps list)
```

This leaks memory because neither can be dropped.

#### Solution: Weak References for Back-Edges

```rust
// Signal stores weak references to its dependents
struct SourceInner<T> {
    reactions: Vec<Weak<RefCell<dyn AnyReaction>>>,  // Weak!
}

// Reaction stores strong references to its dependencies
struct EffectInner {
    deps: Vec<Rc<RefCell<dyn AnySource>>>,  // Strong
}
```

**Convention:**
- Dependencies point "up" with strong refs (reaction -> source)
- Reactions point "down" with weak refs (source -> reaction)

When a reaction is dropped:
1. Its strong refs to sources are released
2. Sources' weak refs become invalid (upgrade returns None)
3. Sources can clean up stale weak refs lazily

---

### Pattern 5: Iterative Graph Traversal

**Used by:** Leptos, and already in the TypeScript source
**Confidence:** HIGH

#### The Problem

Deep derived chains can cause stack overflow with recursion:

```typescript
// 1000 nested deriveds
const d1 = derived(() => signal.value)
const d2 = derived(() => d1.value)
// ...
const d1000 = derived(() => d999.value)
```

#### Solution: Explicit Stack

The TypeScript already does this (see `updateDerivedChain` in tracking.ts):

```typescript
function updateDerivedChain(target: Derived): void {
  const chain: Derived[] = [target]
  let idx = 0

  // Build chain iteratively
  while (idx < chain.length) {
    const current = chain[idx]
    idx++
    // Add dependencies to chain
    for (const dep of current.deps) {
      if (isDirty(dep)) chain.push(dep)
    }
  }

  // Process in reverse (deepest first)
  for (let i = chain.length - 1; i >= 0; i--) {
    updateDerived(chain[i])
  }
}
```

**Rust translation is straightforward** - just use `Vec<NodeId>` as the explicit stack.

---

### Pattern 6: SlotMap / Generational Index Crates

**Used by:** Leptos, Sycamore (internally)
**Confidence:** MEDIUM

#### What It Is

`slotmap` is a crate providing arena-allocated storage with stable keys:

```rust
use slotmap::{SlotMap, new_key_type};

new_key_type! { pub struct SignalKey; }

let mut signals: SlotMap<SignalKey, Box<dyn AnySource>> = SlotMap::with_key();
let key = signals.insert(Box::new(source_inner));
// key is Copy and stable even as other entries are removed
```

#### Why This Port Won't Use It

Per the constraints:
- Zero production dependencies
- Rule Zero: Use `Rc<RefCell<T>>` for simplicity

SlotMap is elegant but adds a dependency. Our approach with `Rc<RefCell<T>>` achieves the same goals differently.

---

## Recommended Stack for This Port

### Core Approach

| Aspect | Recommendation | Rationale |
|--------|---------------|-----------|
| **Signal Storage** | `Rc<RefCell<SourceInner<T>>>` | Rule Zero - match TypeScript ergonomics |
| **Type Erasure** | `dyn AnySource + 'static` trait | Required for heterogeneous collections |
| **Dependency Edges** | Strong refs (reaction -> source) | Ownership flows up the graph |
| **Reaction Edges** | `Weak<RefCell<dyn AnyReaction>>` | Avoid cycles, allow cleanup |
| **Global State** | `thread_local! { RefCell<Context> }` | Direct port of TypeScript globals |
| **Graph Traversal** | Iterative with explicit stack | Already in TypeScript, avoid stack overflow |

### Type Architecture

```rust
// === Type-Erased Traits ===

trait AnySource: Any {
    // Type-erased operations
    fn flags(&self) -> u32;
    fn write_version(&self) -> u64;
    fn reactions(&self) -> &[WeakReaction];
    // ...
}

trait AnyReaction: Any {
    fn flags(&self) -> u32;
    fn deps(&self) -> &[RcSource];
    fn execute(&mut self);
    // ...
}

// === Concrete Types ===

struct SourceInner<T: 'static> { ... }
struct DerivedInner<T: 'static> { ... }  // implements BOTH traits
struct EffectInner { ... }

// === Public Handles ===

pub struct Signal<T>(Rc<RefCell<dyn AnySource>>, PhantomData<T>);
pub struct Derived<T>(Rc<RefCell<dyn AnySource>>, PhantomData<T>);  // Note: stores as Source
pub struct Effect(Rc<RefCell<dyn AnyReaction>>);

// === Type-Erased References ===

type RcSource = Rc<RefCell<dyn AnySource>>;
type WeakReaction = Weak<RefCell<dyn AnyReaction>>;
```

### Key Implementation Details

#### Derived is Both Source AND Reaction

This is the trickiest part. In TypeScript:

```typescript
interface Derived<T> extends Source<T>, Reaction { ... }
```

In Rust, we need `DerivedInner<T>` to implement BOTH `AnySource` and `AnyReaction`:

```rust
struct DerivedInner<T: 'static> {
    // Source fields
    flags: u32,
    value: T,
    write_version: u64,
    read_version: u64,
    equals: Box<dyn Fn(&T, &T) -> bool>,
    reactions: Vec<WeakReaction>,

    // Reaction fields
    deps: Vec<RcSource>,
    fn_: Box<dyn Fn() -> T>,

    // Derived-specific
    effects: Vec<Rc<RefCell<EffectInner>>>,
    parent: Option<WeakReaction>,
}

impl<T: 'static> AnySource for DerivedInner<T> { ... }
impl<T: 'static> AnyReaction for DerivedInner<T> { ... }
```

The challenge: We need to store it as BOTH `RcSource` AND be able to treat it as a reaction. Options:

1. **Two Rc pointers** - Wasteful, complex
2. **Enum wrapper** - `enum Node { Source(RcSource), Derived(RcDerived), Effect(RcEffect) }`
3. **Combined trait** - `trait AnyNode: AnySource + AnyReaction` (but not all nodes are both)

Recommended: **Option 2 (Enum wrapper)** for the dependency graph, with concrete types for public API:

```rust
enum ReactiveNode {
    Source(RcSource),
    Derived(Rc<RefCell<dyn DerivedNode>>),  // Special trait for dual-natured nodes
    Effect(Rc<RefCell<dyn AnyReaction>>),
}
```

#### Circular Module Dependencies

TypeScript has circular imports between tracking, scheduling, and derived. Rust doesn't allow circular dependencies between modules.

**Solution: Late binding via function pointers**

The TypeScript already uses this pattern:

```typescript
// tracking.ts
let updateDerivedImpl: (derived: Derived) => void = () => {
  throw new Error('not initialized')
}

export function setUpdateDerivedImpl(impl: ...) {
  updateDerivedImpl = impl
}

// derived.ts
setUpdateDerivedImpl(updateDerived)
```

Rust version:

```rust
// tracking.rs
static UPDATE_DERIVED: AtomicPtr<fn(&mut dyn AnySource)> = AtomicPtr::new(std::ptr::null_mut());

pub fn set_update_derived_impl(f: fn(&mut dyn AnySource)) {
    UPDATE_DERIVED.store(f as *mut _, Ordering::SeqCst);
}

fn update_derived(source: &mut dyn AnySource) {
    let f = UPDATE_DERIVED.load(Ordering::SeqCst);
    if f.is_null() { panic!("not initialized") }
    unsafe { (*f)(source) }
}
```

Or simpler with `OnceCell`:

```rust
use std::sync::OnceLock;

static UPDATE_DERIVED: OnceLock<fn(RcSource)> = OnceLock::new();

pub fn set_update_derived_impl(f: fn(RcSource)) {
    UPDATE_DERIVED.set(f).expect("already initialized");
}

fn update_derived(source: RcSource) {
    UPDATE_DERIVED.get().expect("not initialized")(source)
}
```

---

## Patterns NOT to Use

### Anti-Pattern 1: Lifetime Annotations Everywhere

**Per Rule Zero - explicitly forbidden**

```rust
// BAD - fighting the borrow checker
struct Signal<'a, T: 'a> {
    inner: &'a RefCell<SourceInner<'a, T>>,
}

struct SourceInner<'a, T: 'a> {
    reactions: Vec<&'a RefCell<dyn Reaction<'a>>>,
}
```

This creates a lifetime infection that makes the API unusable.

### Anti-Pattern 2: Mutex for Single-Threaded Code

```rust
// BAD - unnecessary overhead
struct Signal<T> {
    inner: Arc<Mutex<SourceInner<T>>>,
}
```

The library is single-threaded by default. Use `Rc<RefCell<T>>`. The `sync` feature can add `Arc<RwLock<T>>` later.

### Anti-Pattern 3: Clone-Heavy APIs

```rust
// BAD - forcing users to clone everywhere
impl<T: Clone> Signal<T> {
    pub fn get(&self) -> T {
        self.inner.borrow().value.clone()
    }
}
```

Better: Return reference when possible, require `Clone` only where necessary:

```rust
impl<T> Signal<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.inner.borrow().value)
    }

    pub fn get(&self) -> T where T: Clone {
        self.with(|v| v.clone())
    }
}
```

---

## Dependencies

### Production Dependencies

**NONE** (per constraints)

All patterns implemented with `std` library:
- `std::cell::{Cell, RefCell}`
- `std::rc::{Rc, Weak}`
- `std::any::Any`
- `std::collections::{HashMap, HashSet, VecDeque}`
- `std::sync::OnceLock` (for late binding)

### Dev Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| criterion | 0.5 | Benchmarking |

---

## Comparison with Major Libraries

| Aspect | Leptos | Dioxus | Sycamore | This Port |
|--------|--------|--------|----------|-----------|
| Storage | Arena (SlotMap) | Arena | Arena | Rc<RefCell> |
| Handle type | Copy index | Copy index | Copy index | Clone Rc |
| Runtime | Thread-local | Thread-local | Thread-local | Thread-local |
| Type erasure | dyn Any | dyn Any | dyn Any | dyn AnySource |
| Dependencies | slotmap, etc | generational-box | slotmap | **None** |
| Thread safety | Optional | Optional | Single-thread | Optional (future) |

---

## Open Questions (Require Verification)

1. **Leptos reactive_graph current API** - Training data may be outdated. Verify current implementation.

2. **Dioxus signals 0.5+ changes** - Dioxus underwent significant refactoring. Verify current patterns.

3. **Performance of Rc<RefCell> vs Arena** - Need benchmarks to validate that the simplicity tradeoff is acceptable.

4. **Effect scheduling without microtasks** - How do other Rust libraries handle async effect scheduling without JS microtask queue?

---

## Sources

**Confidence Level: MEDIUM - Training Data Only**

- Leptos source code (as of training cutoff, ~May 2025)
- Dioxus source code (as of training cutoff, ~May 2025)
- Sycamore source code (as of training cutoff, ~May 2025)
- futures-signals crate documentation
- General Rust patterns for type erasure and interior mutability

**Verification Recommended:**
- Clone current versions of Leptos, Dioxus, Sycamore
- Review reactive_graph, signals, and sycamore-reactive crates
- Benchmark Rc<RefCell> approach against arena allocation

---

*Stack research: 2026-01-23*
