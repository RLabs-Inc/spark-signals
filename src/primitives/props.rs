// ============================================================================
// spark-signals - Reactive Props
// Normalize component props to a consistent reactive interface
// Based on @rlabs-inc/signals primitives/props.ts
// ============================================================================


use crate::primitives::derived::{derived, Derived};
use crate::primitives::signal::Signal;

// =============================================================================
// PROP VALUE - A value that can be static, getter, or signal
// =============================================================================

/// A prop value that can be:
/// - A static value
/// - A getter function (computed on access)
/// - A signal (reactive)
///
/// This enables flexible prop APIs where callers can pass values in any form,
/// and the component normalizes them to consistent reactive access.
///
/// # Example
///
/// ```
/// use spark_signals::{PropValue, signal, reactive_prop};
///
/// // Static value
/// let name = PropValue::Static("hello".to_string());
///
/// // Getter function
/// let computed = PropValue::Getter(Box::new(|| "computed".to_string()));
///
/// // From a signal
/// let sig = signal("reactive".to_string());
/// let from_signal = PropValue::from_signal(&sig);
///
/// // All can be converted to Derived for uniform access
/// let derived_name = reactive_prop(name);
/// assert_eq!(derived_name.get(), "hello".to_string());
/// ```
pub enum PropValue<T: Clone + PartialEq + 'static> {
    /// A static (non-reactive) value
    Static(T),

    /// A getter function that computes the value
    Getter(Box<dyn Fn() -> T>),

    /// A signal reference
    Signal(Signal<T>),
}

impl<T: Clone + PartialEq + 'static> PropValue<T> {
    /// Create a PropValue from a signal reference.
    pub fn from_signal(signal: &Signal<T>) -> Self {
        PropValue::Signal(signal.clone())
    }

    /// Create a static PropValue.
    pub fn value(val: T) -> Self {
        PropValue::Static(val)
    }

    /// Create a getter PropValue from a closure.
    pub fn getter<F: Fn() -> T + 'static>(f: F) -> Self {
        PropValue::Getter(Box::new(f))
    }

    /// Unwrap the current value (without creating a reactive dependency).
    pub fn peek(&self) -> T {
        match self {
            PropValue::Static(v) => v.clone(),
            PropValue::Getter(f) => f(),
            PropValue::Signal(s) => s.inner().get(),
        }
    }
}

// =============================================================================
// REACTIVE PROP - Convert PropValue to Derived
// =============================================================================

/// Convert a PropValue to a Derived for uniform reactive access.
///
/// This normalizes any prop input type to a consistent `.get()` interface,
/// enabling components to treat all props uniformly regardless of how they
/// were passed.
///
/// # Example
///
/// ```
/// use spark_signals::{PropValue, signal, reactive_prop, effect_sync};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// // Create a signal-backed prop
/// let count = signal(42);
/// let prop = PropValue::from_signal(&count);
///
/// // Convert to derived
/// let derived_prop = reactive_prop(prop);
///
/// // Track how many times effect runs
/// let runs = Rc::new(Cell::new(0));
/// let runs_clone = runs.clone();
/// let derived_clone = derived_prop.clone();
///
/// let _dispose = effect_sync(move || {
///     let _ = derived_clone.get();
///     runs_clone.set(runs_clone.get() + 1);
/// });
///
/// assert_eq!(runs.get(), 1);
///
/// // Changing the signal triggers the effect through the derived
/// count.set(100);
/// assert_eq!(runs.get(), 2);
/// assert_eq!(derived_prop.get(), 100);
/// ```
pub fn reactive_prop<T: Clone + PartialEq + 'static>(prop: PropValue<T>) -> Derived<T> {
    match prop {
        PropValue::Static(v) => {
            // For static values, create a derived that returns the captured value
            derived(move || v.clone())
        }
        PropValue::Getter(f) => {
            // For getters, the derived calls the getter (tracking any signals inside)
            derived(move || f())
        }
        PropValue::Signal(s) => {
            // For signals, the derived reads from the signal (creating dependency)
            derived(move || s.get())
        }
    }
}

// =============================================================================
// REACTIVE PROPS MACRO HELPER
// =============================================================================

/// A trait for types that can be unwrapped to their inner value.
/// Implemented for PropValue, Signal, Derived, and raw values.
pub trait UnwrapProp<T> {
    /// Get the current value, potentially creating reactive dependencies.
    fn unwrap_value(&self) -> T;
}

impl<T: Clone + PartialEq + 'static> UnwrapProp<T> for PropValue<T> {
    fn unwrap_value(&self) -> T {
        match self {
            PropValue::Static(v) => v.clone(),
            PropValue::Getter(f) => f(),
            PropValue::Signal(s) => s.get(),
        }
    }
}

impl<T: Clone + PartialEq + 'static> UnwrapProp<T> for Signal<T> {
    fn unwrap_value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + PartialEq + 'static> UnwrapProp<T> for Derived<T> {
    fn unwrap_value(&self) -> T {
        self.get()
    }
}

// Note: We don't implement UnwrapProp<T> for T directly because it creates
// ambiguity with Signal<T> and Derived<T> which are also T. Instead, use
// PropValue::Static for static values.

/// Create a derived from any unwrappable prop.
///
/// This is a generic version of `reactive_prop` that works with any type
/// implementing `UnwrapProp`.
pub fn into_derived<T, P>(prop: P) -> Derived<T>
where
    T: Clone + PartialEq + 'static,
    P: UnwrapProp<T> + 'static,
{
    derived(move || prop.unwrap_value())
}

// =============================================================================
// PROPS BUILDER - For struct-based props
// =============================================================================

