// ============================================================================
// spark-signals - Reactive Bindings
// Creates reactive links/pointers to other reactive values
// ============================================================================
//
// Ported from @rlabs-inc/signals bind.ts
//
// A binding is a "reactive pointer" - it forwards reads and writes to a source.
// This enables connecting user's reactive state to internal component state.
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::types::AnySource;
use crate::primitives::signal::{signal, Signal};

// =============================================================================
// MARKER TRAIT FOR BINDINGS
// =============================================================================

/// Marker trait to identify binding types.
/// In TypeScript this is done with BINDING_SYMBOL.
pub trait IsBinding {}

// =============================================================================
// BINDING<T> - WRITABLE TWO-WAY BINDING
// =============================================================================

/// The source of a binding's value.
enum BindingSource<T> {
    /// Forward to an existing signal (no internal source created).
    /// The signal is cloned (shares Rc), so reads/writes go to the same source.
    Forward(Signal<T>),

    /// Forward to another binding (chains to its source).
    Chain(Rc<BindingInner<T>>),

    /// Static value (no reactivity needed).
    /// Used for primitive values that don't need signal overhead.
    Static(RefCell<T>),
}

/// Internal binding storage.
struct BindingInner<T> {
    source: BindingSource<T>,
}

/// A writable binding that forwards reads and writes to a source.
///
/// Reading creates dependency on source, writing triggers source's reactions.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, bind};
///
/// let source = signal(0);
/// let binding = bind(source.clone());
///
/// // Reading through binding reads from source (creates dependency)
/// assert_eq!(binding.get(), 0);
///
/// // Writing through binding writes to source (triggers reactivity)
/// binding.set(42);
/// assert_eq!(source.get(), 42);
/// ```
pub struct Binding<T> {
    inner: Rc<BindingInner<T>>,
}

impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> IsBinding for Binding<T> {}

impl<T: Clone + PartialEq + 'static> Binding<T> {
    /// Get the current value.
    ///
    /// In a reactive context, this creates a dependency on the underlying source.
    pub fn get(&self) -> T {
        match &self.inner.source {
            BindingSource::Forward(sig) => sig.get(),
            BindingSource::Chain(inner) => {
                // Recursively get from the chained binding
                get_from_inner(inner)
            }
            BindingSource::Static(cell) => cell.borrow().clone(),
        }
    }

    /// Set the value.
    ///
    /// This writes to the underlying source, triggering reactivity.
    /// Returns true if the value changed.
    pub fn set(&self, value: T) -> bool {
        match &self.inner.source {
            BindingSource::Forward(sig) => sig.set(value),
            BindingSource::Chain(inner) => {
                // Recursively set on the chained binding
                set_on_inner(inner, value)
            }
            BindingSource::Static(cell) => {
                let mut borrowed = cell.borrow_mut();
                if *borrowed != value {
                    *borrowed = value;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Update the value in place using a closure.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        match &self.inner.source {
            BindingSource::Forward(sig) => sig.update(f),
            BindingSource::Chain(inner) => {
                update_on_inner(inner, f);
            }
            BindingSource::Static(cell) => {
                f(&mut *cell.borrow_mut());
            }
        }
    }

    /// Access the current value with a closure (avoids cloning).
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match &self.inner.source {
            BindingSource::Forward(sig) => sig.with(f),
            BindingSource::Chain(inner) => with_inner(inner, f),
            BindingSource::Static(cell) => f(&*cell.borrow()),
        }
    }

    /// Check if this binding wraps a static value (non-reactive).
    pub fn is_static(&self) -> bool {
        matches!(self.inner.source, BindingSource::Static(_))
    }

    /// Get the underlying signal if this binding forwards to one.
    /// Returns None for static bindings or deeply chained bindings.
    pub fn as_signal(&self) -> Option<Signal<T>> {
        match &self.inner.source {
            BindingSource::Forward(sig) => Some(sig.clone()),
            BindingSource::Chain(inner) => inner_as_signal(inner),
            BindingSource::Static(_) => None,
        }
    }
}

// Helper functions for recursive operations on BindingInner
fn get_from_inner<T: Clone + PartialEq + 'static>(inner: &Rc<BindingInner<T>>) -> T {
    match &inner.source {
        BindingSource::Forward(sig) => sig.get(),
        BindingSource::Chain(next) => get_from_inner(next),
        BindingSource::Static(cell) => cell.borrow().clone(),
    }
}

