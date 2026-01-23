# Architecture Patterns for Rust Reactive Signals

**Domain:** Reactive signals library port (TypeScript to Rust)
**Researched:** 2026-01-23
**Confidence:** MEDIUM (based on training data, no web verification available)

This document addresses the THREE HARD PROBLEMS that caused 5 failed implementation attempts:
1. **Type erasure** - Store `Source<T>` in heterogeneous collections
2. **Circular dependencies** - tracking needs scheduling needs tracking
3. **Borrow rules in cascade** - cascade_maybe_dirty needs mutable graph traversal

---

## Problem 1: Type Erasure for Heterogeneous Signal Storage

### The Challenge

TypeScript can freely store mixed types:
```typescript
interface Source<T> {
  reactions: Reaction[] | null  // Mixed types OK
}

interface Reaction {
  deps: Source[] | null         // Source<number>, Source<string>, etc.
}
```

Rust needs concrete types. We need `Vec<SomeType>` where `SomeType` can hold `Source<i32>`, `Source<String>`, etc.

### Solution A: Generational Arena with Type-Erased Indices (Leptos/Dioxus Pattern)

**How it works:**
- Store signals in a generational arena (slotmap)
- Use type-erased indices (`SignalId`, `NodeId`) for references
- Type information exists only at access time via generic methods

```rust
use std::any::Any;

/// Type-erased storage for any signal value
struct AnySignalInner {
    value: Box<dyn Any>,
    flags: u32,
    write_version: u64,
    reactions: Vec<NodeId>,  // Type-erased references
    read_version: u64,
}

/// The arena storing all reactive nodes
struct ReactiveGraph {
    nodes: SlotMap<NodeId, ReactiveNode>,
}

enum ReactiveNode {
    Source(AnySignalInner),
    Derived(AnyDerivedInner),
    Effect(EffectInner),
}

/// Typed handle for user-facing API
struct Signal<T> {
    id: NodeId,
    _marker: PhantomData<T>,
}

impl<T: 'static> Signal<T> {
    fn get(&self) -> T {
        REACTIVE_GRAPH.with(|graph| {
            let node = &graph.borrow().nodes[self.id];
            // Downcast at access time
            node.value.downcast_ref::<T>().unwrap().clone()
        })
    }
}
```

**Pros:**
- No trait objects needed for storage
- Fast iteration (contiguous memory)
- Natural for graph operations
- Used by Leptos (proven at scale)

**Cons:**
- Requires global/thread-local arena
- Extra indirection through arena
- Downcasting overhead (minimal, but exists)

### Solution B: Trait Objects with Interior Mutability (Sycamore Pattern)

**How it works:**
- Define type-erased traits for node operations
- Store `Rc<dyn AnySource>` in collections
- Concrete types implement the trait

```rust
/// Type-erased source trait
trait AnySource: Any {
    fn flags(&self) -> u32;
    fn set_flags(&self, flags: u32);
    fn write_version(&self) -> u64;
    fn reactions(&self) -> &RefCell<Vec<Weak<dyn AnyReaction>>>;
    fn as_any(&self) -> &dyn Any;  // For downcasting
}

/// Type-erased reaction trait
trait AnyReaction: Any {
    fn flags(&self) -> u32;
    fn set_flags(&self, flags: u32);
    fn deps(&self) -> &RefCell<Vec<Rc<dyn AnySource>>>;
    fn execute(&self);  // Type-erased execution
}

/// Concrete source with type info
struct SourceInner<T> {
    value: RefCell<T>,
    flags: Cell<u32>,
    write_version: Cell<u64>,
    reactions: RefCell<Vec<Weak<dyn AnyReaction>>>,
    read_version: Cell<u64>,
    equals: Box<dyn Fn(&T, &T) -> bool>,
}

impl<T: 'static> AnySource for SourceInner<T> {
    fn flags(&self) -> u32 { self.flags.get() }
    fn set_flags(&self, f: u32) { self.flags.set(f); }
    // ...
}

/// Public handle
struct Signal<T>(Rc<SourceInner<T>>);
```

**Pros:**
- No global arena needed
- Signals are self-contained
- Matches TypeScript structure more directly
- Easier to reason about ownership

**Cons:**
- Trait object overhead (vtable dispatch)
- Scattered memory (not cache-friendly)
- Weak/Rc dance for back-references

