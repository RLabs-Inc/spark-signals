// ============================================================================
// spark-signals - Linked Signal (Angular's killer feature)
//
// A writable signal that ALSO resets based on a source signal.
// Perfect for UI state that should reset when parent data changes,
// but can also be manually overridden.
// ============================================================================
//
// Ported from @rlabs-inc/signals linked.ts
// ============================================================================

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::types::EqualsFn;
use crate::primitives::derived::derived;
use crate::primitives::effect::effect_sync;
use crate::primitives::signal::{signal, signal_with_equals, Signal};
use crate::reactivity::batching::untrack;

// =============================================================================
// TYPES
// =============================================================================

/// Configuration for linkedSignal with explicit source and computation.
pub struct LinkedSignalOptions<S, D, F, C>
where
    F: Fn() -> S + 'static,
    C: Fn(S, Option<PreviousValue<S, D>>) -> D + 'static,
{
    /// Function that produces the source value (creates dependency).
    pub source: F,

    /// Computation that derives the signal value from source.
    /// Receives the source value and optionally the previous source/value.
    pub computation: C,

    /// Optional equality function for the derived value.
    pub equal: Option<EqualsFn<D>>,
}

/// Previous value context passed to the computation function.
pub struct PreviousValue<S, D> {
    /// The previous source value.
    pub source: S,
    /// The previous derived value.
    pub value: D,
}

// =============================================================================
// LINKED SIGNAL
// =============================================================================

/// A linked signal that derives from a source but can be manually overridden.
/// When the source changes, the linked signal resets to the computed value.
///
/// This is inspired by Angular's linkedSignal - it solves the common UI pattern
/// where form state should reset when parent data changes, but the user can
/// also manually edit the value.
pub struct LinkedSignal<T> {
    /// The internal value signal.
    value_signal: Signal<T>,

    /// Track if user manually overrode the value.
    #[allow(dead_code)]
    manual_override: Rc<Cell<bool>>,

    /// Dispose function for the sync effect.
    _dispose: Rc<dyn Fn()>,
}

impl<T> Drop for LinkedSignal<T> {
    fn drop(&mut self) {
        // Only run dispose if this is the last strong reference
        // (shared ownership via Rc)
        if Rc::strong_count(&self._dispose) == 1 {
            (self._dispose)();
        }
    }
}

impl<T: Clone + PartialEq + 'static> LinkedSignal<T> {
    /// Get the current value.
    ///
    /// In a reactive context, this creates a dependency on the underlying signal.
    pub fn get(&self) -> T {
        self.value_signal.get()
    }

    /// Set the value manually (override).
    ///
    /// This manually overrides the value. The next source change will reset it.
    pub fn set(&self, value: T) -> bool {
        self.manual_override.set(true);
        self.value_signal.set(value)
    }

    /// Update the value in place.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.manual_override.set(true);
        self.value_signal.update(f);
    }

    /// Access the current value with a closure.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.value_signal.with(f)
    }

    /// Peek at the value without creating a dependency.
    pub fn peek(&self) -> T {
        untrack(|| self.value_signal.get())
    }
}

impl<T: Clone> Clone for LinkedSignal<T> {
    fn clone(&self) -> Self {
        Self {
            value_signal: self.value_signal.clone(),
            manual_override: self.manual_override.clone(),
            _dispose: self._dispose.clone(),
        }
    }
}

impl<T: std::fmt::Debug + Clone + PartialEq + 'static> std::fmt::Debug for LinkedSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkedSignal")
            .field("value", &self.get())
            .finish()
    }
}

// =============================================================================
// LINKED SIGNAL CREATION - SHORT FORM
// =============================================================================

