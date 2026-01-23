# Domain Pitfalls

**Domain:** Rust reactive signals library (porting TypeScript fine-grained reactivity)
**Researched:** 2026-01-23
**Confidence:** HIGH (based on TypeScript source analysis, 5 failed attempts, Rust memory model knowledge)

---

## Critical Pitfalls

Mistakes that cause rewrites or fundamentally broken implementations.

---

### Pitfall 1: RefCell Borrow Panic During Cascade Updates

**What goes wrong:**
When propagating dirty state through the reactive graph (`markReactions`), you hold a borrow on `signal.reactions` while iterating, then try to modify a reaction's flags or push to a stack. If the reaction being modified is the same `RefCell` you're iterating (or shares a `RefCell` with it), you get a runtime panic: `already borrowed: BorrowMutError`.

**Why it happens:**
The TypeScript version iterates freely because JS has no borrow semantics. The naive Rust translation:

```rust
// DANGEROUS: holds borrow on reactions while potentially mutating
for reaction in signal.reactions.borrow().iter() {
    reaction.borrow_mut().flags |= DIRTY;  // May panic!
}
```

If `reaction` is itself stored somewhere that requires borrowing the same `RefCell`, you get a panic.

**Consequences:**
- Runtime panic during normal reactive operations
- Non-deterministic failures (depends on graph structure)
- Often appears only with complex dependency graphs, passing simple tests

**Prevention:**
1. **Clone references before iteration:**
   ```rust
   let reactions: Vec<_> = signal.reactions.borrow().iter().cloned().collect();
   drop(signal.reactions.borrow()); // Explicit release
   for reaction in reactions {
       reaction.borrow_mut().flags |= DIRTY;
   }
   ```

2. **Use explicit stack with owned references:**
   ```rust
   let mut stack = vec![(signal.clone(), DIRTY)];
   while let Some((sig, status)) = stack.pop() {
       let reactions = sig.reactions.borrow().clone(); // Clone the Vec
       for reaction in reactions {
           // Safe: no outstanding borrows
       }
   }
   ```

3. **Minimize borrow scope:**
   ```rust
   let reactions_snapshot = signal.borrow().reactions.clone();
   // Now signal borrow is released
   for reaction in reactions_snapshot.unwrap_or_default() {
       // Safe to borrow reaction
   }
   ```

**Detection:**
- Write test with 3+ level dependency chain that mutates during cascade
- Test with diamond dependencies (A -> B, A -> C, B -> D, C -> D)
- Run with `RUST_BACKTRACE=1` to catch borrow panics in dev

**Phase to address:** Phase 2 (Reactivity Engine) - Must solve in `markReactions()` and `updateDerivedChain()`.

---

### Pitfall 2: Reference Cycles Causing Memory Leaks

**What goes wrong:**
Sources hold `Vec<Rc<Reaction>>` for reactions. Reactions hold `Vec<Rc<Source>>` for dependencies. This creates a reference cycle: `Source -> Reaction -> Source`. Neither gets dropped because reference counts never reach zero.

**Why it happens:**
TypeScript uses `FinalizationRegistry` to break cycles when objects are GC'd. Rust has no GC - `Rc` cycles are permanent memory leaks.

**Consequences:**
- Memory grows unboundedly as signals/effects are created and "disposed"
- Disposal functions don't actually free memory
- Long-running applications eventually OOM

**Prevention:**
1. **Use Weak references for back-edges:**
   ```rust
   // Source holds strong refs to reactions (forward edge)
   struct SourceInner<T> {
       reactions: Vec<Weak<RefCell<dyn AnyReaction>>>,  // Weak!
   }

   // Reaction holds strong refs to deps (it needs them to live)
   struct ReactionInner {
       deps: Vec<Rc<RefCell<dyn AnySource>>>,  // Strong
   }
   ```

