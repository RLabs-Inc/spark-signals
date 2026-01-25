// ============================================================================
// spark-signals - Equality Functions
// Based on Svelte 5's / @rlabs-inc/signals equality checking
// ============================================================================

use crate::core::types::EqualsFn;

// =============================================================================
// STRICT EQUALITY (Default)
// =============================================================================

/// Default strict equality using PartialEq.
/// This is the default for signal() and source().
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::equals;
///
/// assert!(equals(&42, &42));
/// assert!(!equals(&42, &43));
/// assert!(equals(&"hello", &"hello"));
/// ```
pub fn equals<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

// =============================================================================
// SAFE EQUALITY (Handles NaN and mutable objects)
// =============================================================================

/// Safe not-equal check for f64.
/// Handles NaN correctly: NaN == NaN returns true (unlike IEEE 754).
///
/// In TypeScript, this also considers objects/functions as always not-equal
/// to force updates. In Rust, we focus on the NaN case since PartialEq
/// handles struct comparison correctly.
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::safe_not_equal_f64;
///
/// // Normal values
/// assert!(safe_not_equal_f64(&1.0, &2.0));
/// assert!(!safe_not_equal_f64(&1.0, &1.0));
///
/// // NaN handling - NaN is considered equal to NaN
/// assert!(!safe_not_equal_f64(&f64::NAN, &f64::NAN));
///
/// // But NaN is not equal to regular values
/// assert!(safe_not_equal_f64(&f64::NAN, &1.0));
/// ```
pub fn safe_not_equal_f64(a: &f64, b: &f64) -> bool {
    // NaN check: if a is NaN
    if a.is_nan() {
        // If a is NaN, they're "equal" only if b is also NaN
        // So "not equal" means b is NOT NaN
        return !b.is_nan();
    }

    // Otherwise standard not-equal
    a != b
}

/// Safe equality for f64 values.
/// Handles NaN correctly: NaN == NaN returns true.
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::safe_equals_f64;
///
/// assert!(safe_equals_f64(&1.0, &1.0));
/// assert!(!safe_equals_f64(&1.0, &2.0));
/// assert!(safe_equals_f64(&f64::NAN, &f64::NAN));
/// ```
pub fn safe_equals_f64(a: &f64, b: &f64) -> bool {
    !safe_not_equal_f64(a, b)
}

/// Safe not-equal check for f32.
pub fn safe_not_equal_f32(a: &f32, b: &f32) -> bool {
    if a.is_nan() {
        return !b.is_nan();
    }
    a != b
}

/// Safe equality for f32 values.
pub fn safe_equals_f32(a: &f32, b: &f32) -> bool {
    !safe_not_equal_f32(a, b)
}

/// Generic safe equality that handles Option<f64> and similar patterns.
/// For types that might contain NaN, this provides correct comparison.
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::safe_equals_option_f64;
///
/// assert!(safe_equals_option_f64(&Some(1.0), &Some(1.0)));
/// assert!(safe_equals_option_f64(&None, &None));
/// assert!(safe_equals_option_f64(&Some(f64::NAN), &Some(f64::NAN)));
/// assert!(!safe_equals_option_f64(&Some(1.0), &Some(2.0)));
/// ```
pub fn safe_equals_option_f64(a: &Option<f64>, b: &Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => safe_equals_f64(a, b),
        _ => false,
    }
}

// =============================================================================
// SHALLOW EQUALITY
// =============================================================================

/// Shallow equality for Vec - compares elements one level deep.
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::shallow_equals_vec;
///
/// assert!(shallow_equals_vec(&vec![1, 2, 3], &vec![1, 2, 3]));
/// assert!(!shallow_equals_vec(&vec![1, 2, 3], &vec![1, 2, 4]));
/// assert!(!shallow_equals_vec(&vec![1, 2], &vec![1, 2, 3]));
/// ```
pub fn shallow_equals_vec<T: PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

/// Shallow equality for slices.
pub fn shallow_equals_slice<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

// =============================================================================
// DEEP EQUALITY
// =============================================================================

/// Deep equality - for Rust, this is the same as PartialEq since Rust's
/// derive(PartialEq) already does deep structural comparison.
///
/// In TypeScript, deepEquals uses Bun.deepEquals for recursive comparison.
/// In Rust, #[derive(PartialEq)] on structs already provides this behavior.
///
/// This function exists for API parity with TypeScript.
pub fn deep_equals<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

// =============================================================================
// FACTORY FUNCTIONS
// =============================================================================

/// Never equal - always returns false, forcing updates on every set.
/// Useful for values that should always trigger reactivity.
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::never_equals;
///
/// assert!(!never_equals(&42, &42));
/// assert!(!never_equals(&1, &2));
/// ```
pub fn never_equals<T>(_a: &T, _b: &T) -> bool {
    false
}

/// Always equal - always returns true, never triggering updates.
/// Useful for static values that should never cause reactivity.
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::always_equals;
///
/// assert!(always_equals(&42, &42));
/// assert!(always_equals(&1, &2));
/// ```
pub fn always_equals<T>(_a: &T, _b: &T) -> bool {
    true
}

/// Create a typed equality function from a comparison closure.
/// Converts a closure to a function pointer for use with signals.
///
/// Note: In Rust, we can't easily convert closures to fn pointers,
/// so this is mainly useful for documenting the pattern. For custom
/// equality, use signal_with_equals with a fn pointer directly.
///
/// # Example
/// ```
/// use spark_signals::reactivity::equality::by_field;
///
/// #[derive(Clone)]
/// struct User { id: u32, name: String }
///
/// // Compare users by ID only
/// fn user_equals_by_id(a: &User, b: &User) -> bool {
///     a.id == b.id
/// }
///
/// // Use with signal_with_equals(user, user_equals_by_id)
/// ```
pub fn by_field<T, F, R>(field_fn: F) -> impl Fn(&T, &T) -> bool
where
    F: Fn(&T) -> R,
    R: PartialEq,
{
    move |a, b| field_fn(a) == field_fn(b)
}