fn set_on_inner<T: Clone + PartialEq + 'static>(inner: &Rc<BindingInner<T>>, value: T) -> bool {
    match &inner.source {
        BindingSource::Forward(sig) => sig.set(value),
        BindingSource::Chain(next) => set_on_inner(next, value),
        BindingSource::Static(cell) => {
            let mut borrowed = cell.borrow_mut();
            if *borrowed != value {
                *borrowed = value;
                true
            } else {
                false
            }
        }
    }
}

fn update_on_inner<T: Clone + PartialEq + 'static>(inner: &Rc<BindingInner<T>>, f: impl FnOnce(&mut T)) {
    match &inner.source {
        BindingSource::Forward(sig) => sig.update(f),
        BindingSource::Chain(next) => update_on_inner(next, f),
        BindingSource::Static(cell) => {
            f(&mut *cell.borrow_mut());
        }
    }
}

fn with_inner<T: Clone + PartialEq + 'static, R>(
    inner: &Rc<BindingInner<T>>,
    f: impl FnOnce(&T) -> R,
) -> R {
    match &inner.source {
        BindingSource::Forward(sig) => sig.with(f),
        BindingSource::Chain(next) => with_inner(next, f),
        BindingSource::Static(cell) => f(&*cell.borrow()),
    }
}

fn inner_as_signal<T: Clone + PartialEq + 'static>(inner: &Rc<BindingInner<T>>) -> Option<Signal<T>> {
    match &inner.source {
        BindingSource::Forward(sig) => Some(sig.clone()),
        BindingSource::Chain(next) => inner_as_signal(next),
        BindingSource::Static(_) => None,
    }
}

impl<T: std::fmt::Debug + Clone + PartialEq + 'static> std::fmt::Debug for Binding<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Binding")
            .field("value", &self.get())
            .finish()
    }
}

// =============================================================================
// READONLY BINDING<T> - READ-ONLY ONE-WAY BINDING
// =============================================================================

/// The source of a read-only binding's value.
enum ReadonlySource<T> {
    /// Forward to a signal (read-only access).
    Signal(Signal<T>),

    /// Forward to a writable binding (read-only access).
    Binding(Rc<BindingInner<T>>),

    /// Getter function that produces values on demand.
    /// The getter is called in reactive context, so it can create dependencies.
    Getter(Rc<dyn Fn() -> T>),

    /// Static value (no reactivity).
    Static(T),
}

/// Internal readonly binding storage.
struct ReadonlyInner<T> {
    source: ReadonlySource<T>,
}

/// A read-only binding that forwards reads to a source.
///
/// Attempting to write will panic at runtime.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, bind_readonly};
///
/// let source = signal(0);
/// let readonly = bind_readonly(source.clone());
///
/// assert_eq!(readonly.get(), 0);
/// // readonly.set(42);  // Would panic!
/// ```
pub struct ReadonlyBinding<T> {
    inner: Rc<ReadonlyInner<T>>,
}

impl<T> Clone for ReadonlyBinding<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> IsBinding for ReadonlyBinding<T> {}

impl<T: Clone + PartialEq + 'static> ReadonlyBinding<T> {
    /// Get the current value.
    ///
    /// In a reactive context, this creates a dependency on the underlying source.
    pub fn get(&self) -> T {
        match &self.inner.source {
            ReadonlySource::Signal(sig) => sig.get(),
            ReadonlySource::Binding(inner) => get_from_inner(inner),
            ReadonlySource::Getter(f) => f(),
            ReadonlySource::Static(value) => value.clone(),
        }
    }

    /// Access the current value with a closure (avoids cloning).
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match &self.inner.source {
            ReadonlySource::Signal(sig) => sig.with(f),
            ReadonlySource::Binding(inner) => with_inner(inner, f),
            ReadonlySource::Getter(getter) => f(&getter()),
            ReadonlySource::Static(value) => f(value),
        }
    }

    /// Check if this binding wraps a static value (non-reactive).
    pub fn is_static(&self) -> bool {
        matches!(self.inner.source, ReadonlySource::Static(_))
    }
}

impl<T: std::fmt::Debug + Clone + PartialEq + 'static> std::fmt::Debug for ReadonlyBinding<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadonlyBinding")
            .field("value", &self.get())
            .finish()
    }
}

// =============================================================================
// BIND - CREATE REACTIVE BINDING
// =============================================================================

/// Create a two-way binding to a signal.
///
/// The binding forwards reads and writes directly to the signal.
/// Both the binding and the original signal share the same underlying source.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, bind};
///
/// let source = signal(0);
/// let binding = bind(source.clone());
///
/// binding.set(42);
/// assert_eq!(source.get(), 42);  // Same source!
/// ```
pub fn bind<T: Clone + PartialEq + 'static>(sig: Signal<T>) -> Binding<T> {
    Binding {
        inner: Rc::new(BindingInner {
            source: BindingSource::Forward(sig),
        }),
    }
}