2. **Choose cycle-breaking direction carefully:**
   - Reactions NEED their dependencies (can't read without them)
   - Sources DON'T need their reactions (reactions subscribe/unsubscribe)
   - Therefore: deps are Strong, reactions are Weak

3. **Clean up dead Weak refs periodically:**
   ```rust
   fn cleanup_dead_reactions(&mut self) {
       self.reactions.retain(|weak| weak.upgrade().is_some());
   }
   ```

4. **Explicit disposal breaks cycles:**
   ```rust
   fn dispose(&mut self) {
       // Remove self from all deps' reaction lists
       for dep in self.deps.drain(..) {
           dep.borrow_mut().remove_reaction(self_weak);
       }
   }
   ```

**Detection:**
- Test: create 1000 effects, dispose all, check memory
- Test: create effect -> dispose -> create effect in loop
- Use memory profiler to detect leaked `Rc` allocations

**Phase to address:** Phase 2 (Reactivity Engine) - Design upfront, implement in types.

---

### Pitfall 3: Type Erasure Breaking Value Access

**What goes wrong:**
You need to store `Source<i32>`, `Source<String>`, `Derived<bool>` in the same `Vec<dyn AnySource>`. But then you can't read the actual value because the type is erased.

**Why it happens:**
Rust's trait objects erase the concrete type. `dyn AnySource` can't have methods that mention `T`:
```rust
trait AnySource {
    fn get_value<T>(&self) -> T;  // ERROR: can't have generic methods on trait objects
}
```

**Consequences:**
- Can store heterogeneous signals but can't read their values
- Must downcast, but downcast to WHAT? Caller doesn't know the type
- Leads to `Any` + `TypeId` hacks that are fragile

**Prevention:**
1. **Separate type-erased operations from typed operations:**
   ```rust
   // Type-erased trait - things that don't need T
   trait AnySource: Any {
       fn flags(&self) -> u32;
       fn set_flags(&mut self, flags: u32);
       fn write_version(&self) -> u64;
       fn as_any(&self) -> &dyn Any;
       fn as_any_mut(&mut self) -> &mut dyn Any;
   }

   // Typed operations stay on the concrete type
   impl<T: 'static> Signal<T> {
       pub fn get(&self) -> T { ... }
       pub fn set(&self, value: T) { ... }
   }
   ```

2. **Only store type-erased where truly needed:**
   - `deps: Vec<Rc<RefCell<dyn AnySource>>>` for heterogeneous deps
   - But keep typed handle: `let sig: Signal<i32> = ...`
   - User always works with typed handles, not erased ones

3. **Downcast pattern for specific needs:**
   ```rust
   fn downcast<T: 'static>(source: &dyn AnySource) -> Option<&SourceInner<T>> {
       source.as_any().downcast_ref::<SourceInner<T>>()
   }
   ```

**Detection:**
- Test: create `Signal<i32>` and `Signal<String>`, store in same Vec, retrieve values
- Compile error is good here - forces you to solve the problem, not hide it

**Phase to address:** Phase 1 (Core Foundation) - Types must be designed correctly from start.

---

### Pitfall 4: MAYBE_DIRTY Optimization Incorrectly Implemented

**What goes wrong:**
The MAYBE_DIRTY optimization is the core performance guarantee. If implemented wrong:
- Effects run when they shouldn't (value didn't change)
- Effects don't run when they should (value changed but marked CLEAN)
- Cascade updates recompute everything (no optimization at all)

**Why it happens:**
The algorithm is subtle:
1. Direct dependency of changed signal: mark DIRTY
2. Indirect dependencies (reactions of reactions): mark MAYBE_DIRTY
3. When MAYBE_DIRTY reaction runs, check if deps actually changed via `write_version`
4. If no dep's `write_version > my_write_version`, become CLEAN without recomputing

Getting the version comparison wrong, or marking DIRTY when should be MAYBE_DIRTY, breaks it.

**Consequences:**
- Performance degradation (cascading unnecessary updates)
- Correctness bugs (missed updates or phantom updates)
- Hard to debug - symptoms look like "sometimes wrong"

