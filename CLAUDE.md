# Spark Signals - Rust Port

A faithful port of `@rlabs-inc/signals` TypeScript package to Rust.

---

## Source Reference

The TypeScript implementation to port:
```
/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals
```

**The TypeScript code is the spec.** Port it faithfully, don't simplify it.

---

## Rule Zero - Write Rust Like Functional TypeScript

```rust
// What our code should look like:
let count = signal(0);
let doubled = derived(|| count.get() * 2);
effect(|| println!("{}", doubled.get()));
count.set(5);

// NOT this:
fn create_signal<'a, T: 'static + Clone>(
    ctx: &'a mut ReactiveContext<'a>,
    value: T
) -> SignalHandle<'a, T> where T: PartialEq { ... }
```

**If you're writing lifetime annotations, STOP. You're on the wrong path.**

**The Rust tax we accept (and ONLY this):**
- `.borrow()` / `.borrow_mut()`
- `Rc::new()` / `RefCell::new()`
- `.clone()` when passing ownership

---

## The Three Rules (Non-Negotiable)

### 1. Faithful Port - No Simplification
- Port the TypeScript structure exactly
- If TS has flags, we have flags
- If Derived is both Source AND Reaction, ours is too
- Don't "simplify" - that's how we broke it before

### 2. No Placeholders, No Shortcuts
- If you hit a hard problem, STOP and solve it
- Don't write "simplified for now" or "placeholder" code
- Don't move forward until the current piece actually works
- Comments that describe what code "should do" while the code doesn't do it = failure

### 3. Honest About Blockers
- When stuck, say "I'm stuck on X, let me think"
- Don't paper over complexity with nice-looking but hollow code
- Ask for help rather than hiding incompleteness

---

## TypeScript Structure to Port

```
src/
├── core/
│   ├── constants.ts   # Flags: CLEAN, DIRTY, MAYBE_DIRTY, etc.
│   ├── types.ts       # Interfaces: Source, Reaction, Derived, Effect
│   └── globals.ts     # Thread-local state: activeReaction, writeVersion, etc.
├── reactivity/
│   ├── tracking.ts    # get(), set(), dependency tracking (THE CORE)
│   ├── scheduling.ts  # Effect scheduling, flushSync()
│   └── batching.ts    # batch(), untrack()
├── primitives/
│   ├── signal.ts      # signal(), source()
│   ├── derived.ts     # derived() with MAYBE_DIRTY optimization
│   ├── effect.ts      # effect(), effect.sync(), effect.root()
│   └── ... (bind, slot, linked, selector, scope)
└── collections/
    ├── map.ts         # ReactiveMap
    ├── set.ts         # ReactiveSet
    └── vec.ts         # (Rust addition) ReactiveVec
```

---

## Critical Architecture Points

### Derived is BOTH Source AND Reaction
```typescript
// In TypeScript:
export interface Derived<T> extends Source<T>, Reaction { ... }
```
A derived can be read (it's a Source with a value) AND it responds to changes (it's a Reaction with dependencies). This dual nature is essential for MAYBE_DIRTY to work.

### The MAYBE_DIRTY Optimization
This is the key optimization we kept missing:

1. Signal A changes → Derived B (depends on A) marked **DIRTY**
2. Derived C (depends on B) marked **MAYBE_DIRTY** (not DIRTY!)
3. Effect E (depends on C) marked **MAYBE_DIRTY**

When E runs:
- C is MAYBE_DIRTY, so check B first
- B is DIRTY, recompute B
- If B's value unchanged → C becomes CLEAN → E doesn't run!
- If B's value changed → recompute C → check if C changed → etc.

This avoids cascading updates when intermediate values don't actually change.

### Known Hard Problems to Solve
These are the problems we kept papering over. They need real solutions:

1. **Type erasure**: How to store `Source<T>` in a `Vec<dyn SomeTrait>`
2. **Circular module deps**: tracking.ts ↔ scheduling.ts ↔ derived.ts
3. **Borrow rules in cascade_maybe_dirty**: Need to iterate reactions while potentially mutating them

---

## Previous Attempts - What Went Wrong

1. **Attempt 1**: Too much infrastructure before primitives worked
2. **Attempt 2**: Left explicit TODOs everywhere
3. **Attempt 3**: "Simplified for now" comments hiding incompleteness
4. **Attempt 4**: Placeholder functions that looked complete but did nothing
5. **Attempt 5**: Nice documentation describing what code "should do" while actual code was hollow

Pattern: Each attempt got better at HIDING the incompleteness rather than SOLVING it.

---

## Next Session: GSD Approach

Starting fresh with `/gsd:new-project` to get proper structure, research phase, and verification loops.

The GSD approach will help because:
- Research phase forces understanding before coding
- Phase verification catches hollow implementations
- Atomic commits mean we can't hide incomplete work
