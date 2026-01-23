# Testing Patterns

**Analysis Date:** 2026-01-23

This document describes testing patterns observed in the TypeScript reference implementation (`@rlabs-inc/signals`) that should be ported to Rust (`spark-signals`). The Rust implementation uses different tools but follows analogous patterns.

## Test Framework

**Runner (TypeScript):**
- Bun's built-in test runner (`bun:test`)
- No external test framework dependency

**Runner (Rust equivalent):**
- Rust's built-in `#[test]` macro (no dependency)
- Or: `cargo test` framework
- Benchmarks: Criterion.rs (already configured in `Cargo.toml`)

**Assertion Library (TypeScript):**
- Bun's built-in `expect()` from `bun:test`
- Pattern: `expect(actual).toBe(expected)` or similar

**Assertion Library (Rust equivalent):**
- Rust's built-in `assert_eq!`, `assert!` macros
- Or: Common assertion crates (no preference indicated in codebase yet)

**Run Commands (TypeScript reference):**
```bash
bun test                    # Run all tests
bun test test/unit          # Run unit tests only
bun test test/integration   # Run integration tests
bun test test/performance   # Run performance tests
```

**Run Commands (Rust equivalent):**
```bash
cargo test                  # Run all tests
cargo test --lib           # Run library tests only
cargo test --test '*'      # Run integration tests
cargo bench                # Run benchmarks (criterion)
```

## Test File Organization

**Location (TypeScript):**
- Co-located with source: `src/primitives/signal.ts` → `test/unit/signal.test.ts`
- Test directories mirror source structure: `test/unit/`, `test/integration/`, `test/performance/`

**Location (Rust equivalent):**
- Inline tests: `src/lib.rs` using `#[cfg(test)]` modules
- Separate integration tests: `tests/` directory at crate root
- Benchmarks: `benches/` directory (already established in `benches/signals.rs`)

**Naming (TypeScript):**
- Test files: `[module].test.ts` or `[module].spec.ts`
- Test suites: `describe()` blocks
- Individual tests: `it()` blocks

**Naming (Rust equivalent):**
- Inline test functions: `#[test] fn test_signal_creates_with_initial_value() {}`
- Integration test files: Any `.rs` file in `tests/`
- Benchmark functions: `fn bench_signal_create(c: &mut Criterion) {}`
- Test module naming: `mod tests { #[test] fn ... }`

## Test Structure

**TypeScript Suite Organization:**

```typescript
import { describe, it, expect } from 'bun:test'
import { signal, effect, derived, batch, flushSync, untrack } from '../../src/index.js'

describe('signal', () => {
  it('creates a signal with initial value', () => {
    const count = signal(0)
    expect(count.value).toBe(0)
  })

  it('updates value', () => {
    const count = signal(0)
    count.value = 10
    expect(count.value).toBe(10)
  })

  describe('nested describe for related tests', () => {
    // Related tests grouped
  })
})
```

**Patterns:**
- One `describe()` per primitive (signal, derived, effect, bind, etc.)
- Nested `describe()` for related functionality groups
- `it()` blocks test specific behaviors
- Setup is usually inline in each `it()` (no shared fixtures needed)
- `flushSync()` used to drain the effect queue when testing async behavior

