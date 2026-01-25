// ============================================================================
// spark-signals - Batching
// Group multiple updates into a single reaction cycle
// ============================================================================
//
// Ported from @rlabs-inc/signals batching.ts
// ============================================================================

use crate::core::context::with_context;
use crate::reactivity::scheduling::flush_pending_reactions;

// =============================================================================
// BATCH
// =============================================================================

/// Batch multiple signal updates into a single reaction cycle.
///
/// Without batching, each signal update triggers effects immediately.
/// With batching, effects only run once after all updates complete.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, effect, batch};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// let a = signal(1);
/// let b = signal(2);
/// let run_count = Rc::new(Cell::new(0));
///
/// // Create effect that tracks both signals
/// let run_count_clone = run_count.clone();
/// let a_clone = a.clone();
/// let b_clone = b.clone();
/// let _dispose = effect(move || {
///     let _ = a_clone.get() + b_clone.get();
///     run_count_clone.set(run_count_clone.get() + 1);
/// });
///
/// // Effect ran once on creation
/// assert_eq!(run_count.get(), 1);
///
/// // Without batch: would run effect twice (once per update)
/// // With batch: runs effect once (after both updates)
/// batch(|| {
///     a.set(10);
///     b.set(20);
/// });
///
/// // Effect ran only once more (not twice)
/// assert_eq!(run_count.get(), 2);
/// ```
pub fn batch<T>(f: impl FnOnce() -> T) -> T {
    with_context(|ctx| ctx.enter_batch());

    // Use a guard pattern to ensure we exit the batch even on panic
    struct BatchGuard;

    impl Drop for BatchGuard {
        fn drop(&mut self) {
            let depth = with_context(|ctx| ctx.exit_batch());

            // When outermost batch completes, flush pending reactions
            if depth == 0 {
                flush_pending_reactions();
            }
        }
    }

    let _guard = BatchGuard;
    f()
}

/// Check if currently inside a batch.
///
/// # Example
///
/// ```
/// use spark_signals::{batch, is_batching};
///
/// assert!(!is_batching());
///
/// batch(|| {
///     assert!(is_batching());
/// });
///
/// assert!(!is_batching());
/// ```
pub fn is_batching() -> bool {
    with_context(|ctx| ctx.is_batching())
}

// =============================================================================
// UNTRACK
// =============================================================================

/// Read signals without creating dependencies.
///
/// Useful when you need to read a value but don't want the effect
/// to re-run when it changes.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, effect, untrack};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// let a = signal(1);
/// let b = signal(2);
/// let run_count = Rc::new(Cell::new(0));
///
/// let a_clone = a.clone();
/// let b_clone = b.clone();
/// let run_count_clone = run_count.clone();
/// let _dispose = effect(move || {
///     // This creates a dependency on 'a'
///     let a_val = a_clone.get();
///
///     // This does NOT create a dependency on 'b'
///     let b_val = untrack(|| b_clone.get());
///
///     run_count_clone.set(run_count_clone.get() + 1);
/// });
///
/// assert_eq!(run_count.get(), 1);
///
/// a.set(10); // Effect re-runs (dependency)
/// assert_eq!(run_count.get(), 2);
///
/// b.set(20); // Effect does NOT re-run (untracked)
/// assert_eq!(run_count.get(), 2);
/// ```
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    let prev = with_context(|ctx| ctx.set_untracking(true));

    // Use a guard pattern to ensure we restore even on panic
    struct UntrackGuard {
        prev: bool,
    }

    impl Drop for UntrackGuard {
        fn drop(&mut self) {
            with_context(|ctx| ctx.set_untracking(self.prev));
        }
    }

    let _guard = UntrackGuard { prev };
    f()
}

/// Alias for `untrack()`.
///
/// Some prefer this name as it's more explicit about "peeking" at a value
/// without creating a dependency.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, effect, peek};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// let count = signal(0);
/// let count_clone = count.clone();
///
/// let _dispose = effect(move || {
///     // Peek at value without tracking
///     let val = peek(|| count_clone.get());
///     // Effect won't re-run when count changes
/// });
/// ```
pub fn peek<T>(f: impl FnOnce() -> T) -> T {
    untrack(f)
}

/// Check if currently in untrack mode.
///
/// Returns true if inside an `untrack()` or `peek()` block.
pub fn is_untracking() -> bool {
    with_context(|ctx| ctx.is_untracking())
}

// =============================================================================
// TICK
// =============================================================================