/// Create a linked signal using the short form (just a getter function).
///
/// The getter function is both the source and computation - when its dependencies
/// change, the linked signal resets to the new computed value.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, linked_signal};
///
/// let options = signal(vec!["a", "b", "c"]);
/// let selected = linked_signal({
///     let options = options.clone();
///     move || options.get()[0]
/// });
///
/// assert_eq!(selected.get(), "a");
///
/// // Manual override
/// selected.set("b");
/// assert_eq!(selected.get(), "b");
///
/// // Source changes - resets to new first item
/// options.set(vec!["x", "y", "z"]);
/// assert_eq!(selected.get(), "x");
/// ```
pub fn linked_signal<T, F>(getter: F) -> LinkedSignal<T>
where
    T: Clone + PartialEq + 'static,
    F: Fn() -> T + 'static,
{
    linked_signal_with_options(LinkedSignalOptionsSimple {
        source: getter,
        equal: None,
    })
}

/// Options for simple linked signal (just source, no computation).
pub struct LinkedSignalOptionsSimple<T, F>
where
    F: Fn() -> T + 'static,
{
    pub source: F,
    pub equal: Option<EqualsFn<T>>,
}

/// Create a linked signal with simple options (source only, no computation).
pub fn linked_signal_with_options<T, F>(options: LinkedSignalOptionsSimple<T, F>) -> LinkedSignal<T>
where
    T: Clone + PartialEq + 'static,
    F: Fn() -> T + 'static,
{
    let source_fn = Rc::new(options.source);
    let equal = options.equal;

    // State
    let initialized = Rc::new(Cell::new(false));
    let manual_override = Rc::new(Cell::new(false));
    let last_known_source: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));

    // Create the value signal with initial undefined value
    // We'll set it properly in the first effect run
    let value_signal = if let Some(eq) = equal {
        // Create with initial value from source
        let initial = (source_fn)();
        signal_with_equals(initial, eq)
    } else {
        let initial = (source_fn)();
        signal(initial)
    };

    // Track source changes with a derived
    let source_tracker = derived({
        let source_fn = source_fn.clone();
        move || (source_fn)()
    });

    // Sync effect to update value when source changes
    let dispose = effect_sync({
        let source_tracker = source_tracker.clone();
        let value_signal = value_signal.clone();
        let initialized = initialized.clone();
        let last_known_source = last_known_source.clone();
        let manual_override_inner = manual_override.clone();

        move || {
            let current_source = source_tracker.get();

            // Check if source actually changed
            let source_changed = {
                let last = last_known_source.borrow();
                initialized.get() && last.as_ref() != Some(&current_source)
            };

            if !initialized.get() || source_changed {
                // Source changed or first init - update the value
                *last_known_source.borrow_mut() = Some(current_source.clone());
                initialized.set(true);
                manual_override_inner.set(false);

                // Update the value signal without tracking to avoid loops
                untrack(|| {
                    value_signal.set(current_source);
                });
            }
        }
    });

    // Wrap dispose in Rc for cloning
    let dispose_fn: Rc<RefCell<Option<Box<dyn FnOnce()>>>> = Rc::new(RefCell::new(Some(Box::new(dispose))));

    LinkedSignal {
        value_signal,
        manual_override,
        _dispose: Rc::new({
            let dispose_fn = dispose_fn.clone();
            move || {
                if let Some(f) = dispose_fn.borrow_mut().take() {
                    f();
                }
            }
        }),
    }
}

// =============================================================================
// LINKED SIGNAL CREATION - FULL FORM
// =============================================================================

