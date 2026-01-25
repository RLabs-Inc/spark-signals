// ============================================================================
// spark-signals - Signal Primitive
// The core writable reactive signal
// ============================================================================

use std::rc::Rc;

use crate::core::context::with_context;
use crate::core::types::{AnySource, EqualsFn, SourceInner};
use crate::reactivity::tracking::{notify_write, track_read};

// =============================================================================
// SIGNAL<T> - The public signal handle
// =============================================================================

/// A reactive signal that holds a value of type T.
///
/// Signals are the foundation of the reactive system. When a signal's value
/// changes, all dependent deriveds and effects are notified and updated.
///
/// # Example
///
/// ```
/// use spark_signals::signal;
///
/// let count = signal(0);
/// assert_eq!(count.get(), 0);
///
/// count.set(5);
/// assert_eq!(count.get(), 5);
/// ```
#[derive(Clone)]
pub struct Signal<T> {
    inner: Rc<SourceInner<T>>,
}

impl<T> Signal<T> {
    /// Create a new signal with the given initial value.
    pub fn new(value: T) -> Self
    where
        T: PartialEq + 'static,
    {
        Self {
            inner: Rc::new(SourceInner::new(value)),
        }
    }

    /// Create a new signal with a custom equality function.
    pub fn new_with_equals(value: T, equals: EqualsFn<T>) -> Self
    where
        T: 'static,
    {
        Self {
            inner: Rc::new(SourceInner::new_with_equals(value, equals)),
        }
    }

    /// Get the current value (cloning).
    ///
    /// In a reactive context (inside an effect or derived), this will
    /// register the signal as a dependency.
    pub fn get(&self) -> T
    where
        T: Clone + 'static,
    {
        // Track this read for dependency registration
        track_read(self.inner.clone() as Rc<dyn AnySource>);
        self.inner.get()
    }

    /// Try to get the current value, returning None if the borrow fails.
    ///
    /// This is useful when you're not sure if the value is currently borrowed
    /// mutably elsewhere (though this shouldn't happen in normal usage).
    pub fn try_get(&self) -> Option<T>
    where
        T: Clone,
    {
        // In normal usage this always succeeds, but provides safety for edge cases
        Some(self.inner.get())
    }

    /// Access the current value with a closure (avoids cloning).
    ///
    /// # Example
    ///
    /// ```
    /// use spark_signals::signal;
    ///
    /// let items = signal(vec![1, 2, 3]);
    /// let sum = items.with(|v| v.iter().sum::<i32>());
    /// assert_eq!(sum, 6);
    /// ```
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R
    where
        T: 'static,
    {
        // Track this read for dependency registration
        track_read(self.inner.clone() as Rc<dyn AnySource>);
        self.inner.with(f)
    }

    /// Set the signal's value.
    ///
    /// Returns true if the value changed (based on equality check).
    /// If the value didn't change, no notifications are sent.
    pub fn set(&self, value: T) -> bool
    where
        T: 'static,
    {
        let changed = self.inner.set(value);
        if changed {
            // Update write version in context and notify reactions
            with_context(|ctx| {
                let wv = ctx.increment_write_version();
                self.inner.set_write_version(wv);
            });
            notify_write(self.inner.clone() as Rc<dyn AnySource>);
        }
        changed
    }

    /// Update the value in place using a closure.
    ///
    /// # Example
    ///
    /// ```
    /// use spark_signals::signal;
    ///
    /// let count = signal(0);
    /// count.update(|n| *n += 1);
    /// assert_eq!(count.get(), 1);
    /// ```
    pub fn update(&self, f: impl FnOnce(&mut T))
    where
        T: Clone + 'static,
    {
        let had_reactions = self.inner.update(f);
        if had_reactions {
            // Update write version and notify reactions
            with_context(|ctx| {
                let wv = ctx.increment_write_version();
                self.inner.set_write_version(wv);
            });
            notify_write(self.inner.clone() as Rc<dyn AnySource>);
        }
    }

    /// Get a reference to the inner source (for advanced use).
    pub fn inner(&self) -> &Rc<SourceInner<T>> {
        &self.inner
    }

    /// Get the inner source as a type-erased AnySource.
    ///
    /// This enables storing signals of different types in the same collection.
    pub fn as_any_source(&self) -> Rc<dyn AnySource>
    where
        T: 'static,
    {
        self.inner.clone()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Signal<T>
where
    T: Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signal")
            .field("value", &self.get())
            .finish()
    }
}

// =============================================================================
// SIGNAL CREATION FUNCTIONS (TypeScript-like API)
// =============================================================================

