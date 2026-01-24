# Spark Signals - Progress

## Status: Starting Fresh with GSD

After 5 attempts that each failed in different ways, we're starting fresh using the GSD approach for structure and verification.

### What We Learned

Each attempt got better at *hiding* incompleteness rather than *solving* it:
1. Explicit TODOs everywhere
2. "Simplified for now" comments
3. Placeholder functions
4. Nice documentation masking hollow code

The pattern: hitting hard problems → papering over them → moving forward → broken implementation.

### The Hard Problems That Need Real Solutions

1. **Type erasure**: `Source<T>` → `dyn ReactiveNode` storage
2. **Circular deps**: tracking ↔ scheduling ↔ derived
3. **Borrow rules in cascading**: iterate + mutate reactions

### Next Session

Run `/gsd:new-project` to:
- Research the TypeScript implementation properly
- Design solutions to the hard problems BEFORE coding
- Use phase verification to catch hollow implementations
- Make atomic commits that can't hide incomplete work

---

## Reference: TypeScript Source

```
/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals
```

Key files to study:
- `src/core/types.ts` - The interfaces
- `src/core/constants.ts` - The flags
- `src/core/globals.ts` - Thread-local state
- `src/reactivity/tracking.ts` - THE CORE (get/set/MAYBE_DIRTY)
- `src/primitives/derived.ts` - Derived implementation
