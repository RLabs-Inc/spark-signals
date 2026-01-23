# Feature Landscape: Rust-Specific Reactive Features

**Domain:** Rust reactive signals library (port from TypeScript)
**Researched:** 2026-01-23
**Confidence:** MEDIUM (based on training data from May 2025; WebSearch/Context7 unavailable)

## Executive Summary

Rust reactive libraries (Leptos, Dioxus, Sycamore) offer several features that TypeScript developers would not expect but Rust developers would. The key categories are:

1. **Copy semantics for signals** - Leptos's flagship feature
2. **Async integration** - Resources, Actions, Suspense primitives
3. **Thread-safe variants** - `Send + Sync` signal types
4. **RAII cleanup** - Drop-based lifecycle instead of manual dispose
5. **Zero-cost abstractions** - Arena allocation, compile-time optimization

Our primary TypeScript-like API already provides the core functionality. The question is which Rust-specific features to add as the secondary API surface.

---

## Table Stakes for Rust Developers

Features Rust developers expect. Missing = library feels un-Rusty.

| Feature | Why Expected | Complexity | Recommendation |
|---------|--------------|------------|----------------|
| **Drop-based cleanup** | RAII is fundamental to Rust | Low | MUST HAVE - Replace dispose callbacks with Drop impl |
| **Clone + Default traits** | Standard Rust trait implementations | Low | MUST HAVE - Derive where possible |
| **Debug impl** | Every Rust type should be debuggable | Low | MUST HAVE - Derive or implement |
| **#[must_use] attributes** | Warn when signals/effects ignored | Low | MUST HAVE - Standard Rust pattern |
| **Send + Sync bounds documentation** | Clarity on thread safety | Low | MUST HAVE - Document which types are !Send/!Sync |
| **Result/Option integration** | Handle errors idiomatically | Low | MUST HAVE - `.try_get()`, `.try_set()` variants |
| **Iterator integration** | ReactiveVec/ReactiveMap should be iterable | Medium | SHOULD HAVE - Implement standard iterator traits |
| **Serde integration (optional)** | Serialization is common need | Medium | SHOULD HAVE - Feature-gated serde support |
| **no_std support** | Embedded/WASM use cases | Medium | NICE TO HAVE - Can add later |

### Drop-Based Cleanup (CRITICAL)

**TypeScript pattern:**
```typescript
const dispose = effect(() => console.log(count.value))
// Later:
dispose()
```

**Rust expectation:**
```rust
{
    let _effect = Effect::new(|| println!("{}", count.get()));
    // Effect runs while _effect is in scope
} // Automatically cleaned up when dropped
```

This is non-negotiable for Rust. Manual dispose functions feel foreign.

**Implementation:** Store `Rc<RefCell<EffectInner>>` in a wrapper that calls `destroyEffect` on Drop.

### Result/Option Integration

Rust developers expect error handling to be explicit:

```rust
// Fallible read (for potentially uninitialized signals)
let value: Option<T> = signal.try_get();

// With mapping
let doubled = signal.get_or(0) * 2;

// Fallible derived
let result = derived(|| -> Result<T, E> { ... });
```

---

## Differentiators

Features that would delight Rust developers. Not expected but valued.

### Tier 1: High Value, Moderate Complexity

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Copy signals (Leptos-style)** | Zero-clone ergonomics | High | Major ergonomic win, requires arena allocation |
| **Async resources** | First-class async in signals | High | `Resource<T>` that tracks async state |
| **Macro-free API first** | Works without proc macros | Low | Already planned - Rust-idiomatic secondary API |

### Tier 2: Medium Value, Lower Complexity

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **with()/update() combinators** | FP-style signal manipulation | Low | `.with(\|v\| v * 2)` instead of `.get() * 2` |
| **Mapped signals** | Create derived with transform | Low | `signal.map(\|v\| v * 2)` returns derived |
| **watch() helper** | Explicit dependency + callback | Low | More explicit than effect() |
| **on() selector** | Track specific signal in effect | Low | Like SolidJS's `on()` |

### Tier 3: Lower Priority

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Thread-safe signals** | `Send + Sync` variants | High | Requires Arc<Mutex> internally |
| **Arena-allocated signals** | Performance optimization | High | Leptos's approach |
| **Compile-time tracking** | Zero-runtime overhead | Very High | Would require proc macros |