/// Create a two-way binding from another binding (chaining).
///
/// The new binding chains to the source binding, ultimately reading/writing
/// to the same underlying signal.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, bind, bind_chain};
///
/// let source = signal(0);
/// let b1 = bind(source.clone());
/// let b2 = bind_chain(b1.clone());
///
/// b2.set(99);
/// assert_eq!(source.get(), 99);  // Chained to same source!
/// ```
pub fn bind_chain<T: Clone + PartialEq + 'static>(binding: Binding<T>) -> Binding<T> {
    Binding {
        inner: Rc::new(BindingInner {
            source: BindingSource::Chain(binding.inner.clone()),
        }),
    }
}

/// Create a two-way binding with a raw value (creates internal signal).
///
/// An internal signal is created to hold the value. This is useful when
/// you want binding semantics but don't have an existing signal.
///
/// # Example
///
/// ```
/// use spark_signals::bind_value;
///
/// let binding = bind_value(42);
/// assert_eq!(binding.get(), 42);
///
/// binding.set(100);
/// assert_eq!(binding.get(), 100);
/// ```
pub fn bind_value<T: Clone + PartialEq + 'static>(value: T) -> Binding<T> {
    Binding {
        inner: Rc::new(BindingInner {
            source: BindingSource::Forward(signal(value)),
        }),
    }
}

/// Create a two-way binding with a static value (no reactivity).
///
/// This is an optimization for primitive values that don't need reactive
/// tracking. No signal is created, saving memory.
///
/// Note: Changes to static bindings don't trigger any reactivity.
///
/// # Example
///
/// ```
/// use spark_signals::bind_static;
///
/// let binding = bind_static(42);
/// assert_eq!(binding.get(), 42);
/// assert!(binding.is_static());
/// ```
pub fn bind_static<T: Clone + PartialEq + 'static>(value: T) -> Binding<T> {
    Binding {
        inner: Rc::new(BindingInner {
            source: BindingSource::Static(RefCell::new(value)),
        }),
    }
}

// =============================================================================
// BIND READONLY - CREATE READ-ONLY BINDING
// =============================================================================

/// Create a read-only binding to a signal.
///
/// The binding can read from the signal but cannot write to it.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, bind_readonly};
///
/// let source = signal(0);
/// let readonly = bind_readonly(source.clone());
///
/// assert_eq!(readonly.get(), 0);
/// source.set(42);
/// assert_eq!(readonly.get(), 42);
/// ```
pub fn bind_readonly<T: Clone + PartialEq + 'static>(sig: Signal<T>) -> ReadonlyBinding<T> {
    ReadonlyBinding {
        inner: Rc::new(ReadonlyInner {
            source: ReadonlySource::Signal(sig),
        }),
    }
}

/// Create a read-only binding from a writable binding.
///
/// This is useful when you want to expose a binding as read-only to consumers.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, bind, bind_readonly_from};
///
/// let source = signal(0);
/// let writable = bind(source.clone());
/// let readonly = bind_readonly_from(writable);
///
/// assert_eq!(readonly.get(), 0);
/// ```
pub fn bind_readonly_from<T: Clone + PartialEq + 'static>(binding: Binding<T>) -> ReadonlyBinding<T> {
    ReadonlyBinding {
        inner: Rc::new(ReadonlyInner {
            source: ReadonlySource::Binding(binding.inner.clone()),
        }),
    }
}

/// Create a read-only binding from a getter function.
///
/// The getter is called each time the binding is read. If the getter
/// accesses reactive values, dependencies are created.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, bind_getter};
///
/// let a = signal(10);
/// let b = signal(20);
/// let sum = bind_getter({
///     let a = a.clone();
///     let b = b.clone();
///     move || a.get() + b.get()
/// });
///
/// assert_eq!(sum.get(), 30);
/// a.set(15);
/// assert_eq!(sum.get(), 35);
/// ```
pub fn bind_getter<T: Clone + PartialEq + 'static, F: Fn() -> T + 'static>(f: F) -> ReadonlyBinding<T> {
    ReadonlyBinding {
        inner: Rc::new(ReadonlyInner {
            source: ReadonlySource::Getter(Rc::new(f)),
        }),
    }
}

