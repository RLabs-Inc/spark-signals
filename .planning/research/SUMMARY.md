# Research Summary: Spark Signals Rust Port

**Domain:** Reactive signals library (TypeScript to Rust port)
**Researched:** 2026-01-23
**Overall confidence:** MEDIUM

## Executive Summary

This research focused on the THREE HARD PROBLEMS that caused 5 failed implementation attempts when porting @rlabs-inc/signals from TypeScript to Rust:

1. **Type Erasure** - Storing `Source<T>` of different types in heterogeneous collections
2. **Circular Dependencies** - tracking.rs needs scheduling.rs needs tracking.rs
3. **Borrow Rules in Cascade** - Mutating the reactive graph while traversing it

Each problem has been analyzed with multiple solution approaches from established Rust reactive libraries (Leptos, Dioxus, Sycamore, futures-signals). Specific recommendations are provided that match our constraints (Rule Zero: `Rc<RefCell<T>>`, no lifetime annotations).

## Key Findings

**Stack:** Pure Rust with `Rc<RefCell<T>>` for interior mutability, trait objects for type erasure, thread-local context for globals. Zero dependencies.

**Architecture:** Trait-based type erasure with careful RefCell scoping. Direct port of TypeScript's existing dependency injection pattern for circular dependencies.

**Critical pitfall:** RefCell borrow conflicts during graph traversal - MUST collect references before mutating, never hold borrows across mutations.

## Implications for Roadmap

Based on research, suggested phase structure:

1. **Phase 1: Core Foundation** - flags, types, context
   - Addresses: Type erasure infrastructure (AnySource, AnyReaction traits)
   - Low risk: Well-established Rust patterns

2. **Phase 2: Basic Reactivity** - Signal + get/set
   - Addresses: Prove type erasure works end-to-end
   - Tests: Create signals, read/write values, verify trait object storage

3. **Phase 3: Dependency Tracking** - tracking.rs with mark_reactions
   - Addresses: Borrow rule challenges
   - Critical: Must verify RefCell scoping patterns work

4. **Phase 4: Derived** - MAYBE_DIRTY optimization
   - Addresses: Circular dependency (tracking needs derived)
   - Tests: Deep derived chains, diamond dependencies

5. **Phase 5: Effects & Scheduling** - Complete circular dep injection
   - Addresses: Full reactive loop
   - Tests: Effects triggering effects, batch updates

**Phase ordering rationale:**
- Each phase proves one hard problem before building on it
- Type erasure must work before tracking can store deps
- Tracking must work before derived can use it
- Circular dep injection only needed once derived and effects exist

**Research flags for phases:**
- Phase 3: Likely needs deeper research (RefCell scoping edge cases)
- Phase 4: Standard patterns once Phase 3 verified
- Phase 5: May need research on callback scheduling without microtasks

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Type erasure approach | HIGH | Trait objects are well-established Rust pattern |
| Circular dep solution | HIGH | Direct port of TypeScript's existing pattern |
| Borrow rules solution | MEDIUM | Requires careful implementation, verify with tests |
| Overall feasibility | HIGH | All problems have known solutions |

## Gaps to Address

- **Microtask replacement:** Rust has no microtasks. Need to decide: callback queue + manual flush, or async runtime integration.
- **Performance:** Trait object overhead not benchmarked vs arena approach. May need revisiting if hot path.
- **FinalizationRegistry replacement:** TypeScript uses this for GC-triggered cleanup. Rust uses Drop trait - should work but untested.
- **Async effect support:** TypeScript handles `await` in effects specially. May need phase-specific research.

## Sources

- TypeScript source code at `/Users/rusty/Documents/Projects/AI/Tools/ClaudeTools/memory-ts/packages/signals`
- Training data on Leptos, Dioxus, Sycamore, futures-signals architectures (MEDIUM confidence - not web-verified)
- Existing `.planning/ARCHITECTURE.md` analysis document