### Solution C: Enum Dispatch (futures-signals Pattern)

**How it works:**
- Use enums to wrap different node types
- Dispatch manually based on enum variant

```rust
enum ReactiveNode {
    SourceI32(SourceInner<i32>),
    SourceString(SourceInner<String>),
    SourceBool(SourceInner<bool>),
    // ... limited set of types
    SourceAny(Box<dyn AnySource>),  // Fallback
}
```

**Pros:**
- No vtable overhead for common types
- Can optimize hot paths

**Cons:**
- Limited type set or fallback to trait objects anyway
- Code explosion with many types
- Not general-purpose

### RECOMMENDATION: Hybrid Approach

**For spark-signals, use Solution B (Trait Objects) with arena-style allocation hints:**

```rust
// Type-erased traits for graph storage
trait AnySource: Any {
    fn flags(&self) -> u32;
    fn set_flags(&self, flags: u32);
    fn write_version(&self) -> u64;
    fn read_version(&self) -> u64;
    fn set_read_version(&self, v: u64);
    fn reactions(&self) -> Ref<Vec<Weak<dyn AnyReaction>>>;
    fn reactions_mut(&self) -> RefMut<Vec<Weak<dyn AnyReaction>>>;
    fn as_any(&self) -> &dyn Any;
}

trait AnyReaction: AnySource {  // Note: extends AnySource
    fn deps(&self) -> Ref<Vec<Rc<dyn AnySource>>>;
    fn deps_mut(&self) -> RefMut<Vec<Rc<dyn AnySource>>>;
    fn execute(&self);
    fn is_derived(&self) -> bool;
}

// Type-erased storage aliases
type RcSource = Rc<dyn AnySource>;
type WeakReaction = Weak<dyn AnyReaction>;

// Concrete implementations
struct SourceInner<T> {
    value: RefCell<T>,
    flags: Cell<u32>,
    wv: Cell<u64>,
    rv: Cell<u64>,
    reactions: RefCell<Vec<WeakReaction>>,
    equals: fn(&T, &T) -> bool,
}

impl<T: 'static + Clone> AnySource for SourceInner<T> { ... }
```

**Why this approach:**
1. Matches TypeScript structure closely (faithful port)
2. No global arena complexity
3. Works with `Rc<RefCell<T>>` per Rule Zero
4. Derived can implement both `AnySource` and `AnyReaction`

---

## Problem 2: Circular Module Dependencies

### The Challenge

TypeScript circular dependencies work at runtime:
```
tracking.ts -> scheduling.ts (scheduleEffect)
scheduling.ts -> tracking.ts (isDirty, updateDerived)
derived.ts -> tracking.ts (get, set, updateReaction)
tracking.ts -> derived.ts (updateDerived)
```

Rust modules cannot be circular. Compilation fails.

### Solution A: Dependency Injection at Runtime (Leptos Pattern)

**How it works:**
- Forward declare functions as `static mut` or thread-local
- Set implementations at initialization

```rust
// tracking.rs
type UpdateDerivedFn = fn(&dyn AnyReaction);
static UPDATE_DERIVED: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

pub fn update_derived(derived: &dyn AnyReaction) {
    let f: UpdateDerivedFn = unsafe {
        std::mem::transmute(UPDATE_DERIVED.load(Ordering::Relaxed))
    };
    f(derived)
}

pub fn set_update_derived_impl(f: UpdateDerivedFn) {
    UPDATE_DERIVED.store(f as *mut (), Ordering::Relaxed);
}

// derived.rs
fn update_derived_impl(derived: &dyn AnyReaction) {
    // actual implementation
}

// lib.rs (initialization)
pub fn init() {
    tracking::set_update_derived_impl(derived::update_derived_impl);
    scheduling::set_update_effect_impl(effect::update_effect_impl);
}
```

**Note:** TypeScript already uses this pattern:
```typescript
// tracking.ts
let updateDerivedImpl: (derived: Derived) => void = () => {
  throw new Error('updateDerived not initialized')
}
export function setUpdateDerivedImpl(impl: (derived: Derived) => void) {
  updateDerivedImpl = impl
}
```

**Pros:**
- Direct port of TypeScript pattern
- No restructuring needed
- Works

**Cons:**
- Runtime indirection
- Must remember to initialize
- Unsafe pointer manipulation

