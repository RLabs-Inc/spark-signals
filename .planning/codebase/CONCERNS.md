# Codebase Concerns

**Analysis Date:** 2026-01-23

---

## Project Status - Critical Blocker

**Current State:**
- Only stub file exists: `src/lib.rs` (10 lines)
- No implementation has been completed
- Benchmark harness is written but will fail to compile (`benches/signals.rs` imports non-existent modules)
- This is a fresh start after 5 failed attempts

**Blocking Issue:**
- Project is currently non-functional. Must implement core reactivity system before ANY other work.
- Benchmarks define API contract but implementation does not exist.

---

## Tech Debt - Historical Pattern

**Pattern from Previous Attempts (Attempts 1-5):**

Five consecutive failures followed a consistent pattern:
1. Initial attempt built too much infrastructure before primitives worked
2. Subsequent attempts left explicit TODO comments everywhere
3. Later attempts used "simplified for now" comments to hide incompleteness
4. Penultimate attempt wrote placeholder functions that looked complete but did nothing
5. Final attempt added nice documentation describing what code "should do" while actual code was hollow

**Core Issue:** Each attempt got better at *hiding* incompleteness rather than *solving* the hard problems.

**Implications:**
- Cannot use incremental progress without verification
- Must solve known hard problems completely BEFORE moving forward
- Code that "looks correct" may be functionally empty

**Files Affected:**
- All previous attempt code (now deleted, existed in earlier branches)

---

## Known Hard Problems (Blocking Implementation)

These problems caused all previous implementation failures. Solutions must be solved BEFORE coding primitives.

### 1. Type Erasure in Heterogeneous Collections

**Problem:**
- Must store `Source<T>` values of different types in same container (`Vec<...>`)
- Dependencies list holds `Signal<i32>`, `Signal<String>`, `Derived<bool>`, etc.
- Cannot downcast without runtime type information

**Files Affected:**
- `src/core/types.rs` (to be created) - Must define trait hierarchy
- `src/reactivity/tracking.rs` (to be created) - Dependency collection and lookup

**Current Impact:**
- Cannot implement `Source` interface with generic value field in a way that stores mixed types
- The TraitObject approach in Rust requires careful lifetime and pointer management

**Partial Solutions in Design:**
- `ARCHITECTURE.md` proposes:
  ```rust
  type RcSource = Rc<RefCell<dyn AnySource>>;
  ```
- But `AnySource` trait definition needs to work across all possible `T` types
- Must solve: how to store `T` value internally while type-erasing for collections

**Fix Approach:**
1. Define base trait `AnySource` with operations that don't require `T` knowledge (flags, version counters)
2. Define typed trait `TypedSource<T>` extending `AnySource` with value-specific ops
3. Use downcast pattern: `as_any()` + `TypeId` for runtime type information
4. Test on simple case: two signals of different types stored in `Vec<Rc<RefCell<dyn AnySource>>>`

---

### 2. Circular Module Dependencies

**Problem:**
- `tracking.rs` needs functions from `scheduling.rs` (to schedule effects when dirty)
- `scheduling.rs` needs functions from `tracking.rs` (to run reactions when scheduled)
- `derived.rs` needs both (it IS a source AND a reaction)
- Rust forbids circular module imports

**Files Affected (to be created):**
- `src/reactivity/tracking.rs` - Core dependency tracking
- `src/reactivity/scheduling.rs` - Effect scheduling
- `src/primitives/derived.rs` - Derived implementation

**Current Impact:**
- Cannot structure code following TypeScript module organization
- Must reorganize module hierarchy OR use internal module structure

**Design Solutions in ARCHITECTURE.md:**
1. Single monolithic `reactivity.rs` module (consolidate tracking + scheduling)
2. Pull scheduling into tracking as private functions
3. Use `pub(crate)` visibility to manage boundaries

**Fix Approach:**
1. Create `src/reactivity/mod.rs` as private module containing all coordination logic
2. Split into `tracking`, `scheduling`, `batching` as internal submodules
3. Only export high-level `get()`, `set()`, `batch()` functions
4. Verify circular dependencies disappear at compile time

---

### 3. Borrow Rules in Cascading Updates

**Problem:**
In `markReactions()` / `cascade_maybe_dirty()` algorithm:
```
while stack not empty:
    (signal, status) = stack.pop()
    for reaction in signal.reactions:  // Borrow signal.reactions
        if not DIRTY:
            setSignalStatus(reaction, status)  // May mutate reaction!
        if reaction is DERIVED:
            stack.push((reaction, MAYBE_DIRTY))  // Add to stack while iterating!
```