/// Wait for the next update cycle.
///
/// In TypeScript with microtasks, this waits for a Promise tick then flushes.
/// In Rust without microtasks, this simply flushes all pending effects synchronously.
///
/// Use this when you need to ensure all pending effects have run before continuing.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, effect, batch, tick};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// let count = signal(0);
/// let seen = Rc::new(Cell::new(0));
///
/// let count_clone = count.clone();
/// let seen_clone = seen.clone();
/// let _dispose = effect(move || {
///     seen_clone.set(count_clone.get());
/// });
///
/// // Inside a batch, effects are deferred
/// batch(|| {
///     count.set(42);
///     // Effect hasn't run yet
/// });
///
/// // But tick() ensures effects have flushed
/// tick();
/// assert_eq!(seen.get(), 42);
/// ```
pub fn tick() {
    crate::reactivity::scheduling::flush_sync();
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{signal, effect, derived};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn batch_defers_effects() {
        let a = signal(1);
        let b = signal(2);
        let run_count = Rc::new(Cell::new(0));

        let run_count_clone = run_count.clone();
        let a_clone = a.clone();
        let b_clone = b.clone();
        let _dispose = effect(move || {
            let _ = a_clone.get() + b_clone.get();
            run_count_clone.set(run_count_clone.get() + 1);
        });

        // Effect runs once on creation
        assert_eq!(run_count.get(), 1);

        // Batch multiple updates
        batch(|| {
            a.set(10);
            // Effect should NOT have run yet
            assert_eq!(run_count.get(), 1);

            b.set(20);
            // Still should NOT have run
            assert_eq!(run_count.get(), 1);
        });

        // After batch exits, effect should have run exactly once
        assert_eq!(run_count.get(), 2);
    }

    #[test]
    fn batch_returns_value() {
        let result = batch(|| {
            42
        });
        assert_eq!(result, 42);

        let s = batch(|| String::from("hello"));
        assert_eq!(s, "hello");
    }

    #[test]
    fn nested_batches_work() {
        let a = signal(0);
        let run_count = Rc::new(Cell::new(0));

        let run_count_clone = run_count.clone();
        let a_clone = a.clone();
        let _dispose = effect(move || {
            let _ = a_clone.get();
            run_count_clone.set(run_count_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        batch(|| {
            a.set(1);

            batch(|| {
                a.set(2);
                a.set(3);
            });

            // Inner batch exited but outer batch still active
            // Effect should NOT have run yet
            assert_eq!(run_count.get(), 1);

            a.set(4);
        });

        // After outermost batch exits, effect runs once
        assert_eq!(run_count.get(), 2);
        assert_eq!(a.get(), 4);
    }

    #[test]
    fn is_batching_flag() {
        assert!(!is_batching());

        batch(|| {
            assert!(is_batching());

            batch(|| {
                assert!(is_batching());
            });

            assert!(is_batching());
        });

        assert!(!is_batching());
    }

    #[test]
    fn batch_with_derived() {
        let a = signal(1);
        let b = signal(2);

        let a_clone = a.clone();
        let b_clone = b.clone();
        let sum = derived(move || a_clone.get() + b_clone.get());

        let run_count = Rc::new(Cell::new(0));
        let run_count_clone = run_count.clone();
        let sum_clone = sum.clone();
        let _dispose = effect(move || {
            let _ = sum_clone.get();
            run_count_clone.set(run_count_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);
        assert_eq!(sum.get(), 3);

        batch(|| {
            a.set(10);
            b.set(20);
        });

        // Effect should have run once after batch
        assert_eq!(run_count.get(), 2);
        assert_eq!(sum.get(), 30);
    }

    #[test]
    fn batch_panic_safety() {
        let a = signal(0);
        let run_count = Rc::new(Cell::new(0));

        let run_count_clone = run_count.clone();
        let a_clone = a.clone();
        let _dispose = effect(move || {
            let _ = a_clone.get();
            run_count_clone.set(run_count_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        // Batch that panics
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            batch(|| {
                a.set(42);
                panic!("intentional panic");
            });
        }));

        assert!(result.is_err());

        // BatchGuard should have cleaned up - no longer in batch
        assert!(!is_batching());

        // The effect should have run because BatchGuard flushed on drop
        // (or at minimum, we're not stuck in a batch state)
    }

    #[test]
    fn multiple_signals_batch() {
        let signals: Vec<_> = (0..10).map(|i| signal(i)).collect();
        let total = Rc::new(Cell::new(0));
        let run_count = Rc::new(Cell::new(0));

        let signals_clone: Vec<_> = signals.iter().map(|s| s.clone()).collect();
        let total_clone = total.clone();
        let run_count_clone = run_count.clone();
        let _dispose = effect(move || {
            let sum: i32 = signals_clone.iter().map(|s| s.get()).sum();
            total_clone.set(sum);
            run_count_clone.set(run_count_clone.get() + 1);
        });

        // Initial: 0+1+2+...+9 = 45
        assert_eq!(total.get(), 45);
        assert_eq!(run_count.get(), 1);

        // Update all signals in a batch
        batch(|| {
            for (i, sig) in signals.iter().enumerate() {
                sig.set((i * 10) as i32);
            }
        });

        // Effect should run only once
        assert_eq!(run_count.get(), 2);
        // New total: 0+10+20+...+90 = 450
        assert_eq!(total.get(), 450);
    }

    // =========================================================================
    // UNTRACK TESTS
    // =========================================================================

    #[test]
    fn untrack_prevents_dependency() {
        let a = signal(1);
        let b = signal(2);
        let run_count = Rc::new(Cell::new(0));

        let a_clone = a.clone();
        let b_clone = b.clone();
        let run_count_clone = run_count.clone();
        let _dispose = effect(move || {
            // Track a
            let _a_val = a_clone.get();

            // Don't track b
            let _b_val = untrack(|| b_clone.get());

            run_count_clone.set(run_count_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        // a.set triggers effect (tracked)
        a.set(10);
        assert_eq!(run_count.get(), 2);

        // b.set does NOT trigger effect (untracked)
        b.set(20);
        assert_eq!(run_count.get(), 2);

        // a.set still triggers
        a.set(100);
        assert_eq!(run_count.get(), 3);
    }

    #[test]
    fn untrack_returns_value() {
        let count = signal(42);
        let count_clone = count.clone();

        let result = untrack(|| count_clone.get());
        assert_eq!(result, 42);

        let s = signal(String::from("hello"));
        let s_clone = s.clone();
        let result = untrack(|| s_clone.get());
        assert_eq!(result, "hello");
    }

    #[test]
    fn peek_is_alias_for_untrack() {
        let a = signal(1);
        let run_count = Rc::new(Cell::new(0));

        let a_clone = a.clone();
        let run_count_clone = run_count.clone();
        let _dispose = effect(move || {
            // Using peek instead of untrack
            let _val = peek(|| a_clone.get());
            run_count_clone.set(run_count_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        // Changing a should NOT trigger effect (peeked)
        a.set(10);
        assert_eq!(run_count.get(), 1);
    }

    #[test]
    fn is_untracking_flag() {
        assert!(!is_untracking());

        untrack(|| {
            assert!(is_untracking());
        });

        assert!(!is_untracking());
    }

    #[test]
    fn nested_untrack() {
        let a = signal(1);
        let run_count = Rc::new(Cell::new(0));

        let a_clone = a.clone();
        let run_count_clone = run_count.clone();
        let _dispose = effect(move || {
            untrack(|| {
                untrack(|| {
                    let _ = a_clone.get();
                });
            });
            run_count_clone.set(run_count_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        // Should not trigger (deeply nested untrack)
        a.set(10);
        assert_eq!(run_count.get(), 1);
    }

    #[test]
    fn untrack_in_derived() {
        let a = signal(1);
        let b = signal(2);

        let a_clone = a.clone();
        let b_clone = b.clone();
        let d = derived(move || {
            // Track a, untrack b
            a_clone.get() + untrack(|| b_clone.get())
        });

        assert_eq!(d.get(), 3);

        // a changes - derived should recompute
        a.set(10);
        assert_eq!(d.get(), 12); // 10 + 2

        // b changes - derived should NOT recompute (cached)
        b.set(20);
        assert_eq!(d.get(), 12); // Still cached: 10 + 2 (b was untracked)
    }

    #[test]
    fn untrack_panic_safety() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            untrack(|| {
                panic!("intentional panic");
            });
        }));

        assert!(result.is_err());

        // Should have restored untracking state
        assert!(!is_untracking());
    }

    // =========================================================================
    // TICK TESTS
    // =========================================================================

    #[test]
    fn tick_flushes_pending_effects() {
        let count = signal(0);
        let seen = Rc::new(Cell::new(0));

        let count_clone = count.clone();
        let seen_clone = seen.clone();
        let _dispose = effect(move || {
            seen_clone.set(count_clone.get());
        });

        assert_eq!(seen.get(), 0);

        count.set(42);

        // tick ensures effects have run
        tick();
        assert_eq!(seen.get(), 42);
    }

    #[test]
    fn tick_after_batch() {
        let count = signal(0);
        let seen = Rc::new(Cell::new(0));

        let count_clone = count.clone();
        let seen_clone = seen.clone();
        let _dispose = effect(move || {
            seen_clone.set(count_clone.get());
        });

        batch(|| {
            count.set(100);
        });

        // Batch already flushed, but tick is idempotent
        tick();
        assert_eq!(seen.get(), 100);
    }
}
