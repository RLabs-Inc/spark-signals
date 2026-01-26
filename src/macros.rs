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
/// // Instead of:
/// // let a_clone = a.clone();
/// // let b_clone = b.clone();
/// // derived(move || a_clone.get() + b_clone.get());
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

// Note: We don't define derived! or effect! macros yet as they would likely
// conflict with the function names or require distinct naming (e.g., derived_fn!).
// The cloned! macro provides 90% of the ergonomic benefit with 0% of the confusion.