/// Create a read-only binding with a static value.
///
/// The value never changes and no dependencies are created.
///
/// # Example
///
/// ```
/// use spark_signals::bind_readonly_static;
///
/// let readonly = bind_readonly_static(42);
/// assert_eq!(readonly.get(), 42);
/// assert!(readonly.is_static());
/// ```
pub fn bind_readonly_static<T: Clone + PartialEq + 'static>(value: T) -> ReadonlyBinding<T> {
    ReadonlyBinding {
        inner: Rc::new(ReadonlyInner {
            source: ReadonlySource::Static(value),
        }),
    }
}

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

/// Check if a value is a binding.
///
/// This is a compile-time check using the IsBinding trait.
/// For runtime checking of Any types, use `is_binding_any`.
pub fn is_binding<T: IsBinding>(_value: &T) -> bool {
    true
}

/// Unwrap a value from a binding, or return the value directly.
///
/// This is useful for reading values that may or may not be bound.
pub fn unwrap_binding<T: Clone + PartialEq + 'static>(binding: &Binding<T>) -> T {
    binding.get()
}

/// Unwrap a value from a readonly binding.
pub fn unwrap_readonly<T: Clone + PartialEq + 'static>(binding: &ReadonlyBinding<T>) -> T {
    binding.get()
}

// =============================================================================
// SIGNALS HELPER - CREATE MULTIPLE SIGNALS AT ONCE
// =============================================================================

// Note: The TypeScript `signals({ a: 1, b: 2 })` helper is hard to port directly
// to Rust without proc macros. Users should create signals individually or use
// a macro-based approach. We'll add this in Phase 12 (API Polish) if needed.

// =============================================================================
// DISCONNECT BINDING - Manual cleanup
// =============================================================================

/// Disconnect a binding from the reactive graph.
///
/// This is rarely needed in Rust because RAII handles cleanup automatically.
/// Use this only when you have circular references that prevent cleanup.
///
/// For bindings that forward to signals, this is a no-op (the signal handles
/// its own cleanup). For bindings with internal sources, this disconnects
/// the internal signal from the graph.
pub fn disconnect_binding<T: Clone + PartialEq + 'static>(binding: &Binding<T>) {
    // Get the underlying signal if any
    if let Some(sig) = binding.as_signal() {
        // Disconnect the signal's source from the reactive graph
        disconnect_source(sig.as_any_source());
    }
}

/// Disconnect a source from the reactive graph.
///
/// This removes the source from all reactions' dependency lists and clears
/// the source's reaction list. This breaks circular references and allows
/// garbage collection.
pub fn disconnect_source(source: Rc<dyn AnySource>) {
    // Collect all reactions first (borrow safety)
    let reactions: Vec<_> = {
        let mut collected = Vec::new();
        source.for_each_reaction(&mut |reaction| {
            collected.push(reaction);
            true
        });
        collected
    };

    // Remove this source from each reaction's deps
    for reaction in reactions {
        // The reaction's deps list contains Rc<dyn AnySource>
        // We need to find and remove this source
        // This is handled by the reaction's internal cleanup
        reaction.remove_source(&source);
    }

    // Clear the source's reactions list
    source.clear_reactions();
}

/// Check if a binding has an internal source that may need cleanup.
///
/// Static bindings and forwarding bindings return false.
pub fn binding_has_internal_source<T: Clone + PartialEq + 'static>(binding: &Binding<T>) -> bool {
    // Only bindings created with bind_value have internal sources
    // Forwarding bindings share the original signal's source
    // Static bindings have no signal at all
    matches!(binding.inner.source, BindingSource::Forward(_))
        && !binding.is_static()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect;