### Solution B: Trait-Based Inversion (Sycamore Pattern)

**How it works:**
- Define traits that capture the "holes"
- Pass implementations via trait objects or generics

```rust
// core/traits.rs
trait DerivedUpdater {
    fn update_derived(&self, derived: &dyn AnyReaction);
}

trait EffectScheduler {
    fn schedule_effect(&self, reaction: &dyn AnyReaction);
}

// tracking.rs
pub fn get<T>(signal: &impl AnySource, ctx: &impl DerivedUpdater) -> T {
    // ...
    if signal.is_derived() {
        ctx.update_derived(signal.as_reaction());
    }
    // ...
}

// The context carries all implementations
struct ReactiveRuntime {
    // implements DerivedUpdater, EffectScheduler, etc.
}
```

**Pros:**
- Type-safe, no unsafe
- Compiler checks dependencies

**Cons:**
- Pervasive context passing
- More complex APIs
- Doesn't match TypeScript structure

### Solution C: Single Module with Regions (Simple Approach)

**How it works:**
- Put all interdependent code in one module
- Use `#[cfg]` or comments to logically separate

```rust
// reactivity.rs - ALL core reactivity in one file
// === TRACKING REGION ===
pub fn get<T>(signal: &Source<T>) -> T { ... }
pub fn set<T>(signal: &Source<T>, value: T) { ... }

// === SCHEDULING REGION ===
pub fn schedule_effect(reaction: &dyn AnyReaction) { ... }
pub fn flush_effects() { ... }

// === DERIVED REGION ===
pub fn update_derived(derived: &Derived) { ... }
```

**Pros:**
- No circular dependency possible
- Simple, direct

**Cons:**
- Large file
- Harder to navigate
- Doesn't match TypeScript structure

### Solution D: Layered Architecture (Dioxus Pattern)

**How it works:**
- Restructure to eliminate cycles
- Lower layers don't know about higher layers

```
Layer 0: types.rs (AnySource, AnyReaction traits)
Layer 1: graph.rs (markReactions, setSignalStatus - no scheduling)
Layer 2: derived.rs (updateDerived - calls graph, no scheduling)
Layer 3: scheduling.rs (imports derived, graph)
Layer 4: tracking.rs (imports all, ties together)
```

```rust
// graph.rs - pure graph operations
pub fn mark_reactions(signal: &dyn AnySource, status: u32) {
    // Only propagates flags, doesn't schedule
    // Returns list of effects that need scheduling
}

// scheduling.rs - imports graph, derived
pub fn mark_and_schedule(signal: &dyn AnySource, status: u32) {
    let effects_to_schedule = graph::mark_reactions(signal, status);
    for effect in effects_to_schedule {
        schedule_effect(effect);
    }
}

// tracking.rs - top-level coordinator
pub fn set<T>(signal: &Source<T>, value: T) {
    // ...
    scheduling::mark_and_schedule(signal, DIRTY);
}
```

**Pros:**
- Clean architecture
- No runtime indirection
- Type-safe

**Cons:**
- Requires restructuring TypeScript logic
- More indirection in call graph

### RECOMMENDATION: Solution A (Dependency Injection)

**Rationale:**
1. TypeScript ALREADY uses this pattern
2. Minimizes deviation from source
3. Simple to implement
4. Runtime cost is negligible (one function pointer load)

**Implementation:**

```rust
// tracking.rs
use std::cell::Cell;

thread_local! {
    static UPDATE_DERIVED: Cell<fn(&dyn AnyReaction)> = Cell::new(|_| {
        panic!("updateDerived not initialized - import derived module first");
    });

    static SCHEDULE_EFFECT: Cell<fn(&dyn AnyReaction)> = Cell::new(|_| {
        panic!("scheduleEffect not initialized - import scheduling module first");
    });
}

pub fn update_derived(derived: &dyn AnyReaction) {
    UPDATE_DERIVED.with(|f| (f.get())(derived))
}

pub fn set_update_derived_impl(f: fn(&dyn AnyReaction)) {
    UPDATE_DERIVED.with(|cell| cell.set(f))
}

// Similarly for schedule_effect

// lib.rs
mod tracking;
mod scheduling;
mod derived;
mod effect;

/// Initialize the reactive runtime. Must be called before using signals.
pub fn init() {
    tracking::set_update_derived_impl(derived::update_derived_internal);
    tracking::set_schedule_effect_impl(scheduling::schedule_effect_internal);
    scheduling::set_update_effect_impl(effect::update_effect_internal);
}
```