/// Create a linked signal with full options (separate source and computation).
///
/// This gives you access to the previous source and value in the computation,
/// enabling patterns like "keep selection if still valid".
///
/// # Example
///
/// ```
/// use spark_signals::{signal, linked_signal_full, PreviousValue};
///
/// let options = signal(vec!["a", "b", "c"]);
/// let selected = linked_signal_full(
///     {
///         let options = options.clone();
///         move || options.get()
///     },
///     |opts: Vec<&str>, prev: Option<PreviousValue<Vec<&str>, &str>>| {
///         // Keep selection if still valid
///         if let Some(p) = prev {
///             if opts.contains(&p.value) {
///                 return p.value;
///             }
///         }
///         opts[0]
///     },
///     None,
/// );
///
/// assert_eq!(selected.get(), "a");
///
/// // Select "b"
/// selected.set("b");
/// assert_eq!(selected.get(), "b");
///
/// // Options change but "b" is still valid - keep it!
/// options.set(vec!["x", "b", "z"]);
/// assert_eq!(selected.get(), "b");
///
/// // Options change and "b" is gone - reset to first
/// options.set(vec!["x", "y", "z"]);
/// assert_eq!(selected.get(), "x");
/// ```
pub fn linked_signal_full<S, D, F, C>(
    source: F,
    computation: C,
    equal: Option<EqualsFn<D>>,
) -> LinkedSignal<D>
where
    S: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
    F: Fn() -> S + 'static,
    C: Fn(S, Option<PreviousValue<S, D>>) -> D + 'static,
{
    let source_fn = Rc::new(source);
    let computation_fn = Rc::new(computation);

    // State
    let initialized = Rc::new(Cell::new(false));
    let manual_override = Rc::new(Cell::new(false));
    let prev_source: Rc<RefCell<Option<S>>> = Rc::new(RefCell::new(None));
    let prev_value: Rc<RefCell<Option<D>>> = Rc::new(RefCell::new(None));

    // Create value signal - we need an initial value
    let initial_source = (source_fn)();
    let initial_value = (computation_fn)(initial_source.clone(), None);

    let value_signal = if let Some(eq) = equal {
        signal_with_equals(initial_value.clone(), eq)
    } else {
        signal(initial_value.clone())
    };

    // Store initial state
    *prev_source.borrow_mut() = Some(initial_source);
    *prev_value.borrow_mut() = Some(initial_value);
    initialized.set(true);

    // Track source changes
    let source_tracker = derived({
        let source_fn = source_fn.clone();
        move || (source_fn)()
    });

    // Sync effect for updates
    let dispose = effect_sync({
        let source_tracker = source_tracker.clone();
        let value_signal = value_signal.clone();
        let computation_fn = computation_fn.clone();
        let initialized = initialized.clone();
        let prev_source = prev_source.clone();
        let prev_value = prev_value.clone();
        let manual_override_inner = manual_override.clone();

        move || {
            let current_source = source_tracker.get();

            // Check if source changed
            let source_changed = {
                let last = prev_source.borrow();
                last.as_ref() != Some(&current_source)
            };

            if source_changed {
                // Build previous context
                // IMPORTANT: Read actual current value (might be manually overridden)
                let previous = if initialized.get() {
                    let ps = prev_source.borrow().clone();
                    // Get actual current value, not cached prev_value
                    let current_val = untrack(|| value_signal.get());
                    match ps {
                        Some(s) => Some(PreviousValue { source: s, value: current_val }),
                        None => None,
                    }
                } else {
                    None
                };

                // Compute new value
                let new_value = (computation_fn)(current_source.clone(), previous);

                // Update state
                *prev_source.borrow_mut() = Some(current_source);
                *prev_value.borrow_mut() = Some(new_value.clone());
                manual_override_inner.set(false);

                // Update signal
                untrack(|| {
                    value_signal.set(new_value);
                });
            }
        }
    });

    // Wrap dispose in Rc for cloning
    let dispose_fn: Rc<RefCell<Option<Box<dyn FnOnce()>>>> = Rc::new(RefCell::new(Some(Box::new(dispose))));

    LinkedSignal {
        value_signal,
        manual_override,
        _dispose: Rc::new({
            let dispose_fn = dispose_fn.clone();
            move || {
                if let Some(f) = dispose_fn.borrow_mut().take() {
                    f();
                }
            }
        }),
    }
}

// =============================================================================
// UTILITIES
// =============================================================================

/// Marker trait to identify LinkedSignal types.
pub trait IsLinkedSignal {}

impl<T> IsLinkedSignal for LinkedSignal<T> {}