---

## Copy Signals Deep Dive

**What Leptos does:**

Leptos signals are `Copy` - you can pass them around without `.clone()`:

```rust
// Leptos style
let count = create_signal(0);
let doubled = create_memo(move || count() * 2); // No .clone()!
spawn(move || println!("{}", count())); // Still no .clone()!
```

**How it works:**
- Signals are just IDs (indices) into a runtime arena
- The actual data lives in thread-local storage
- Signal "values" are essentially smart pointers that copy cheaply

**Tradeoffs:**
- PRO: Incredible ergonomics, feels like magic
- CON: Requires runtime/arena management
- CON: Less predictable cleanup (arena-based, not RAII)
- CON: Harder to reason about (data not where you expect)

**Recommendation:** Do NOT implement Copy signals in core library.

Rationale:
1. We prioritize faithful TypeScript port over Rust idioms
2. Copy signals require fundamentally different architecture (arena)
3. `Rc<RefCell<T>>` with `.clone()` is acceptable Rust tax
4. Can add arena-based variant later as opt-in feature

---

## Async Resource Pattern Deep Dive

All major Rust frameworks have async primitives that TypeScript libraries don't:

**Leptos:**
```rust
let user = create_resource(
    || user_id.get(),  // Source signal
    |id| fetch_user(id) // Async fetcher
);

// In component:
match user.get() {
    None => "Loading...",
    Some(Ok(user)) => user.name,
    Some(Err(e)) => format!("Error: {e}"),
}
```

**Dioxus:**
```rust
let user = use_resource(|| async move {
    fetch_user(user_id()).await
});
```

**Pattern breakdown:**
- Resource tracks: source signal, async fetch function, current state
- States: Unresolved, Pending, Resolved(T), Rejected(E)
- Automatically refetches when source changes

**Recommendation:** Add Resource as Rust-specific addition (post-core).

This is genuinely useful and doesn't exist in the TypeScript source. Would look like:

```rust
let user = Resource::new(
    || user_id.get(),
    |id| async move { fetch_user(id).await }
);

// Reading returns ResourceState<T, E>
match user.get() {
    ResourceState::Unresolved => "Not started",
    ResourceState::Pending => "Loading...",
    ResourceState::Resolved(user) => user.name.as_str(),
    ResourceState::Error(e) => "Error",
}
```

---

## Anti-Features

Features to explicitly NOT implement. Common in Rust reactive libraries but wrong for this project.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **JSX-style macros** | We're not a UI framework | Keep API pure Rust, no DSL |
| **Component system** | Scope creep, not our domain | Focus on signals, let others build components |
| **Virtual DOM / rendering** | Not our concern | Pure reactivity, no rendering |
| **Server functions / RPC** | Framework feature, not signals | Stay focused on reactive primitives |
| **Routing** | Framework feature | Not our domain |
| **Copy signals by default** | Requires arena architecture | Offer `Rc<RefCell>` as default, arena as opt-in later |
| **Implicit global runtime** | Hidden state is un-Rusty | Explicit context, thread-local for convenience |
| **Async-only effects** | TS source has sync effects | Keep both sync and async patterns |
| **Heavy proc macro usage** | Compile time cost, magic | Prefer runtime + simple declarative macros |

### Why No Copy Signals By Default

This deserves emphasis. While Leptos's Copy signals are elegant:

1. **Architecture mismatch:** Our TypeScript source uses object references, not arena indices
2. **Faithful port principle:** Changing fundamental data model breaks "faithful port"
3. **RAII conflict:** Arena allocation means data outlives handles; Drop cleanup is less intuitive
4. **Complexity:** Arena requires careful lifetime management, generation counters for safety

The Rust tax of `.clone()` on `Rc` is acceptable:
```rust
let count = signal(0);
let count2 = count.clone(); // Explicit, clear, Rust-like
effect(move || println!("{}", count2.get()));
```

---

## Feature Dependencies

```
Core (must work first):
  signal() ──┬── derived() ──── effect()
             │
             └── batch() / untrack()

After core:
  signal()
     │
     ├── .map() / .with() (convenience methods)
     │
     ├── ReactiveVec / ReactiveMap / ReactiveSet (collections)
     │
     └── Resource (async, requires effect system)

Independent additions:
  - Serde support (feature-gated)
  - Debug impls
  - Iterator impls
```