---

## Problem 3: Borrow Rules During Graph Traversal

### The Challenge

TypeScript mutates while iterating freely:
```typescript
function markReactions(signal: Source, status: number): void {
  const stack = [{ signal, status }]
  while (stack.length > 0) {
    const { signal, status } = stack.pop()!
    const reactions = signal.reactions  // Borrow
    for (const reaction of reactions) {
      setSignalStatus(reaction, status)  // Mutate!
      if (reaction.f & DERIVED) {
        stack.push({ signal: reaction, status: MAYBE_DIRTY })
      }
    }
  }
}
```

Rust's borrow checker prevents this: you can't mutate `reaction` while iterating `signal.reactions`.

### Solution A: Clone-on-Access (Simple but Costly)

**How it works:**
- Clone the reactions list before iterating
- Mutations don't affect iteration

```rust
fn mark_reactions(signal: &dyn AnySource, status: u32) {
    let mut stack = vec![(signal.clone_rc(), status)];

    while let Some((signal, status)) = stack.pop() {
        // Clone reactions to avoid borrow conflict
        let reactions: Vec<_> = signal.reactions()
            .iter()
            .filter_map(|w| w.upgrade())
            .collect();

        for reaction in reactions {
            reaction.set_flags(
                (reaction.flags() & STATUS_MASK) | status
            );

            if reaction.is_derived() {
                stack.push((reaction.as_source(), MAYBE_DIRTY));
            } else if (reaction.flags() & DIRTY) == 0 {
                schedule_effect(&reaction);
            }
        }
    }
}
```

**Pros:**
- Simple, obviously correct
- Matches TypeScript semantics

**Cons:**
- Allocation per iteration
- O(n) clone cost

### Solution B: Index-Based Iteration (Dioxus Pattern)

**How it works:**
- Use indices instead of references
- Access via arena/global, not local borrow

```rust
fn mark_reactions(signal_id: NodeId, status: u32) {
    let mut stack = vec![(signal_id, status)];

    while let Some((signal_id, status)) = stack.pop() {
        // Get reaction IDs (not references)
        let reaction_ids: Vec<NodeId> = GRAPH.with(|g| {
            g.borrow().nodes[signal_id]
                .reactions()
                .clone()
        });

        for reaction_id in reaction_ids {
            GRAPH.with(|g| {
                let mut graph = g.borrow_mut();
                let reaction = &mut graph.nodes[reaction_id];
                reaction.set_flags((reaction.flags() & STATUS_MASK) | status);

                if reaction.is_derived() {
                    // Can push, no borrow conflict
                }
            });

            // Schedule outside borrow
            if should_schedule {
                schedule_effect(reaction_id);
            }
        }
    }
}
```

**Pros:**
- No cloning
- Natural for arena-based design

**Cons:**
- Requires arena architecture
- More complex access patterns

### Solution C: RefCell with Careful Scoping

**How it works:**
- Use `RefCell` for interior mutability
- Carefully scope borrows to avoid conflicts

```rust
fn mark_reactions(signal: Rc<dyn AnySource>, status: u32) {
    let mut stack = vec![(signal, status)];

    while let Some((signal, status)) = stack.pop() {
        // Scope 1: Read reactions (immutable borrow)
        let reactions: Vec<Rc<dyn AnyReaction>> = {
            signal.reactions()  // Returns Ref<Vec<Weak<...>>>
                .iter()
                .filter_map(|w| w.upgrade())
                .collect()
        };  // Borrow ends here

        // Scope 2: Mutate each reaction (separate borrows)
        for reaction in reactions {
            // Each reaction is its own RefCell, so this is fine
            let flags = reaction.flags();
            let not_dirty = (flags & DIRTY) == 0;

            if not_dirty {
                reaction.set_flags((flags & STATUS_MASK) | status);
            }

            if reaction.is_derived() {
                // reaction implements AnySource, get as source
                stack.push((reaction.as_source(), MAYBE_DIRTY));
            } else if not_dirty {
                schedule_effect(&reaction);
            }
        }
    }
}
```