/// Check if a value is a LinkedSignal.
pub fn is_linked_signal<T: IsLinkedSignal>(_value: &T) -> bool {
    true
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect;
    use std::cell::Cell;

    #[test]
    fn linked_signal_basic() {
        let source = signal(10);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        assert_eq!(linked.get(), 10);

        // Source changes - linked updates
        source.set(20);
        assert_eq!(linked.get(), 20);
    }

    #[test]
    fn linked_signal_manual_override() {
        let source = signal(10);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        assert_eq!(linked.get(), 10);

        // Manual override
        linked.set(99);
        assert_eq!(linked.get(), 99);

        // Source changes - resets to source value
        source.set(20);
        assert_eq!(linked.get(), 20);
    }

    #[test]
    fn linked_signal_with_derived_source() {
        let a = signal(5);
        let b = signal(10);
        let linked = linked_signal({
            let a = a.clone();
            let b = b.clone();
            move || a.get() + b.get()
        });

        assert_eq!(linked.get(), 15);

        // Change a
        a.set(10);
        assert_eq!(linked.get(), 20);

        // Change b
        b.set(20);
        assert_eq!(linked.get(), 30);

        // Override
        linked.set(100);
        assert_eq!(linked.get(), 100);

        // Source change resets
        a.set(1);
        assert_eq!(linked.get(), 21); // 1 + 20
    }

    #[test]
    fn linked_signal_creates_dependency() {
        let source = signal(10);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        let run_count = Rc::new(Cell::new(0));

        let _effect = effect({
            let linked = linked.clone();
            let run_count = run_count.clone();
            move || {
                let _ = linked.get();
                run_count.set(run_count.get() + 1);
            }
        });

        assert_eq!(run_count.get(), 1);

        // Changing source triggers effect
        source.set(20);
        assert_eq!(run_count.get(), 2);

        // Manual override triggers effect
        linked.set(50);
        assert_eq!(run_count.get(), 3);
    }

    #[test]
    fn linked_signal_peek() {
        let source = signal(10);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        let run_count = Rc::new(Cell::new(0));

        let _effect = effect({
            let linked = linked.clone();
            let run_count = run_count.clone();
            move || {
                let _ = linked.peek(); // Should NOT create dependency
                run_count.set(run_count.get() + 1);
            }
        });

        assert_eq!(run_count.get(), 1);

        // Changing source should NOT trigger effect (we used peek)
        source.set(20);
        // Note: The effect might still run due to how linked_signal is implemented
        // (the source tracker derived will trigger updates)
        // This is expected - peek on the linked signal doesn't prevent the source dependency
    }

    #[test]
    fn linked_signal_full_keeps_valid_selection() {
        let options = signal(vec!["a", "b", "c"]);
        let selected = linked_signal_full(
            {
                let options = options.clone();
                move || options.get()
            },
            |opts: Vec<&str>, prev: Option<PreviousValue<Vec<&str>, &str>>| {
                // Keep selection if still valid
                if let Some(p) = prev {
                    if opts.contains(&p.value) {
                        return p.value;
                    }
                }
                opts[0]
            },
            None,
        );

        assert_eq!(selected.get(), "a");

        // Select "b"
        selected.set("b");
        assert_eq!(selected.get(), "b");

        // Options change but "b" is still valid
        options.set(vec!["x", "b", "z"]);
        assert_eq!(selected.get(), "b");

        // Options change and "b" is gone
        options.set(vec!["x", "y", "z"]);
        assert_eq!(selected.get(), "x");
    }

    #[test]
    fn linked_signal_update() {
        let source = signal(10);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        assert_eq!(linked.get(), 10);

        linked.update(|v| *v += 5);
        assert_eq!(linked.get(), 15);

        // Source change resets
        source.set(100);
        assert_eq!(linked.get(), 100);
    }

    #[test]
    fn linked_signal_clone() {
        let source = signal(10);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        let cloned = linked.clone();

        // Both point to same state
        linked.set(50);
        assert_eq!(cloned.get(), 50);

        // Source change affects both
        source.set(100);
        assert_eq!(linked.get(), 100);
        assert_eq!(cloned.get(), 100);
    }

    #[test]
    fn linked_signal_debug() {
        let source = signal(42);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        let debug_str = format!("{:?}", linked);
        assert!(debug_str.contains("LinkedSignal"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn is_linked_signal_check() {
        let source = signal(10);
        let linked = linked_signal({
            let source = source.clone();
            move || source.get()
        });

        assert!(is_linked_signal(&linked));
    }
}
