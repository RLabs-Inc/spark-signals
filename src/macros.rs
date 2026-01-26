// ============================================================================
// spark-signals - Ergonomic Macros
// ============================================================================

/// Helper macro to clone variables into a move closure.
///
/// This reduces the boilerplate of manually cloning `Rc` or `Signal` types
/// before moving them into a closure.
///
/// # Usage
///
/// ```rust
/// use spark_signals::{cloned, signal, derived};
///
/// let a = signal(1);
/// let b = signal(2);
///
/// // Use:
/// let sum = derived(cloned!(a, b => move || a.get() + b.get()));
/// ```
#[macro_export]
macro_rules! cloned {
    ($($n:ident),+ => $e:expr) => {
        {
            $( let $n = $n.clone(); )+
            $e
        }
    };
}

/// Create a derived signal with automatic variable capturing.
///
/// Wraps `derived(cloned!(... => move || ...))`.
///
/// # Usage
///
/// ```rust
/// use spark_signals::{derived, signal};
/// let a = signal(1);
/// let b = signal(2);
///
/// // Clean syntax: list deps => expression
/// let sum = derived!(a, b => a.get() + b.get());
/// ```
#[macro_export]
macro_rules! derived {
    // Case 1: With dependencies
    ($($deps:ident),+ => $body:expr) => {
        $crate::derived($crate::cloned!($($deps),+ => move || $body))
    };
    // Case 2: No dependencies (just expression)
    ($body:expr) => {
        $crate::derived(move || $body)
    };
}

/// Create an effect with automatic variable capturing.
///
/// Wraps `effect(cloned!(... => move || ...))`.
///
/// # Usage
///
/// ```rust
/// use spark_signals::{effect, signal};
/// let log = signal(vec![]);
///
/// effect!(log => {
///     println!("Log changed: {:?}", log.get());
/// });
/// ```
#[macro_export]
macro_rules! effect {
    // Case 1: With dependencies
    ($($deps:ident),+ => $body:expr) => {
        $crate::effect($crate::cloned!($($deps),+ => move || $body))
    };
    // Case 2: No dependencies
    ($body:expr) => {
        $crate::effect(move || $body)
    };
}