**Issue:** Cannot iterate over `signal.reactions` while potentially modifying it (if a reaction being processed changes its dependency graph).

**Files Affected (to be created):**
- `src/reactivity/tracking.rs` - markReactions() function
- Any code that mutates dependency graphs during traversal

**Current Impact:**
- Cannot implement core dirty propagation algorithm
- Rust borrow checker will reject the straightforward TypeScript translation

**Fix Approach:**
1. **Explicit stack approach**: Instead of recursive or iterator, use explicit `Vec` stack
   - Pop from stack, process, push new items
   - Doesn't require holding reference to `reactions` while mutating
2. **Clone reaction refs before processing**: If needed, clone `Rc<>` references out of array
3. **Deferred mutations**: Collect mutations, apply after traversal
4. **Test first**: Write test with 3+ level dependency chain that mutates during cascade

---

## Missing Critical Features

### Core Reactivity Engine

**What's Missing:**
- Entire reactive graph system
- Dependency tracking (get/set)
- Dirty propagation algorithm
- Effect scheduling

**Impact:**
- Cannot read or write signals
- Cannot create effects or derived signals
- Application cannot respond to changes

**Blocks:**
- ALL other features
- Benchmarks will not compile
- Cannot test anything

**Priority:** CRITICAL - BLOCKING

**Implementation in PROGRESS.md suggests phases:**

Phase 1: Core Foundation (flags, types, globals, constants)
Phase 2: Reactivity Engine (get/set, scheduling, batching)
Phase 3: Primitives (signal, derived, effect)
Phase 4+: Advanced primitives and collections

---

## Test Coverage Gaps

### No Tests Exist

**What's Not Tested:**
- Any functionality
- No unit tests
- No integration tests
- No property-based tests

**Files:**
- No test directory exists
- `src/lib.rs` is empty

**Risk:**
- CRITICAL: Cannot verify core algorithms work
- CRITICAL: Cannot catch regressions on MAYBE_DIRTY optimization (the key correctness guarantee)
- HIGH: Cannot validate borrow rules in cascade_maybe_dirty until tests exist
- HIGH: Memory leaks possible without cycle detection tests

**Ported Test Requirements:**
The TypeScript implementation at:
```
/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals
```
has existing tests that should be ported:
- Core primitives tests
- MAYBE_DIRTY optimization verification
- Dependency tracking tests
- Memory cycle tests

---

## Design Complexity - MAYBE_DIRTY Optimization

**Critical Algorithm Not Yet Implemented:**

The MAYBE_DIRTY optimization is the core correctness and performance guarantee:

1. Signal A changes → Derived B marked **DIRTY**
2. Derived C (depends on B) marked **MAYBE_DIRTY** (not fully DIRTY!)
3. Effect E (depends on C) marked **MAYBE_DIRTY**

When E runs:
- C is MAYBE_DIRTY, so check B first
- B is DIRTY, recompute B
- If B's value unchanged → C becomes CLEAN → E doesn't run
- If B's value changed → recompute C → check if C changed → etc.

**Why Fragile:**
- Requires precise version tracking (write_version per signal, read_version per reaction)
- Must correctly implement `cascade_maybe_dirty()` without triggering unnecessary updates
- MAYBE_DIRTY → CLEAN propagation is non-obvious

**Implementation Location:**
- `src/reactivity/tracking.rs` - updateDerivedChain() (lines 291-310 in ARCHITECTURE.md)

**Risk:**
- If implemented incorrectly: cascading updates trigger unnecessarily
- If version tracking is wrong: effects run when they shouldn't, or don't run when they should
- Hard to debug: symptoms are performance degradation or missed updates

**Test Plan:**
Must include:
```
1. Signal -> Derived (no change) -> Effect
   - Signal changes, Derived recomputes but value stays same
   - Effect should NOT run

2. Signal -> Derived1 -> Derived2 -> Effect
   - Signal changes, Derived1 value unchanged
   - Derived2 should be marked CLEAN (not trigger recompute)
   - Effect should NOT run

3. Chain of 10 deriveds, only 3rd changed
   - 4th-10th should become CLEAN without recomputing
```

---

## Edition Anomaly