**Rust Equivalent Structure:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_creates_with_initial_value() {
        let count = signal(0);
        assert_eq!(count.get(), 0);
    }

    #[test]
    fn test_signal_updates_value() {
        let count = signal(0);
        count.set(10);
        assert_eq!(count.get(), 10);
    }

    mod nested_behavior_group {
        use super::*;

        #[test]
        fn related_test() {
            // Test related behavior
        }
    }
}
```

## Test Patterns

**Dependency Tracking Test (TypeScript):**

```typescript
it('tracks dependencies in effects', () => {
  const count = signal(0)
  let observed = 0
  let runs = 0

  effect(() => {
    observed = count.value  // Read creates dependency
    runs++
  })

  flushSync()
  expect(runs).toBe(1)
  expect(observed).toBe(0)

  count.value = 5          // Triggers effect re-run
  flushSync()
  expect(runs).toBe(2)
  expect(observed).toBe(5)
})
```

**Key elements:**
- Create signal(s)
- Track how many times effect runs (counter variable)
- Track what value effect observed
- Use `flushSync()` to drain pending effects synchronously
- Verify effect ran and observed correct values
- Trigger change and verify effect re-ran

**Equality Function Test (TypeScript):**

```typescript
it('uses equality function', () => {
  const count = signal(0)
  let runs = 0

  effect(() => {
    count.value
    runs++
  })

  flushSync()
  expect(runs).toBe(1)

  // Same value - should NOT trigger
  count.value = 0
  flushSync()
  expect(runs).toBe(1)

  // Different value - should trigger
  count.value = 1
  flushSync()
  expect(runs).toBe(2)
})
```

**Key elements:**
- Test that setting to same value doesn't trigger effect
- Test that setting to different value does trigger
- Useful for testing equality implementation

**Lazy Evaluation Test (TypeScript):**

```typescript
it('is lazy - only computes when read', () => {
  const count = signal(1)
  let computations = 0

  const doubled = derived(() => {
    computations++
    return count.value * 2
  })

  expect(computations).toBe(0)

  doubled.value // First read
  expect(computations).toBe(1)

  doubled.value // Cached
  expect(computations).toBe(1)

  count.value = 5 // Marks dirty
  // Still haven't recomputed
  expect(computations).toBe(1)

  doubled.value // Now recomputes
  expect(computations).toBe(2)
})
```

**Key elements:**
- Verify derived doesn't compute until read
- Verify second read uses cache
- Verify write doesn't trigger computation (lazy)
- Verify next read after write recomputes

## Mocking

**Framework:** No external mocking library used in reference implementation

**Patterns:**
- Use closures to capture test variables
- Example: `const eff = effect(() => { runCount++; data = signal.value })`
- Verify side effects via captured variables

**What to Mock:**
- Nothing in the signals library (it's pure reactivity, no external deps)
- In applications using signals: DOM updates, API calls, I/O

**What NOT to Mock:**
- Signals themselves (test the real implementation)
- Effects (test the real execution)
- Derived values (test the real computation)

## Fixtures and Factories

**Test Data (TypeScript pattern):**

No separate fixture files in the reference implementation. Instead:
- Create test data inline in each test
- Use simple factories for repeated patterns

Example from bind tests:
```typescript
describe('bind', () => {
  it('creates a binding to a signal', () => {
    const source = signal(0)  // Inline fixture
    const binding = bind(source)
    expect(binding.value).toBe(0)
  })

  it('writes through to the source', () => {
    const source = signal(0)  // Repeated fixture
    const binding = bind(source)
    binding.value = 42
    expect(source.value).toBe(42)
  })
})
```

**No separate fixture files** - each test creates what it needs

**Location:**
- Inline in test files
- Shared utilities in a `test/utils.ts` if needed (not used in reference impl)

## Coverage

**Requirements (TypeScript reference):**
- No explicit coverage target documented
- Comprehensive test suite covers all primitives and patterns
- Test files include unit, integration, and performance tests

**View Coverage (Rust equivalent):**
```bash
cargo tarpaulin --out Html  # Generate coverage report
cargo tarpaulin              # Text output
```

Or with LLVM coverage:
```bash
RUSTFLAGS="-C instrument-coverage" cargo test
llvm-cov report
```

## Test Types

**Unit Tests:**
- **Scope:** Single primitive (signal, derived, effect, bind, etc.) in isolation
- **Approach:** Create primitive, verify basic operations
- **Location:** `test/unit/[primitive].test.ts` in TypeScript, inline in `src/` for Rust
- **Example:** Test signal creation, value get/set, equality checking

**Integration Tests:**
- **Scope:** Multiple primitives working together
- **Approach:** Create signal → derived → effect chain, verify propagation
- **Location:** `test/integration/` in TypeScript, `tests/` directory in Rust
- **Example:** Nested derived signals, effect chains, deep reactivity

**Performance Tests:**
- **Scope:** Benchmarking performance characteristics
- **Approach:** Use criterion.rs with `black_box()` to prevent optimization
- **Location:** `benches/` directory (already configured)
- **Example:** `bench_signal_create()`, `bench_derived_chain()`, `bench_many_effects()`

## Common Patterns

**Async Testing (TypeScript):**

```typescript
it('handles async operations', async () => {
  const count = signal(0)
  const doubled = derived(() => count.value * 2)

  effect(() => {
    // Effect body
  })

  // In TypeScript, effects are scheduled asynchronously
  // Use flushSync() to run them immediately in tests
  flushSync()

  // Or use await for real async (uncommon in signals tests)
  await Promise.resolve()
})
```

**Rust Equivalent:**
- Rust's test runner is synchronous by default
- For actual async effects, use `#[tokio::test]` if needed
- In signals, effects typically run synchronously in Rust port