**Key insight:** Each `SourceInner<T>` has its OWN `RefCell`, so:
- Borrowing `signal.reactions()` borrows ONE RefCell
- Borrowing `reaction.flags()` borrows a DIFFERENT RefCell
- No conflict

**Pros:**
- Works with trait object design
- Minimal overhead
- Clear ownership

**Cons:**
- Must be careful about borrow scopes
- Can still panic at runtime if scopes overlap

### Solution D: Copy-on-Write with Dirty Tracking

**How it works:**
- Track which nodes are "dirty" in a separate bitset
- Iterate without mutation, then batch-apply

```rust
fn mark_reactions(signal: &dyn AnySource, status: u32) {
    // Phase 1: Collect what needs updating
    let mut to_mark: Vec<(Rc<dyn AnyReaction>, u32)> = vec![];
    let mut to_schedule: Vec<Rc<dyn AnyReaction>> = vec![];

    let mut stack = vec![(signal.clone_rc(), status)];
    while let Some((signal, status)) = stack.pop() {
        for reaction in signal.reactions().iter().filter_map(|w| w.upgrade()) {
            let flags = reaction.flags();
            if (flags & DIRTY) == 0 {
                to_mark.push((reaction.clone(), status));
            }
            if reaction.is_derived() {
                stack.push((reaction.as_source(), MAYBE_DIRTY));
            } else if (flags & DIRTY) == 0 {
                to_schedule.push(reaction.clone());
            }
        }
    }

    // Phase 2: Apply mutations
    for (reaction, status) in to_mark {
        let flags = reaction.flags();
        reaction.set_flags((flags & STATUS_MASK) | status);
    }

    for reaction in to_schedule {
        schedule_effect(&reaction);
    }
}
```

**Pros:**
- No borrow conflicts possible
- Batch operations can be optimized

**Cons:**
- Two passes
- Extra allocations

### RECOMMENDATION: Solution C (RefCell with Careful Scoping)

**Rationale:**
1. Works with trait object architecture (Problem 1 solution)
2. Minimal overhead
3. Matches TypeScript behavior closely
4. Each node has its own RefCell, so concurrent access to different nodes is fine

**Critical Implementation Pattern:**

```rust
// GOOD: Borrows don't overlap
fn mark_reactions(signal: Rc<dyn AnySource>, status: u32) {
    let mut stack = vec![(signal, status)];

    while let Some((signal, status)) = stack.pop() {
        // Collect reactions BEFORE any mutation
        let reactions: Vec<Rc<dyn AnyReaction>> = signal
            .reactions()         // Borrows signal's RefCell
            .iter()
            .filter_map(|w| w.upgrade())
            .collect();          // Borrow ENDS when collect() returns

        for reaction in reactions {
            // Now we can safely borrow reaction's RefCell
            // because it's a DIFFERENT RefCell than signal's
            let flags = reaction.flags();
            if (flags & DIRTY) == 0 {
                reaction.set_flags((flags & STATUS_MASK) | status);
            }
            // ...
        }
    }
}

// BAD: Would panic at runtime
fn mark_reactions_bad(signal: Rc<dyn AnySource>, status: u32) {
    let reactions = signal.reactions();  // Borrow starts
    for reaction in reactions.iter() {
        // If reaction IS signal (self-loop), this panics!
        reaction.set_flags(...);  // Tries to borrow again
    }  // Borrow ends
}
```

---

## Complete Architecture Recommendation

### Module Structure

```
src/
├── lib.rs              # Public API, init()
├── core/
│   ├── mod.rs
│   ├── flags.rs        # Flag constants (DIRTY, CLEAN, etc.)
│   ├── types.rs        # AnySource, AnyReaction traits
│   └── context.rs      # Thread-local ReactiveContext
├── reactivity/
│   ├── mod.rs
│   ├── tracking.rs     # get(), set(), updateReaction()
│   ├── scheduling.rs   # scheduleEffect(), flushEffects()
│   └── batching.rs     # batch(), untrack()
├── primitives/
│   ├── mod.rs
│   ├── signal.rs       # Signal<T>, SourceInner<T>
│   ├── derived.rs      # Derived<T>, DerivedInner<T>
│   └── effect.rs       # Effect, EffectInner
├── collections/
│   ├── mod.rs
│   ├── map.rs          # ReactiveMap
│   ├── set.rs          # ReactiveSet
│   └── vec.rs          # ReactiveVec
└── equality.rs         # Equality functions
```