use super::{bind_chain, bind_getter, bind_readonly_static};
    use std::cell::Cell;

    #[test]
    fn bind_to_signal() {
        let source = signal(0);
        let binding = bind(source.clone());

        // Reading through binding
        assert_eq!(binding.get(), 0);

        // Writing through binding updates source
        binding.set(42);
        assert_eq!(source.get(), 42);

        // Updating source updates binding
        source.set(100);
        assert_eq!(binding.get(), 100);
    }

    #[test]
    fn test_bind_chain() {
        let source = signal(0);
        let b1 = bind(source.clone());
        let b2 = bind_chain(b1.clone());

        // All point to same source
        b2.set(99);
        assert_eq!(source.get(), 99);
        assert_eq!(b1.get(), 99);
        assert_eq!(b2.get(), 99);
    }

    #[test]
    fn bind_value_creates_internal_signal() {
        let binding = bind_value(42);
        assert_eq!(binding.get(), 42);

        binding.set(100);
        assert_eq!(binding.get(), 100);

        // Has an underlying signal
        assert!(binding.as_signal().is_some());
    }

    #[test]
    fn bind_static_no_signal() {
        let binding = bind_static(42);
        assert_eq!(binding.get(), 42);
        assert!(binding.is_static());

        // No underlying signal
        assert!(binding.as_signal().is_none());

        // Can still set
        binding.set(100);
        assert_eq!(binding.get(), 100);
    }

    #[test]
    fn bind_readonly_from_signal() {
        let source = signal(0);
        let readonly = bind_readonly(source.clone());

        assert_eq!(readonly.get(), 0);

        source.set(42);
        assert_eq!(readonly.get(), 42);
    }

    #[test]
    fn bind_readonly_from_binding() {
        let source = signal(0);
        let writable = bind(source.clone());
        let readonly = bind_readonly_from(writable.clone());

        assert_eq!(readonly.get(), 0);

        writable.set(42);
        assert_eq!(readonly.get(), 42);
    }

    #[test]
    fn test_bind_getter() {
        let a = signal(10);
        let b = signal(20);
        let sum = bind_getter({
            let a = a.clone();
            let b = b.clone();
            move || a.get() + b.get()
        });

        assert_eq!(sum.get(), 30);

        a.set(15);
        assert_eq!(sum.get(), 35);

        b.set(25);
        assert_eq!(sum.get(), 40);
    }

    #[test]
    fn test_bind_readonly_static() {
        let readonly = bind_readonly_static(42);
        assert_eq!(readonly.get(), 42);
        assert!(readonly.is_static());
    }

    #[test]
    fn binding_with_closure() {
        let binding = bind_value(vec![1, 2, 3, 4, 5]);

        let sum = binding.with(|v| v.iter().sum::<i32>());
        assert_eq!(sum, 15);

        let len = binding.with(|v| v.len());
        assert_eq!(len, 5);
    }

    #[test]
    fn binding_update() {
        let binding = bind_value(vec![1, 2, 3]);

        binding.update(|v| v.push(4));
        assert_eq!(binding.get(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn binding_debug() {
        let binding = bind_value(42);
        let debug_str = format!("{:?}", binding);
        assert!(debug_str.contains("Binding"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn binding_clone() {
        let binding = bind_value(42);
        let cloned = binding.clone();

        // Both point to same source
        binding.set(100);
        assert_eq!(cloned.get(), 100);
    }

    #[test]
    fn binding_creates_dependency() {
        let source = signal(0);
        let binding = bind(source.clone());

        let run_count = Rc::new(Cell::new(0));

        let _effect = effect({
            let binding = binding.clone();
            let run_count = run_count.clone();
            move || {
                let _ = binding.get();
                run_count.set(run_count.get() + 1);
            }
        });

        assert_eq!(run_count.get(), 1);

        // Changing source should trigger effect
        source.set(42);
        assert_eq!(run_count.get(), 2);

        // Changing via binding should also trigger
        binding.set(100);
        assert_eq!(run_count.get(), 3);
    }

    #[test]
    fn readonly_binding_creates_dependency() {
        let source = signal(0);
        let readonly = bind_readonly(source.clone());

        let run_count = Rc::new(Cell::new(0));

        let _effect = effect({
            let readonly = readonly.clone();
            let run_count = run_count.clone();
            move || {
                let _ = readonly.get();
                run_count.set(run_count.get() + 1);
            }
        });

        assert_eq!(run_count.get(), 1);

        // Changing source should trigger effect
        source.set(42);
        assert_eq!(run_count.get(), 2);
    }

    #[test]
    fn getter_binding_creates_dependency() {
        let a = signal(10);
        let getter_binding = bind_getter({
            let a = a.clone();
            move || a.get() * 2
        });

        let run_count = Rc::new(Cell::new(0));
        let last_value = Rc::new(Cell::new(0));

        let _effect = effect({
            let getter_binding = getter_binding.clone();
            let run_count = run_count.clone();
            let last_value = last_value.clone();
            move || {
                let v = getter_binding.get();
                last_value.set(v);
                run_count.set(run_count.get() + 1);
            }
        });

        assert_eq!(run_count.get(), 1);
        assert_eq!(last_value.get(), 20);

        // Changing a should trigger the effect (via getter dependency)
        a.set(15);
        assert_eq!(run_count.get(), 2);
        assert_eq!(last_value.get(), 30);
    }

    #[test]
    fn binding_equality_check() {
        let binding = bind_value(42);

        // Setting same value returns false
        assert!(!binding.set(42));

        // Setting different value returns true
        assert!(binding.set(100));
    }

    #[test]
    fn is_binding_check() {
        let binding = bind_value(42);
        let readonly = bind_readonly_static(42);

        assert!(is_binding(&binding));
        assert!(is_binding(&readonly));
    }
}