**Cleanup Testing (TypeScript):**

```typescript
it('runs cleanup function on re-run', () => {
  const count = signal(0)
  let cleanups = 0

  effect(() => {
    count.value  // Create dependency
    return () => {
      cleanups++  // Cleanup function
    }
  })

  flushSync()
  expect(cleanups).toBe(0)  // First run, no cleanup yet

  count.value = 1
  flushSync()
  expect(cleanups).toBe(1)  // Previous effect's cleanup ran
})
```

**Rust pattern:** Similar - use closures to track cleanup execution

**Error Testing (TypeScript):**

```typescript
it('throws on invalid mutation in derived', () => {
  const sig = signal(0)
  const d = derived(() => {
    sig.value = 5  // ERROR: Cannot write in derived
    return sig.value
  })

  expect(() => d.value).toThrow(
    'Cannot write to signals inside a derived'
  )
})
```

**Key elements:**
- Verify specific error message
- Test error condition in action
- Use `expect().toThrow()` or `assert!()` depending on framework

## Benchmarking (Criterion.rs)

**Already configured in `benches/signals.rs`** - examples:

```rust
fn bench_signal_get(c: &mut Criterion) {
    let s = signal(42i32);
    c.bench_function("signal_get", |b| {
        b.iter(|| {
            black_box(s.get())
        })
    });
}

fn bench_derived_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("derived_chain");

    for depth in [1, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("depth", depth),
            &depth,
            |b, &depth| {
                // Build and benchmark chain
            },
        );
    }

    group.finish();
}

criterion_group!(
    signal_benches,
    bench_signal_create,
    bench_signal_get,
    bench_signal_set,
);

criterion_main!(signal_benches);
```

**Patterns:**
- Use `black_box()` to prevent compiler optimizations
- Group related benchmarks with `benchmark_group()`
- Use parameterized benchmarks with `BenchmarkId` for varying inputs
- Run with `cargo bench`

## Test Categories

**Existing in TypeScript reference:**

1. **unit/signal.test.ts** - Signal creation, get/set, equality
2. **unit/bind.test.ts** - Binding creation, read-through, write-through
3. **unit/linked.test.ts** - Linked signal behavior
4. **unit/scope.test.ts** - Effect scope lifecycle
5. **unit/proxy.test.ts** - Deep reactive proxy behavior
6. **unit/collections.test.ts** - ReactiveMap, ReactiveSet
7. **integration/\*.test.ts** - Cross-primitive scenarios
8. **performance/\*.test.ts** - Performance comparisons

**Port to Rust with similar structure:**

```
src/
├── lib.rs          # Inline unit tests for each module
├── signal.rs
│   └── #[cfg(test)] mod tests { #[test] ... }
├── derived.rs
│   └── #[cfg(test)] mod tests { #[test] ... }
└── ...

tests/               # Integration tests
├── signal_integration.rs
├── complex_chains.rs
└── ...

benches/             # Benchmarks (already have signals.rs)
├── signals.rs
└── ...
```

---

*Testing analysis: 2026-01-23*