### Core Types

```rust
// core/types.rs

use std::any::Any;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

/// Type-erased source trait
pub trait AnySource: Any {
    fn flags(&self) -> u32;
    fn set_flags(&self, flags: u32);
    fn write_version(&self) -> u64;
    fn set_write_version(&self, v: u64);
    fn read_version(&self) -> u64;
    fn set_read_version(&self, v: u64);

    fn reactions(&self) -> Ref<Vec<WeakReaction>>;
    fn reactions_mut(&self) -> RefMut<Vec<WeakReaction>>;

    fn as_any(&self) -> &dyn Any;
    fn clone_rc(&self) -> RcSource;
}

/// Type-erased reaction trait (extends AnySource for Derived)
pub trait AnyReaction: Any {
    fn flags(&self) -> u32;
    fn set_flags(&self, flags: u32);
    fn write_version(&self) -> u64;
    fn set_write_version(&self, v: u64);

    fn deps(&self) -> Ref<Vec<RcSource>>;
    fn deps_mut(&self) -> RefMut<Vec<RcSource>>;
    fn set_deps(&self, deps: Option<Vec<RcSource>>);

    fn execute(&self) -> Box<dyn Any>;  // Returns cleanup fn for effects

    fn is_derived(&self) -> bool;
    fn as_source(&self) -> Option<RcSource>;  // If derived

    fn clone_rc(&self) -> RcReaction;
}

/// Type aliases for storage
pub type RcSource = Rc<dyn AnySource>;
pub type WeakSource = Weak<dyn AnySource>;
pub type RcReaction = Rc<dyn AnyReaction>;
pub type WeakReaction = Weak<dyn AnyReaction>;
```

### Thread-Local Context

```rust
// core/context.rs

use std::cell::{Cell, RefCell};
use super::types::*;

thread_local! {
    pub static CONTEXT: RefCell<ReactiveContext> = RefCell::new(ReactiveContext::new());
}

pub struct ReactiveContext {
    // Tracking
    pub active_reaction: Option<RcReaction>,
    pub active_effect: Option<RcReaction>,  // Always an Effect
    pub untracking: bool,

    // Versions
    pub write_version: u64,
    pub read_version: u64,

    // Dependency collection
    pub new_deps: Option<Vec<RcSource>>,
    pub skipped_deps: usize,
    pub untracked_writes: Option<Vec<RcSource>>,

    // Batching
    pub batch_depth: u32,
    pub pending_reactions: Vec<RcReaction>,
    pub queued_root_effects: Vec<RcReaction>,
    pub is_flushing_sync: bool,
}

impl ReactiveContext {
    pub fn new() -> Self {
        Self {
            active_reaction: None,
            active_effect: None,
            untracking: false,
            write_version: 1,
            read_version: 0,
            new_deps: None,
            skipped_deps: 0,
            untracked_writes: None,
            batch_depth: 0,
            pending_reactions: Vec::new(),
            queued_root_effects: Vec::new(),
            is_flushing_sync: false,
        }
    }
}

// Helper macros for context access
macro_rules! with_context {
    ($f:expr) => {
        CONTEXT.with(|ctx| $f(&mut *ctx.borrow_mut()))
    };
}

macro_rules! with_context_ref {
    ($f:expr) => {
        CONTEXT.with(|ctx| $f(&*ctx.borrow()))
    };
}
```

### Concrete Signal Implementation