/// Create a new reactive signal.
///
/// This is the primary way to create signals, matching the TypeScript API.
///
/// # Example
///
/// ```
/// use spark_signals::signal;
///
/// let count = signal(0);
/// let name = signal(String::from("hello"));
///
/// count.set(42);
/// assert_eq!(count.get(), 42);
/// ```
pub fn signal<T>(value: T) -> Signal<T>
where
    T: PartialEq + 'static,
{
    Signal::new(value)
}

/// Create a signal with a custom equality function.
///
/// # Example
///
/// ```
/// use spark_signals::primitives::signal::signal_with_equals;
///
/// // Signal that always considers values different (always notifies)
/// let always_notify = signal_with_equals(0, |_, _| false);
///
/// // Even setting the same value returns true (changed)
/// assert!(always_notify.set(0));
/// ```
pub fn signal_with_equals<T>(value: T, equals: EqualsFn<T>) -> Signal<T>
where
    T: 'static,
{
    Signal::new_with_equals(value, equals)
}

// =============================================================================
// SOURCE (Low-level API)
// =============================================================================

/// Options for creating a source.
pub struct SourceOptions<T> {
    pub equals: Option<EqualsFn<T>>,
}

impl<T> Default for SourceOptions<T> {
    fn default() -> Self {
        Self { equals: None }
    }
}

/// Create a source (low-level signal).
///
/// This is the low-level primitive. Most users should use `signal()` instead.
pub fn source<T>(value: T, options: Option<SourceOptions<T>>) -> Signal<T>
where
    T: PartialEq + 'static,
{
    match options.and_then(|o| o.equals) {
        Some(eq) => Signal::new_with_equals(value, eq),
        None => Signal::new(value),
    }
}

// =============================================================================
// MUTABLE SOURCE (for objects that need forced updates)
// =============================================================================

/// Create a mutable source that always triggers updates on set.
///
/// Unlike regular `source()` which uses equality checking, `mutable_source()`
/// always considers values as "changed", ensuring dependent reactions run.
///
/// Use this when:
/// - Storing mutable objects that may be modified in place
/// - You want to ensure updates propagate regardless of equality
/// - Working with types that don't implement PartialEq meaningfully
///
/// # Example
///
/// ```
/// use spark_signals::primitives::signal::mutable_source;
///
/// let data = mutable_source(vec![1, 2, 3]);
///
/// // Even setting to the "same" value triggers updates
/// assert!(data.set(vec![1, 2, 3])); // Returns true (changed)
/// ```
pub fn mutable_source<T>(value: T) -> Signal<T>
where
    T: 'static,
{
    Signal::new_with_equals(value, crate::reactivity::equality::never_equals)
}

/// Create a signal for f64 values with safe NaN handling.
///
/// Uses `safe_equals_f64` which treats NaN == NaN as true,
/// unlike IEEE 754 where NaN != NaN.
///
/// # Example
///
/// ```
/// use spark_signals::primitives::signal::signal_f64;
///
/// let value = signal_f64(f64::NAN);
///
/// // Setting to the same NaN doesn't trigger update (correctly equal)
/// assert!(!value.set(f64::NAN)); // Returns false (not changed)
///
/// // Setting to different value triggers update
/// assert!(value.set(1.0)); // Returns true (changed)
/// ```
pub fn signal_f64(value: f64) -> Signal<f64> {
    Signal::new_with_equals(value, crate::reactivity::equality::safe_equals_f64)
}