**Prevention:**
1. **Port the TypeScript algorithm exactly:**
   ```rust
   fn mark_reactions(signal: &Source, status: u32) {
       let mut stack = vec![(signal.clone(), status)];
       while let Some((sig, stat)) = stack.pop() {
           for reaction in sig.reactions() {
               if (reaction.flags() & DIRTY) == 0 {
                   reaction.set_flags_status(stat);
               }
               if reaction.is_derived() {
                   // MAYBE_DIRTY for indirect deps, not DIRTY!
                   stack.push((reaction.as_source(), MAYBE_DIRTY));
               } else if was_not_dirty {
                   schedule_effect(reaction);
               }
           }
       }
   }
   ```

2. **Write the canonical test cases:**
   ```rust
   #[test]
   fn maybe_dirty_prevents_cascade() {
       let a = signal(1);
       let b = derived(|| a.get() * 2);  // 2
       let c = derived(|| b.get() + 0);  // 2, will return same value
       let runs = Rc::new(RefCell::new(0));
       let runs_clone = runs.clone();
       effect(move || {
           c.get();
           *runs_clone.borrow_mut() += 1;
       });

       flush_sync();
       assert_eq!(*runs.borrow(), 1);  // Initial run

       a.set(1);  // Same value - no change
       flush_sync();
       assert_eq!(*runs.borrow(), 1);  // Should NOT run again

       a.set(2);  // Different value
       flush_sync();
       assert_eq!(*runs.borrow(), 2);  // Should run
   }
   ```

3. **Test the chain behavior:**
   - 10 deriveds in chain, change root
   - If middle derived computes same value, rest should NOT recompute

**Detection:**
- Effect run counter in tests
- Log when derived recomputes vs becomes CLEAN
- Performance benchmarks (slow = probably recomputing unnecessarily)

**Phase to address:** Phase 2-3 (Reactivity Engine + Derived) - Core algorithm lives here.

---

### Pitfall 5: Circular Module Dependencies