```rust
// primitives/signal.rs

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::Rc;
use std::any::Any;
use crate::core::types::*;
use crate::core::flags::*;

pub struct SourceInner<T> {
    value: RefCell<T>,
    flags: Cell<u32>,
    wv: Cell<u64>,
    rv: Cell<u64>,
    reactions: RefCell<Vec<WeakReaction>>,
    equals: fn(&T, &T) -> bool,
}

impl<T: 'static + Clone> SourceInner<T> {
    pub fn new(value: T, equals: fn(&T, &T) -> bool) -> Self {
        Self {
            value: RefCell::new(value),
            flags: Cell::new(SOURCE | CLEAN),
            wv: Cell::new(0),
            rv: Cell::new(0),
            reactions: RefCell::new(Vec::new()),
            equals,
        }
    }

    pub fn get_value(&self) -> T {
        self.value.borrow().clone()
    }

    pub fn set_value(&self, new_value: T) -> bool {
        let old = self.value.borrow();
        if (self.equals)(&*old, &new_value) {
            return false;
        }
        drop(old);
        *self.value.borrow_mut() = new_value;
        true
    }
}

impl<T: 'static + Clone> AnySource for SourceInner<T> {
    fn flags(&self) -> u32 { self.flags.get() }
    fn set_flags(&self, f: u32) { self.flags.set(f); }
    fn write_version(&self) -> u64 { self.wv.get() }
    fn set_write_version(&self, v: u64) { self.wv.set(v); }
    fn read_version(&self) -> u64 { self.rv.get() }
    fn set_read_version(&self, v: u64) { self.rv.set(v); }

    fn reactions(&self) -> Ref<Vec<WeakReaction>> {
        self.reactions.borrow()
    }
    fn reactions_mut(&self) -> RefMut<Vec<WeakReaction>> {
        self.reactions.borrow_mut()
    }

    fn as_any(&self) -> &dyn Any { self }

    fn clone_rc(&self) -> RcSource {
        // This requires storing Rc somewhere - see note below
        unimplemented!("Need self-referential Rc pattern")
    }
}

/// Public Signal handle
pub struct Signal<T>(pub(crate) Rc<SourceInner<T>>);

impl<T: 'static + Clone> Signal<T> {
    pub fn new(value: T) -> Self {
        Self::with_equals(value, |a, b| a == b)
    }

    pub fn with_equals(value: T, equals: fn(&T, &T) -> bool) -> Self {
        Signal(Rc::new(SourceInner::new(value, equals)))
    }

    pub fn get(&self) -> T {
        crate::reactivity::tracking::get(&self.0)
    }

    pub fn set(&self, value: T) {
        crate::reactivity::tracking::set(&self.0, value);
    }
}
```

### Dependency Injection Setup

```rust
// lib.rs

mod core;
mod reactivity;
mod primitives;
mod collections;
mod equality;

pub use primitives::{Signal, Derived, Effect};
pub use reactivity::batching::{batch, untrack, peek};
pub use reactivity::scheduling::{flush_sync, tick};

/// Initialize the reactive runtime.
/// Called automatically on first signal creation, but can be called explicitly.
pub fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        reactivity::tracking::set_update_derived_impl(
            primitives::derived::update_derived_internal
        );
        reactivity::tracking::set_schedule_effect_impl(
            reactivity::scheduling::schedule_effect_internal
        );
        reactivity::scheduling::set_update_effect_impl(
            primitives::effect::update_effect_internal
        );
    });
}

// Auto-init via ctor (optional, for convenience)
#[cfg(feature = "auto-init")]
#[ctor::ctor]
fn auto_init() {
    init();
}
```

---

## Summary: How Each Problem Is Solved

| Problem | Solution | Pattern |
|---------|----------|---------|
| **Type Erasure** | Trait objects (`dyn AnySource`, `dyn AnyReaction`) | Each concrete type implements traits, stored as `Rc<dyn Trait>` |
| **Circular Deps** | Dependency injection via thread-local function pointers | Same pattern TypeScript uses, just with `thread_local!` |
| **Borrow Conflicts** | Careful RefCell scoping, collect-then-mutate | Each node has own RefCell, borrows don't overlap |

---

## Confidence Assessment

| Area | Confidence | Reason |
|------|------------|--------|
| Type erasure via traits | HIGH | Well-established Rust pattern, used by multiple libraries |
| Dependency injection | HIGH | Direct port of TypeScript's existing pattern |
| RefCell scoping | MEDIUM | Requires careful implementation, runtime panics possible |
| Overall architecture | MEDIUM | Based on training data, not verified against latest library versions |

**Verification needed:**
- Test RefCell scoping patterns with actual derived chains
- Benchmark trait object overhead vs arena approach
- Verify circular reference handling with Weak pointers

---

## Next Steps for Implementation

1. **Start with `core/` module**: flags, types, context
2. **Implement `primitives/signal.rs`** with full `AnySource` trait
3. **Implement basic `tracking::get()`** - verify type erasure works
4. **Implement `tracking::set()` and `mark_reactions()`** - verify borrow patterns
5. **Add derived and effect** - verify circular dep injection works
6. **Integration tests** with deep derived chains - stress test all three solutions