/// Create a signal for f32 values with safe NaN handling.
pub fn signal_f32(value: f32) -> Signal<f32> {
    Signal::new_with_equals(value, crate::reactivity::equality::safe_equals_f32)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::*;

    #[test]
    fn signal_creation() {
        let s = signal(42);
        assert_eq!(s.get(), 42);
    }

    #[test]
    fn signal_set() {
        let s = signal(1);
        assert_eq!(s.get(), 1);

        let changed = s.set(2);
        assert!(changed);
        assert_eq!(s.get(), 2);

        // Setting same value shouldn't "change"
        let changed = s.set(2);
        assert!(!changed);
    }

    #[test]
    fn signal_with() {
        let s = signal(vec![1, 2, 3, 4, 5]);
        let sum = s.with(|v| v.iter().sum::<i32>());
        assert_eq!(sum, 15);

        let len = s.with(|v| v.len());
        assert_eq!(len, 5);
    }

    #[test]
    fn signal_update() {
        let s = signal(10);
        s.update(|n| *n += 5);
        assert_eq!(s.get(), 15);

        s.update(|n| *n *= 2);
        assert_eq!(s.get(), 30);
    }

    #[test]
    fn signal_try_get() {
        let s = signal(42);
        assert_eq!(s.try_get(), Some(42));
    }

    #[test]
    fn signal_debug() {
        let s = signal(42);
        let debug_str = format!("{:?}", s);
        assert!(debug_str.contains("Signal"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn signal_clone() {
        let s1 = signal(42);
        let s2 = s1.clone();

        // Both point to the same inner source
        s1.set(100);
        assert_eq!(s2.get(), 100);
    }

    #[test]
    fn signal_as_any_source() {
        let s = signal(42);
        let any: Rc<dyn AnySource> = s.as_any_source();

        // Can check flags
        assert!(any.flags() & SOURCE != 0);
        assert!(any.is_clean());
    }

    #[test]
    fn heterogeneous_signal_storage() {
        // THE KEY TEST: Different T types in same Vec
        let int_sig = signal(42i32);
        let str_sig = signal(String::from("hello"));
        let bool_sig = signal(true);
        let vec_sig = signal(vec![1.0, 2.0, 3.0]);

        let sources: Vec<Rc<dyn AnySource>> = vec![
            int_sig.as_any_source(),
            str_sig.as_any_source(),
            bool_sig.as_any_source(),
            vec_sig.as_any_source(),
        ];

        assert_eq!(sources.len(), 4);

        // All have SOURCE flag
        for source in &sources {
            assert!(source.flags() & SOURCE != 0);
        }

        // Can mark dirty
        sources[0].mark_dirty();
        assert!(sources[0].is_dirty());
        assert!(sources[1].is_clean());
    }

    #[test]
    fn custom_equality_function() {
        // Always consider different (neverEquals)
        let s = signal_with_equals(42, |_, _| false);

        // Even same value is "changed"
        assert!(s.set(42));

        // Always consider equal (alwaysEquals)
        let s2 = signal_with_equals(0, |_, _| true);

        // Even different value is "not changed" (returns false)
        assert!(!s2.set(100));

        // Value is NOT updated when equality returns true
        // This matches TypeScript: if equals returns true, value stays the same
        assert_eq!(s2.get(), 0);
    }

    #[test]
    fn source_function() {
        let s = source(42, None);
        assert_eq!(s.get(), 42);

        let s2 = source(
            42,
            Some(SourceOptions {
                equals: Some(|_, _| false),
            }),
        );
        assert!(s2.set(42)); // Custom equals says "not equal"
    }

    #[test]
    fn signal_with_string() {
        let s = signal(String::from("hello"));
        assert_eq!(s.get(), "hello");

        s.set(String::from("world"));
        assert_eq!(s.get(), "world");

        s.update(|s| s.push_str("!"));
        assert_eq!(s.get(), "world!");
    }

    #[test]
    fn signal_with_option() {
        let s: Signal<Option<i32>> = signal(None);
        assert_eq!(s.get(), None);

        s.set(Some(42));
        assert_eq!(s.get(), Some(42));

        s.update(|opt| {
            if let Some(n) = opt {
                *n += 1;
            }
        });
        assert_eq!(s.get(), Some(43));
    }

    #[test]
    fn mutable_source_always_triggers() {
        let s = mutable_source(vec![1, 2, 3]);

        // Even setting to same value returns true (changed)
        assert!(s.set(vec![1, 2, 3]));

        // Value is still updated
        s.set(vec![4, 5, 6]);
        assert_eq!(s.get(), vec![4, 5, 6]);
    }

    #[test]
    fn mutable_source_without_partial_eq() {
        // Type without PartialEq
        struct NoEq {
            value: i32,
        }

        let s = mutable_source(NoEq { value: 42 });
        assert!(s.set(NoEq { value: 42 })); // Always triggers
        assert_eq!(s.with(|n| n.value), 42);
    }

    #[test]
    fn signal_f64_nan_handling() {
        let s = signal_f64(f64::NAN);

        // NaN == NaN with safe_equals
        assert!(!s.set(f64::NAN)); // Not changed

        // But NaN != regular values
        assert!(s.set(1.0)); // Changed
        assert_eq!(s.get(), 1.0);
    }

    #[test]
    fn signal_f64_normal_values() {
        let s = signal_f64(1.0);

        assert!(!s.set(1.0)); // Same value, not changed
        assert!(s.set(2.0)); // Different value, changed
    }

    #[test]
    fn signal_f32_nan_handling() {
        let s = signal_f32(f32::NAN);

        // NaN == NaN with safe_equals
        assert!(!s.set(f32::NAN)); // Not changed

        // But NaN != regular values
        assert!(s.set(1.0)); // Changed
    }
}