/// A builder pattern for creating reactive props structs.
///
/// # Example
///
/// ```
/// use spark_signals::{PropValue, PropsBuilder, signal, reactive_prop};
///
/// // Define a component's props
/// struct ButtonProps {
///     label: PropValue<String>,
///     disabled: PropValue<bool>,
///     count: PropValue<i32>,
/// }
///
/// impl ButtonProps {
///     fn new() -> Self {
///         Self {
///             label: PropValue::Static("Click me".to_string()),
///             disabled: PropValue::Static(false),
///             count: PropValue::Static(0),
///         }
///     }
///
///     fn with_label(mut self, label: PropValue<String>) -> Self {
///         self.label = label;
///         self
///     }
///
///     fn with_disabled(mut self, disabled: PropValue<bool>) -> Self {
///         self.disabled = disabled;
///         self
///     }
///
///     fn with_count(mut self, count: PropValue<i32>) -> Self {
///         self.count = count;
///         self
///     }
/// }
///
/// // In component implementation, convert to reactive:
/// fn use_button(props: ButtonProps) {
///     let label = reactive_prop(props.label);
///     let disabled = reactive_prop(props.disabled);
///     let count = reactive_prop(props.count);
///
///     // Now all props are Derived<T> with uniform .get() access
/// }
///
/// // Usage with different input types:
/// let label_signal = signal("Dynamic".to_string());
///
/// let props = ButtonProps::new()
///     .with_label(PropValue::from_signal(&label_signal))
///     .with_disabled(PropValue::Static(true));
///
/// use_button(props);
/// ```
pub struct PropsBuilder<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> PropsBuilder<T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Default for PropsBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::effect::effect_sync;
    use crate::primitives::signal::signal;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn prop_value_static() {
        let prop = PropValue::Static(42);
        assert_eq!(prop.peek(), 42);
    }

    #[test]
    fn prop_value_getter() {
        let counter = Rc::new(Cell::new(0));
        let counter_clone = counter.clone();

        let prop = PropValue::getter(move || {
            counter_clone.set(counter_clone.get() + 1);
            100
        });

        // Each peek calls the getter
        assert_eq!(prop.peek(), 100);
        assert_eq!(counter.get(), 1);

        assert_eq!(prop.peek(), 100);
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn prop_value_signal() {
        let sig = signal(42);
        let prop = PropValue::from_signal(&sig);

        assert_eq!(prop.peek(), 42);

        sig.set(100);
        assert_eq!(prop.peek(), 100);
    }

    #[test]
    fn reactive_prop_static() {
        let prop = PropValue::Static(42);
        let derived = reactive_prop(prop);

        assert_eq!(derived.get(), 42);
    }

    #[test]
    fn reactive_prop_getter_with_signal() {
        let source = signal(10);
        let source_clone = source.clone();

        let prop = PropValue::getter(move || source_clone.get() * 2);
        let derived = reactive_prop(prop);

        assert_eq!(derived.get(), 20);

        source.set(5);
        assert_eq!(derived.get(), 10);
    }

    #[test]
    fn reactive_prop_signal_creates_dependency() {
        let source = signal(42);
        let prop = PropValue::from_signal(&source);
        let derived = reactive_prop(prop);

        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();
        let derived_clone = derived.clone();

        let _dispose = effect_sync(move || {
            let _ = derived_clone.get();
            runs_clone.set(runs_clone.get() + 1);
        });

        assert_eq!(runs.get(), 1);

        source.set(100);
        assert_eq!(runs.get(), 2);
        assert_eq!(derived.get(), 100);
    }

    #[test]
    fn unwrap_prop_trait() {
        let sig = signal(42);
        let derived = crate::primitives::derived::derived({
            let sig = sig.clone();
            move || sig.get() * 2
        });
        let prop = PropValue::Static(10);

        assert_eq!(UnwrapProp::<i32>::unwrap_value(&sig), 42);
        assert_eq!(UnwrapProp::<i32>::unwrap_value(&derived), 84);
        assert_eq!(UnwrapProp::<i32>::unwrap_value(&prop), 10);
    }

    #[test]
    fn into_derived_from_signal() {
        let sig = signal(42);
        let derived: Derived<i32> = into_derived(sig.clone());

        assert_eq!(derived.get(), 42);

        sig.set(100);
        assert_eq!(derived.get(), 100);
    }

    #[test]
    fn into_derived_from_prop_value() {
        let prop = PropValue::Static(42);
        let derived: Derived<i32> = into_derived(prop);

        assert_eq!(derived.get(), 42);
    }

    #[test]
    fn prop_value_convenience_constructors() {
        let static_prop = PropValue::value(42);
        assert_eq!(static_prop.peek(), 42);

        let getter_prop = PropValue::getter(|| 100);
        assert_eq!(getter_prop.peek(), 100);

        let sig = signal(200);
        let signal_prop = PropValue::from_signal(&sig);
        assert_eq!(signal_prop.peek(), 200);
    }

    #[test]
    fn component_props_pattern() {
        // Simulate a component's props struct
        struct MyProps {
            name: PropValue<String>,
            count: PropValue<i32>,
        }

        // Create props with mixed input types
        let count_signal = signal(5);

        let props = MyProps {
            name: PropValue::Static("hello".to_string()),
            count: PropValue::from_signal(&count_signal),
        };

        // In component, convert to reactive
        let name = reactive_prop(props.name);
        let count = reactive_prop(props.count);

        assert_eq!(name.get(), "hello");
        assert_eq!(count.get(), 5);

        // Signal changes propagate
        count_signal.set(10);
        assert_eq!(count.get(), 10);
    }
}