**What goes wrong:**
Direct translation of TypeScript structure leads to:
- `tracking.rs` needs `scheduling.rs` (to schedule effects)
- `scheduling.rs` needs `tracking.rs` (to run reactions)
- `derived.rs` needs both (it's Source AND Reaction)
- Rust compiler rejects circular imports

**Why it happens:**
TypeScript allows circular imports at runtime. Rust's module system requires a DAG (directed acyclic graph).

**Consequences:**
- Won't compile
- Forces awkward restructuring
- Previous attempts created "placeholder" functions to break cycles, but those placeholders were never filled in

**Prevention:**
1. **Single module approach:**
   Put all reactivity logic in one `reactivity.rs`:
   ```rust
   // reactivity.rs
   mod tracking { ... }
   mod scheduling { ... }
   // Both can see each other via super::
   ```

2. **Function pointer injection:**
   ```rust
   // tracking.rs
   static SCHEDULE_EFFECT: AtomicPtr<fn(&Reaction)> = ...;

   pub fn set_schedule_effect(f: fn(&Reaction)) {
       SCHEDULE_EFFECT.store(f as *mut _, Ordering::SeqCst);
   }

   // scheduling.rs (on init)
   tracking::set_schedule_effect(schedule_effect);
   ```

3. **Trait-based abstraction:**
   ```rust
   // core.rs
   trait Scheduler {
       fn schedule(&self, reaction: &dyn Reaction);
   }

   // tracking.rs uses dyn Scheduler
   // scheduling.rs implements Scheduler
   ```

**Detection:**
- Compiler error (the good kind - forces you to solve it)
- If you find yourself writing `todo!()` or `unimplemented!()` to break cycles, STOP

**Phase to address:** Phase 1-2 (Module structure must be decided upfront)

---

## Moderate Pitfalls

Mistakes that cause delays, technical debt, or subtle bugs.

---

### Pitfall 6: Forgetting to Drop Borrows Before Nested Operations

**What goes wrong:**
```rust
fn update_derived(derived: &RefCell<DerivedInner>) {
    let inner = derived.borrow();
    let value = (inner.fn)();  // fn might read other signals
    // Still holding borrow on inner!
    inner.value = value;  // Need borrow_mut, but have borrow
}
```

Even if you try `borrow_mut()`, if the computation function reads `derived` itself (self-dependency), you'll panic.

**Prevention:**
```rust
fn update_derived(derived: &RefCell<DerivedInner>) {
    let fn_clone = derived.borrow().fn.clone();
    let value = fn_clone();  // No borrow held during execution
    derived.borrow_mut().value = value;
}
```

**Phase to address:** Phase 3 (Primitives) - All computation functions.

---

### Pitfall 7: Effect Self-Invalidation Infinite Loop

**What goes wrong:**
An effect writes to a signal it reads:
```rust
let count = signal(0);
effect(|| {
    let c = count.get();
    count.set(c + 1);  // Writes to its own dependency!
});
// Infinite loop: effect runs, writes, triggers itself, runs...
```

**Why it happens:**
This is a real user error, but the library must handle it gracefully (error, not hang).

**Prevention:**
The TypeScript version has `MAX_FLUSH_COUNT = 1000` in `flushSync()`:
```rust
const MAX_ITERATIONS: u32 = 1000;
let mut iterations = 0;
while has_pending_effects() {
    iterations += 1;
    if iterations > MAX_ITERATIONS {
        panic!("Maximum update depth exceeded. Effect may be self-invalidating.");
    }
    flush_one_cycle();
}
```

**Phase to address:** Phase 2 (Scheduling) - flushSync implementation.

---

### Pitfall 8: Stale Closure Over Moved Values

**What goes wrong:**
```rust
let count = signal(0);
let doubled = derived(|| count.get() * 2);
drop(count);  // count is gone
doubled.get();  // Tries to read count - undefined behavior or panic
```

**Why it happens:**
Rust closures capture by reference or move. If the source is dropped while a derived still references it, the derived holds a dangling reference (if using raw pointers) or a dead `Weak` (if using Weak).

**Prevention:**
1. Use `Rc`/`Weak` so references are always valid or clearly dead
2. When dereferencing `Weak`, handle `None` gracefully:
   ```rust
   fn get_dep_value(&self) -> Option<...> {
       self.dep.upgrade()?.borrow().value
   }
   ```
3. Test what happens when sources are dropped before their dependents

**Phase to address:** Phase 2-3 (Types and Primitives) - Reference design.

---

### Pitfall 9: Version Counter Overflow

**What goes wrong:**
`write_version` and `read_version` are `u64`. They increment on every write/read cycle. After 2^64 operations, they wrap to 0.

**Why it happens:**
Unlikely in practice (would take millennia at realistic rates), but:
- If you use `u32`, wrap happens after ~4 billion ops (reachable in long-running apps)
- Wrap causes version comparisons to fail: `new_version < old_version` becomes true

**Prevention:**
- Use `u64` (TypeScript uses JS numbers which are f64, effectively ~53 bits)
- For extreme paranoia, use wrapping comparison:
  ```rust
  fn version_greater(a: u64, b: u64) -> bool {
      a.wrapping_sub(b) < (u64::MAX / 2)
  }
  ```

**Phase to address:** Phase 1 (Types) - Version counter types.

---

### Pitfall 10: Thread-Local State Not Actually Thread-Local

**What goes wrong:**
Using `static mut` or a single global for reactive context:
```rust
static mut CONTEXT: Option<Context> = None;  // NOT thread-local!
```

Multiple threads would corrupt shared state.

**Why it happens:**
The library is designed single-threaded (`Rc`, not `Arc`), but if someone uses it from multiple threads, UB ensues.

**Prevention:**
Use `thread_local!`:
```rust
thread_local! {
    static CONTEXT: RefCell<Context> = RefCell::new(Context::new());
}

fn with_context<R>(f: impl FnOnce(&mut Context) -> R) -> R {
    CONTEXT.with(|ctx| f(&mut ctx.borrow_mut()))
}
```

**Phase to address:** Phase 1 (Globals) - Context storage.

---

### Pitfall 11: Equality Function Comparing Closures

**What goes wrong:**
Default equality uses `==`. For `T = Fn() -> i32`, this doesn't work:
```rust
let a: Box<dyn Fn() -> i32> = Box::new(|| 1);
let b: Box<dyn Fn() -> i32> = Box::new(|| 1);
a == b  // Compile error: can't compare functions
```

**Prevention:**
1. Bound `T: PartialEq` for signals that use default equality
2. Provide `neverEquals` for function-valued signals:
   ```rust
   let callback = signal_with_eq(|| {}, never_equals);
   ```
3. Document that function-valued signals need custom equality

**Phase to address:** Phase 3 (Signal primitive) - Equality bounds.

---

## Minor Pitfalls

Mistakes that cause annoyance but are quickly fixable.

---

### Pitfall 12: Forgetting Clone Bound on T

**What goes wrong:**
```rust
impl<T> Signal<T> {
    fn get(&self) -> T {
        self.inner.borrow().value  // ERROR: cannot move out of borrowed
    }
}
```

**Prevention:**
```rust
impl<T: Clone> Signal<T> {
    fn get(&self) -> T {
        self.inner.borrow().value.clone()
    }
}
```

Or use `Rc<T>` for expensive-to-clone types.

**Phase to address:** Phase 3 (All primitives) - API design.

---

### Pitfall 13: Debug Output Triggering Reactive Reads

**What goes wrong:**
```rust
impl<T: Debug> Debug for Signal<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Signal({:?})", self.get())  // Tracks dependency!
    }
}
```

Printing a signal in a debugger creates a dependency, potentially changing behavior.

**Prevention:**
Use `peek()` or raw borrow for debug:
```rust
impl<T: Debug> Debug for Signal<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Signal({:?})", self.inner.borrow().value)  // No tracking
    }
}
```

**Phase to address:** Phase 3 (Primitives) - Debug implementations.

---

### Pitfall 14: Panic in User Callback Leaves Dirty State

**What goes wrong:**
```rust
fn update_effect(effect: &Effect) {
    effect.set_flags(UPDATING);
    (effect.fn)();  // User code panics here
    effect.set_flags(CLEAN);  // Never reached!
}
// Effect stuck in UPDATING state forever
```

**Prevention:**
Use RAII guard or explicit cleanup:
```rust
struct UpdateGuard<'a>(&'a Effect);
impl Drop for UpdateGuard<'_> {
    fn drop(&mut self) {
        self.0.clear_updating_flag();
    }
}

fn update_effect(effect: &Effect) {
    let _guard = UpdateGuard(effect);
    effect.set_flags(UPDATING);
    (effect.fn)();  // If panic, guard runs cleanup
}
```

**Phase to address:** Phase 3 (Effect) - Error handling.

---

## Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| Phase 1: Core Types | Type erasure (Pitfall 3) | Design AnySource trait carefully upfront; test with heterogeneous Vec |
| Phase 1: Globals | Thread-local (Pitfall 10) | Use `thread_local!` macro from start |
| Phase 2: Tracking | Borrow panic in cascade (Pitfall 1) | Clone refs before iteration; explicit stack |
| Phase 2: Tracking | MAYBE_DIRTY wrong (Pitfall 4) | Port TS algorithm exactly; write canonical tests |
| Phase 2: Scheduling | Infinite loop (Pitfall 7) | MAX_ITERATIONS guard |
| Phase 2: Types | Memory leaks (Pitfall 2) | Weak refs for reactions from start |
| Phase 3: Signal | Clone bound (Pitfall 12) | Bound T: Clone or use Rc<T> |
| Phase 3: Derived | Borrow during compute (Pitfall 6) | Clone fn before calling |
| Phase 3: Effect | Panic recovery (Pitfall 14) | RAII guard for flags |
| All Phases | Circular modules (Pitfall 5) | Single reactivity module or function injection |

---

## Sources

- TypeScript source code at `/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals`
- Analysis of 5 failed implementation attempts documented in PROGRESS.md and CONCERNS.md
- Rust borrow checker semantics (core language knowledge)
- `Rc`/`RefCell` panic conditions (standard library documentation)
- Svelte 5 reactivity design (TypeScript source is derived from Svelte 5)

---

*Pitfalls audit: 2026-01-23*