// =============================================================================
// EQUALITY FUNCTION CONSTRUCTORS (for EqualsFn<T>)
// =============================================================================

/// Get the default equality function for a type.
/// This is `equals` - uses PartialEq.
pub fn default_equals_fn<T: PartialEq + 'static>() -> EqualsFn<T> {
    equals
}

/// Get the never-equals function for a type.
pub fn never_equals_fn<T: 'static>() -> EqualsFn<T> {
    never_equals
}

/// Get the always-equals function for a type.
pub fn always_equals_fn<T: 'static>() -> EqualsFn<T> {
    always_equals
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equals() {
        assert!(equals(&42, &42));
        assert!(!equals(&42, &43));
        assert!(equals(&"hello", &"hello"));
        assert!(!equals(&"hello", &"world"));
    }

    #[test]
    fn test_safe_equals_f64_normal() {
        assert!(safe_equals_f64(&1.0, &1.0));
        assert!(!safe_equals_f64(&1.0, &2.0));
        assert!(safe_equals_f64(&0.0, &0.0));
        assert!(safe_equals_f64(&-0.0, &0.0)); // -0.0 == 0.0 in IEEE 754
    }

    #[test]
    fn test_safe_equals_f64_nan() {
        // The key test: NaN == NaN should be true with safe_equals
        assert!(safe_equals_f64(&f64::NAN, &f64::NAN));

        // But NaN != regular values
        assert!(!safe_equals_f64(&f64::NAN, &1.0));
        assert!(!safe_equals_f64(&1.0, &f64::NAN));
    }

    #[test]
    fn test_safe_equals_f64_infinity() {
        assert!(safe_equals_f64(&f64::INFINITY, &f64::INFINITY));
        assert!(safe_equals_f64(&f64::NEG_INFINITY, &f64::NEG_INFINITY));
        assert!(!safe_equals_f64(&f64::INFINITY, &f64::NEG_INFINITY));
    }

    #[test]
    fn test_safe_equals_f32() {
        assert!(safe_equals_f32(&1.0f32, &1.0f32));
        assert!(safe_equals_f32(&f32::NAN, &f32::NAN));
        assert!(!safe_equals_f32(&f32::NAN, &1.0f32));
    }

    #[test]
    fn test_safe_equals_option_f64() {
        assert!(safe_equals_option_f64(&Some(1.0), &Some(1.0)));
        assert!(safe_equals_option_f64(&None, &None));
        assert!(safe_equals_option_f64(&Some(f64::NAN), &Some(f64::NAN)));
        assert!(!safe_equals_option_f64(&Some(1.0), &None));
        assert!(!safe_equals_option_f64(&None, &Some(1.0)));
    }

    #[test]
    fn test_shallow_equals_vec() {
        assert!(shallow_equals_vec(&vec![1, 2, 3], &vec![1, 2, 3]));
        assert!(!shallow_equals_vec(&vec![1, 2, 3], &vec![1, 2, 4]));
        assert!(!shallow_equals_vec(&vec![1, 2], &vec![1, 2, 3]));
        assert!(shallow_equals_vec::<i32>(&vec![], &vec![]));
    }

    #[test]
    fn test_shallow_equals_slice() {
        let a = [1, 2, 3];
        let b = [1, 2, 3];
        let c = [1, 2, 4];
        assert!(shallow_equals_slice(&a, &b));
        assert!(!shallow_equals_slice(&a, &c));
    }

    #[test]
    fn test_deep_equals() {
        #[derive(PartialEq, Debug)]
        struct Nested {
            inner: Vec<i32>,
        }

        let a = Nested {
            inner: vec![1, 2, 3],
        };
        let b = Nested {
            inner: vec![1, 2, 3],
        };
        let c = Nested {
            inner: vec![1, 2, 4],
        };

        assert!(deep_equals(&a, &b));
        assert!(!deep_equals(&a, &c));
    }

    #[test]
    fn test_never_equals() {
        assert!(!never_equals(&42, &42));
        assert!(!never_equals(&"same", &"same"));
    }

    #[test]
    fn test_always_equals() {
        assert!(always_equals(&42, &43));
        assert!(always_equals(&"different", &"values"));
    }

    #[test]
    fn test_by_field() {
        #[derive(Clone)]
        struct User {
            id: u32,
            name: String,
        }

        let eq_by_id = by_field(|u: &User| u.id);

        let user1 = User {
            id: 1,
            name: "Alice".to_string(),
        };
        let user2 = User {
            id: 1,
            name: "Bob".to_string(),
        };
        let user3 = User {
            id: 2,
            name: "Alice".to_string(),
        };

        // Same ID = equal (even with different names)
        assert!(eq_by_id(&user1, &user2));
        // Different ID = not equal (even with same name)
        assert!(!eq_by_id(&user1, &user3));
    }

    #[test]
    fn test_equality_fn_constructors() {
        let eq: EqualsFn<i32> = default_equals_fn();
        assert!(eq(&42, &42));
        assert!(!eq(&42, &43));

        let never: EqualsFn<i32> = never_equals_fn();
        assert!(!never(&42, &42));

        let always: EqualsFn<i32> = always_equals_fn();
        assert!(always(&42, &43));
    }
}