---

## MVP Feature Set

For initial release, prioritize:

### Must Have (Table Stakes)
1. All TypeScript primitives ported (signal, derived, effect, batch, untrack)
2. Drop-based cleanup (RAII)
3. Debug impls
4. Clone impls
5. Documentation of thread-safety (!Send, !Sync)

### Should Have (Ergonomics)
1. `.with()` / `.update()` combinators
2. `.map()` for creating derived from signal
3. `watch()` helper
4. Result/Option integration

### Defer to Post-MVP
1. Resource (async) - complex, can add later
2. Serde support - feature gate, add later
3. Thread-safe variants - design for, implement later
4. Arena/Copy signals - fundamentally different architecture

---

## Comparison: What Other Rust Libraries Offer

| Feature | Leptos | Dioxus | Sycamore | Spark (Planned) |
|---------|--------|--------|----------|-----------------|
| Basic signals | Yes | Yes | Yes | Yes |
| Derived/Memo | Yes | Yes | Yes | Yes |
| Effects | Yes | Yes | Yes | Yes |
| Copy signals | Yes | No | No | No (Rc) |
| Async Resource | Yes | Yes | Yes | Post-MVP |
| Collections | No | No | No | Yes (TS parity) |
| Bindings | No | No | No | Yes (TS parity) |
| Slots | No | No | No | Yes (TS parity) |
| Selector | No | No | No | Yes (TS parity) |
| EffectScope | Partial | No | Yes | Yes (TS parity) |
| Batching | Yes | Yes | Yes | Yes |
| Deep reactivity | No | No | No | Yes (TS parity) |

**Key insight:** We have MORE features than other Rust libraries because we're porting a complete signals library. The TypeScript source includes advanced primitives (bind, slot, linkedSignal, selector) that Rust UI frameworks don't have.

Our differentiator is **completeness**, not Rust-specific features.

---

## Rust-Idiomatic API Design

For the secondary Rust-idiomatic API:

### Naming Conventions

| TypeScript | Rust-Idiomatic |
|------------|----------------|
| `signal(value)` | `Signal::new(value)` |
| `derived(fn)` | `Derived::new(fn)` or `Memo::new(fn)` |
| `effect(fn)` | `Effect::new(fn)` |
| `effect.sync(fn)` | `Effect::sync(fn)` or `SyncEffect::new(fn)` |
| `.value` getter | `.get()` method |
| `.value` setter | `.set(value)` method |

### Builder Pattern Option

```rust
Signal::new(0)
    .with_equals(my_equals_fn)
    .with_name("count") // For debugging
    .build()
```

### Trait-Based Design

```rust
trait Readable<T> {
    fn get(&self) -> T;
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R;
}

trait Writable<T>: Readable<T> {
    fn set(&self, value: T);
    fn update(&self, f: impl FnOnce(&mut T));
}
```

---

## Sources and Confidence

| Claim | Confidence | Basis |
|-------|------------|-------|
| Leptos uses Copy signals via arena | HIGH | Well-documented, widely discussed |
| Dioxus uses hooks pattern | HIGH | Official documentation pattern |
| Resource pattern is common | HIGH | All three frameworks have it |
| Drop-based cleanup expected | HIGH | Fundamental Rust idiom |
| Copy signals require arena | HIGH | Fundamental to how they work |
| Specific API details | MEDIUM | Based on training data, may have changed |

**Gaps:**
- Exact current APIs of Leptos/Dioxus/Sycamore (may have evolved since May 2025)
- Performance characteristics of different approaches
- Community preferences/trends

**Recommendation:** Before finalizing Rust-specific additions, verify current state of these libraries via their documentation.

---

## Summary for Roadmap

1. **Phase 1-3:** Focus on TypeScript parity (core primitives)
   - Add Rust table stakes: Drop, Clone, Debug, #[must_use]

2. **Phase 4:** Add Rust ergonomics
   - `.with()`, `.update()`, `.map()` combinators
   - `watch()` helper
   - Result/Option integration

3. **Post-MVP:** Consider Rust-specific additions
   - Resource (async integration)
   - Serde support (feature-gated)
   - Thread-safe variants (if demand exists)

4. **Explicitly NOT planned:**
   - Copy signals (architecture mismatch)
   - UI framework features (not our domain)
   - Heavy proc macros (prefer simplicity)