**Issue:**
`Cargo.toml` specifies:
```toml
edition = "2024"
```

**Problem:**
Rust editions are: 2015, 2018, 2021. There is no 2024 edition.

**Impact:**
- HIGH: Build will fail
- Cargo will reject this during compilation

**Files:**
- `Cargo.toml` (line 4)

**Fix:**
Change to `edition = "2021"` (current stable edition as of Feb 2025)

---

## Benchmark API Contract vs Empty Implementation

**Situation:**
`benches/signals.rs` defines comprehensive benchmarks calling:
- `signal(0i32)`
- `derived(|| ...)`
- `effect(|| {})`
- `batch(|| { ... })`

**Problem:**
These functions don't exist yet. Benchmarks will fail to compile until implementation is complete.

**Impact:**
- MEDIUM: Cannot verify performance
- Cannot compare with TypeScript baseline (benchmark suite designed for comparison)

**Files:**
- `benches/signals.rs` - 370 lines of benchmark code depending on non-existent API

**Dependency:**
- Must fully implement phases 1-3 before benchmarks can run
- Benchmarks are tests of final implementation, not incremental verification

---

## Deployment & Version

**Not Applicable:**
This is a library-phase project. No deployment concerns yet.

**Future Concerns:**
- Edition 2021+ required (currently broken)
- Licensing: MIT specified in Cargo.toml (OK)
- Repository not yet pushed to GitHub (expected for fresh start)

---

## Dependencies

**Current:**
- Zero dependencies in main library
- `criterion` for benchmarks (acceptable)

**Concern:**
- No external dependencies means all algorithms must be implemented from scratch
- No reference implementations available to guide design (must stay faithful to TypeScript spec)
- Type erasure and trait objects are non-trivial patterns that need careful implementation

---

## Memory & Lifecycle Management

**Design Decision:**
Using `Rc<RefCell<T>>` for memory management (not Arc/RwLock in default feature)

**Concerns:**
1. **Reference Cycles Possible**: Can create cycles in dependency graph
   - TypeScript uses `FinalizationRegistry` for cleanup
   - Rust must use `Weak<>` references strategically
   - **Not Yet Designed**: Which back-references should be Weak?

2. **Runtime Borrow Errors**: `RefCell` panics on borrow conflicts
   - If two parts of code borrow same data simultaneously, panic at runtime
   - Must carefully control borrow lifetime in nested operations
   - **Risky in**: cascade_maybe_dirty(), updateReaction() chains

3. **Sync Feature**: Optional `sync` feature for `Arc<RwLock<T>>`
   - Not yet implemented
   - Would require separate code paths or generic trait approach
   - **Design Question**: How to switch between Rc and Arc at compile time?

**Files to Consider:**
- `src/core/types.rs` (to be created) - Must specify which references are Weak
- `src/reactivity/tracking.rs` (to be created) - Must manage borrow lifetimes carefully

---

## Architecture Decision Documentation

**Current State:**
- Design is well-documented in `ARCHITECTURE.md`
- Design identifies the hard problems
- Design suggests solutions

**Risk:**
- Design is aspirational, not yet validated
- The solutions suggested (type erasure with AnySource, module consolidation, explicit stacks) may have issues not apparent until coding
- Previous attempts may have encountered issues with these specific design solutions

**Recommendation:**
- When implementing, validate design assumptions incrementally
- Document any design changes that become necessary during implementation
- Keep ARCHITECTURE.md updated with actual decisions made

---

## Summary: Risk Matrix

| Area | Severity | Status | Dependency |
|------|----------|--------|------------|
| Type Erasure | CRITICAL | Unsolved | Blocks all code |
| Circular Dependencies | CRITICAL | Design only | Blocks structure |
| Borrow Rules in Cascade | CRITICAL | Design only | Blocks dirty propagation |
| Edition 2024 Error | HIGH | Ready to fix | Blocks build |
| No Implementation | CRITICAL | Starting fresh | Blocks everything |
| No Tests | CRITICAL | Not started | Blocks validation |
| MAYBE_DIRTY Optimization | HIGH | Design only | Blocks correctness |
| Benchmark Compilation | MEDIUM | Deferred | Deferred to phase 3+ |
| Reference Cycles | MEDIUM | Not yet designed | Post phase 2 |
| Sync Feature | MEDIUM | Not yet designed | Post phase 3 |

---

*Concerns audit: 2026-01-23*
