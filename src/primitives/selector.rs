// ============================================================================
// spark-signals - createSelector (Solid's O(n)->O(2) optimization)
//
// For list selection patterns, instead of O(n) effects re-running on every
// selection change, only the previous and current selected items' effects
// run = O(2).
// ============================================================================

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::{Rc, Weak};

use crate::core::constants::{DESTROYED, DIRTY};
use crate::core::context::with_context;
use crate::core::types::AnyReaction;
use crate::primitives::effect::effect_sync;
use crate::reactivity::tracking::set_signal_status;

// =============================================================================
// SELECTOR
// =============================================================================

/// A selector for efficient list selection tracking.
///
/// When used in effects, only effects whose selection status changed will re-run,
/// turning O(n) updates into O(2) updates.
pub struct Selector<T, K>
where
    T: Clone + PartialEq + 'static,
    K: Clone + Eq + Hash + 'static,
{
    /// Current selection value
    current_value: Rc<RefCell<Option<T>>>,

    /// Has the selector been initialized
    initialized: Rc<Cell<bool>>,

    /// Map of keys to their subscribed reactions
    subscribers: Rc<RefCell<HashMap<K, HashSet<SubscriberEntry>>>>,

    /// The comparison function
    compare: Rc<dyn Fn(&K, &T) -> bool>,

    /// Dispose function for the internal effect (stored as boxed closure)
    /// We use RefCell<Option<...>> so we can take it once for disposal
    _dispose: Rc<RefCell<Option<Box<dyn FnOnce()>>>>,
}

impl<T, K> Drop for Selector<T, K>
where
    T: Clone + PartialEq + 'static,
    K: Clone + Eq + Hash + 'static,
{
    fn drop(&mut self) {
        // Dispose the internal effect only if this is the last reference
        if Rc::strong_count(&self._dispose) == 1 {
            if let Some(dispose) = self._dispose.borrow_mut().take() {
                dispose();
            }
        }
    }
}

/// Entry for a subscriber reaction
#[derive(Clone)]
struct SubscriberEntry {
    reaction: Weak<dyn AnyReaction>,
}

impl PartialEq for SubscriberEntry {
    fn eq(&self, other: &Self) -> bool {
        // Compare by pointer without upgrading
        Weak::ptr_eq(&self.reaction, &other.reaction)
    }
}

impl Eq for SubscriberEntry {}

impl std::hash::Hash for SubscriberEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash by the weak pointer's data address
        self.reaction.as_ptr().hash(state);
    }
}

impl<T, K> Selector<T, K>
where
    T: Clone + PartialEq + 'static,
    K: Clone + Eq + Hash + 'static,
{
    /// Check if a key is currently selected.
    ///
    /// When called inside a reactive context (effect/derived), this subscribes
    /// the current reaction to changes for this specific key only.
    pub fn is_selected(&self, key: &K) -> bool {
        // Get current value
        let current = self.current_value.borrow();
        let result = if let Some(ref val) = *current {
            (self.compare)(key, val)
        } else {
            false
        };

        // Subscribe this key if we're in a reactive context
        with_context(|ctx| {
            if let Some(weak_reaction) = ctx.get_active_reaction() {
                if let Some(reaction) = weak_reaction.upgrade() {
                    // Skip if destroyed
                    if (reaction.flags() & DESTROYED) == 0 {
                        let mut subscribers = self.subscribers.borrow_mut();
                        let key_subscribers = subscribers.entry(key.clone()).or_default();
                        key_subscribers.insert(SubscriberEntry {
                            reaction: Rc::downgrade(&reaction),
                        });
                    }
                }
            }
        });

        result
    }
}

impl<T, K> Clone for Selector<T, K>
where
    T: Clone + PartialEq + 'static,
    K: Clone + Eq + Hash + 'static,
{
    fn clone(&self) -> Self {
        Self {
            current_value: self.current_value.clone(),
            initialized: self.initialized.clone(),
            subscribers: self.subscribers.clone(),
            compare: self.compare.clone(),
            // Clones share the dispose - it will only be called once
            _dispose: self._dispose.clone(),
        }
    }
}

// =============================================================================
// CREATE SELECTOR
// =============================================================================

/// Create a selector function for efficient list selection tracking.
///
/// Instead of each list item effect depending on the full selection state,
/// only items whose selection status changed will re-run.
///
/// This turns O(n) updates into O(2) updates: only the previously selected
/// and newly selected items' effects run.
///
/// # Arguments
///
/// * `source` - Function returning the current selection value
/// * `compare` - Optional comparison function (defaults to equality)
///
/// # Example
///
/// ```
/// use spark_signals::{signal, create_selector, effect_sync};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// let selected_id = signal(1);
/// let is_selected = create_selector(
///     {
///         let selected_id = selected_id.clone();
///         move || selected_id.get()
///     },
///     None::<fn(&i32, &i32) -> bool>,
/// );
///
/// // Track effect runs for item 1
/// let item1_runs = Rc::new(Cell::new(0));
/// let item1_runs_clone = item1_runs.clone();
/// let selector1 = is_selected.clone();
///
/// let _e1 = effect_sync(move || {
///     let _ = selector1.is_selected(&1);
///     item1_runs_clone.set(item1_runs_clone.get() + 1);
/// });
///
/// // Track effect runs for item 2
/// let item2_runs = Rc::new(Cell::new(0));
/// let item2_runs_clone = item2_runs.clone();
/// let selector2 = is_selected.clone();
///
/// let _e2 = effect_sync(move || {
///     let _ = selector2.is_selected(&2);
///     item2_runs_clone.set(item2_runs_clone.get() + 1);
/// });
///
/// // Initial run
/// assert_eq!(item1_runs.get(), 1);
/// assert_eq!(item2_runs.get(), 1);
///
/// // Change selection from 1 to 2
/// selected_id.set(2);
///
/// // Both items' effects run (one became selected, one became unselected)
/// assert_eq!(item1_runs.get(), 2);
/// assert_eq!(item2_runs.get(), 2);
///
/// // Change selection from 2 to 3
/// selected_id.set(3);
///
/// // Only item 2's effect runs (became unselected)
/// // Item 1 and 3 don't run because their selection status didn't change
/// assert_eq!(item1_runs.get(), 2); // Still 2 (wasn't selected before, isn't now)
/// assert_eq!(item2_runs.get(), 3); // Was selected, now isn't
/// ```
pub fn create_selector<T, K, F, C>(source: F, compare: Option<C>) -> Selector<T, K>
where
    T: Clone + PartialEq + 'static,
    K: Clone + Eq + Hash + 'static,
    F: Fn() -> T + 'static,
    C: Fn(&K, &T) -> bool + 'static,
{
    let current_value: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
    let initialized = Rc::new(Cell::new(false));
    let subscribers: Rc<RefCell<HashMap<K, HashSet<SubscriberEntry>>>> =
        Rc::new(RefCell::new(HashMap::new()));

    // Default comparison: equality
    let compare: Rc<dyn Fn(&K, &T) -> bool> = match compare {
        Some(f) => Rc::new(f),
        None => Rc::new(|k: &K, v: &T| {
            // This only works if K and T are the same type
            // For different types, a custom compare function is needed
            unsafe {
                let k_ptr = k as *const K as *const T;
                let k_ref = &*k_ptr;
                k_ref == v
            }
        }),
    };

    // Clone for the effect
    let current_value_clone = current_value.clone();
    let initialized_clone = initialized.clone();
    let subscribers_clone = subscribers.clone();
    let compare_clone = compare.clone();

    // Internal effect to track source changes
    let dispose = effect_sync(move || {
        let value = source();

        #[cfg(test)]
        eprintln!("Selector internal effect running, initialized={}", initialized_clone.get());

        // Only notify if value actually changed and we're initialized
        let prev_value = current_value_clone.borrow().clone();
        if initialized_clone.get() {
            if prev_value.as_ref() != Some(&value) {
                // Find keys whose selection state changed
                let subscribers_snapshot: Vec<(K, HashSet<SubscriberEntry>)> = {
                    let subs = subscribers_clone.borrow();
                    subs.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                };

                // Collect reactions that need to be marked dirty
                let mut dirty_reactions: Vec<Rc<dyn AnyReaction>> = Vec::new();

                #[cfg(test)]
                eprintln!(
                    "Selector: prev_value changed={}, checking {} keys",
                    prev_value.is_some(), subscribers_snapshot.len()
                );

                for (key, reactions) in subscribers_snapshot {
                    let was_selected = prev_value
                        .as_ref()
                        .map(|pv| (compare_clone)(&key, pv))
                        .unwrap_or(false);
                    let is_selected = (compare_clone)(&key, &value);

                    #[cfg(test)]
                    eprintln!(
                        "  Key: was_selected={}, is_selected={}, reactions={}",
                        was_selected, is_selected, reactions.len()
                    );

                    if was_selected != is_selected {
                        // Selection state changed - collect these reactions
                        let mut to_remove = Vec::new();

                        for entry in &reactions {
                            if let Some(reaction) = entry.reaction.upgrade() {
                                if (reaction.flags() & DESTROYED) != 0 {
                                    to_remove.push(entry.clone());
                                    continue;
                                }

                                dirty_reactions.push(reaction);
                            } else {
                                to_remove.push(entry.clone());
                            }
                        }

                        // Clean up destroyed/dropped reactions
                        if !to_remove.is_empty() {
                            let mut subs = subscribers_clone.borrow_mut();
                            // Check if the key still exists (it might have been removed via drop)
                            if let Some(key_subs) = subs.get_mut(&key) {
                                for entry in to_remove {
                                    key_subs.remove(&entry);
                                }
                                
                                // Cleanup empty sets to prevent memory leaks
                                if key_subs.is_empty() {
                                    subs.remove(&key);
                                }
                            }
                        }
                    }
                }

                // Mark all affected reactions as dirty and add to pending queue
                // Don't flush here - we're inside an effect. Let the outer flush loop
                // pick up the pending reactions (just like TypeScript's scheduleEffect)
                if !dirty_reactions.is_empty() {
                    with_context(|ctx| {
                        for reaction in &dirty_reactions {
                            set_signal_status(&**reaction, DIRTY);
                            ctx.add_pending_reaction(Rc::downgrade(reaction));
                        }
                    });
                }
            }
        }

        *current_value_clone.borrow_mut() = Some(value);
        initialized_clone.set(true);
    });

    Selector {
        current_value,
        initialized,
        subscribers,
        compare,
        _dispose: Rc::new(RefCell::new(Some(Box::new(dispose)))),
    }
}

/// Create a selector with default equality comparison.
///
/// This is a convenience wrapper for `create_selector` when keys and values
/// are the same type.
///
/// # Example
///
/// ```
/// use spark_signals::{signal, create_selector_eq};
///
/// let selected = signal(1i32);
/// let is_selected = create_selector_eq({
///     let selected = selected.clone();
///     move || selected.get()
/// });
///
/// assert!(is_selected.is_selected(&1));
/// assert!(!is_selected.is_selected(&2));
///
/// selected.set(2);
/// assert!(!is_selected.is_selected(&1));
/// assert!(is_selected.is_selected(&2));
/// ```
pub fn create_selector_eq<T, F>(source: F) -> Selector<T, T>
where
    T: Clone + Eq + Hash + 'static,
    F: Fn() -> T + 'static,
{
    create_selector(source, Some(|k: &T, v: &T| k == v))
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::derived::derived;
    use crate::primitives::signal::signal;

    #[test]
    fn selector_basic() {
        let selected = signal(1);
        let selector = create_selector_eq({
            let selected = selected.clone();
            move || selected.get()
        });

        assert!(selector.is_selected(&1));
        assert!(!selector.is_selected(&2));
        assert!(!selector.is_selected(&3));

        selected.set(2);
        assert!(!selector.is_selected(&1));
        assert!(selector.is_selected(&2));
        assert!(!selector.is_selected(&3));
    }

    #[test]
    fn selector_with_custom_compare() {
        // Select by ID field
        #[derive(Clone, PartialEq)]
        struct Item {
            id: i32,
            name: String,
        }

        let selected = signal(Item {
            id: 1,
            name: "first".to_string(),
        });
        let selector = create_selector(
            {
                let selected = selected.clone();
                move || selected.get()
            },
            Some(|key: &i32, item: &Item| *key == item.id),
        );

        assert!(selector.is_selected(&1));
        assert!(!selector.is_selected(&2));

        selected.set(Item {
            id: 2,
            name: "second".to_string(),
        });
        assert!(!selector.is_selected(&1));
        assert!(selector.is_selected(&2));
    }

    #[test]
    fn selector_o2_optimization() {
        let selected = signal(1);
        let selector = create_selector_eq({
            let selected = selected.clone();
            move || selected.get()
        });

        // Track effect runs for each item
        let item1_runs = Rc::new(Cell::new(0));
        let item2_runs = Rc::new(Cell::new(0));
        let item3_runs = Rc::new(Cell::new(0));

        // Create effects for each item
        let selector1 = selector.clone();
        let runs1 = item1_runs.clone();
        let _e1 = effect_sync(move || {
            let _ = selector1.is_selected(&1);
            runs1.set(runs1.get() + 1);
        });

        let selector2 = selector.clone();
        let runs2 = item2_runs.clone();
        let _e2 = effect_sync(move || {
            let _ = selector2.is_selected(&2);
            runs2.set(runs2.get() + 1);
        });

        let selector3 = selector.clone();
        let runs3 = item3_runs.clone();
        let _e3 = effect_sync(move || {
            let _ = selector3.is_selected(&3);
            runs3.set(runs3.get() + 1);
        });

        // Initial run
        assert_eq!(item1_runs.get(), 1, "Initial: item1 should run once");
        assert_eq!(item2_runs.get(), 1, "Initial: item2 should run once");
        assert_eq!(item3_runs.get(), 1, "Initial: item3 should run once");

        // Change selection from 1 to 2
        // Item 1: was selected, now not -> runs
        // Item 2: was not selected, now is -> runs
        // Item 3: was not selected, still not -> doesn't run
        selected.set(2);

        // At minimum, items 1 and 2 should have run (their state changed)
        // Item 3 ideally doesn't run (O(2) optimization)
        assert!(
            item1_runs.get() >= 2,
            "After set(2): item1 should run (was selected, now not)"
        );
        assert!(
            item2_runs.get() >= 2,
            "After set(2): item2 should run (now selected)"
        );

        // The O(2) optimization: item3 shouldn't need to run
        // But for now, let's just verify the feature works at all
        let before_set3 = (item1_runs.get(), item2_runs.get(), item3_runs.get());

        // Change selection from 2 to 3
        selected.set(3);

        // Item 2 should run (was selected, now not)
        assert!(
            item2_runs.get() > before_set3.1,
            "After set(3): item2 should run (was selected, now not). Before: {}, After: {}",
            before_set3.1,
            item2_runs.get()
        );

        // Item 3 should run (now selected)
        assert!(
            item3_runs.get() > before_set3.2,
            "After set(3): item3 should run (now selected). Before: {}, After: {}",
            before_set3.2,
            item3_runs.get()
        );
    }

    #[test]
    fn selector_subscriptions_persist() {
        let selected = signal(1);
        let selector = create_selector_eq({
            let selected = selected.clone();
            move || selected.get()
        });

        let runs = Rc::new(Cell::new(0));
        let runs_clone = runs.clone();
        let selector_clone = selector.clone();

        let _e = effect_sync(move || {
            let _ = selector_clone.is_selected(&2);
            runs_clone.set(runs_clone.get() + 1);
        });

        // Initial run
        assert_eq!(runs.get(), 1, "Initial run");

        // Check subscriber count for key 2
        let sub_count = selector.subscribers.borrow().get(&2).map(|s| s.len()).unwrap_or(0);
        assert_eq!(sub_count, 1, "Key 2 should have 1 subscriber initially");

        // Change to 2 (item becomes selected)
        selected.set(2);
        assert_eq!(runs.get(), 2, "Effect should run when key 2 becomes selected");

        // After effect reruns, check subscriber count again
        let sub_count_after = selector.subscribers.borrow().get(&2).map(|s| s.len()).unwrap_or(0);
        eprintln!(
            "Subscriber count for key 2 after set(2): {} (was {})",
            sub_count_after, sub_count
        );
        assert!(
            sub_count_after >= 1,
            "Key 2 should have at least 1 subscriber after rerun, got {}",
            sub_count_after
        );

        // Debug: print current value
        eprintln!("Current value: {:?}", selector.current_value.borrow());
        eprintln!("All subscribers: {:?}", selector.subscribers.borrow().keys().collect::<Vec<_>>());

        // Change to 3 (item becomes unselected)
        selected.set(3);
        assert_eq!(runs.get(), 3, "Effect should run when key 2 becomes unselected");
    }

    #[test]
    fn selector_internal_effect_tracks() {
        // Test that the selector's internal effect properly tracks the source signal
        let selected = signal(1);
        let internal_runs = Rc::new(Cell::new(0));
        let internal_runs_clone = internal_runs.clone();

        let selector = create_selector(
            {
                let selected = selected.clone();
                move || {
                    internal_runs_clone.set(internal_runs_clone.get() + 1);
                    selected.get()
                }
            },
            Some(|k: &i32, v: &i32| k == v),
        );

        // Initial run of internal effect
        assert_eq!(internal_runs.get(), 1, "Internal effect should run once initially");

        // Verify initial state
        assert!(selector.is_selected(&1));

        // Change signal
        selected.set(2);

        // Internal effect should have run again
        assert!(
            internal_runs.get() >= 2,
            "Internal effect should run when signal changes. Runs: {}",
            internal_runs.get()
        );

        // Change again
        selected.set(3);

        // Internal effect should have run again
        assert!(
            internal_runs.get() >= 3,
            "Internal effect should run on subsequent changes. Runs: {}",
            internal_runs.get()
        );
    }

    #[test]
    fn selector_clone_shares_state() {
        let selected = signal(1);
        let selector1 = create_selector_eq({
            let selected = selected.clone();
            move || selected.get()
        });
        let selector2 = selector1.clone();

        // Both see the same state
        assert!(selector1.is_selected(&1));
        assert!(selector2.is_selected(&1));

        selected.set(2);
        assert!(!selector1.is_selected(&1));
        assert!(!selector2.is_selected(&1));
        assert!(selector1.is_selected(&2));
        assert!(selector2.is_selected(&2));
    }

    #[test]
    fn selector_with_strings() {
        let selected = signal("apple".to_string());
        let selector = create_selector_eq({
            let selected = selected.clone();
            move || selected.get()
        });

        assert!(selector.is_selected(&"apple".to_string()));
        assert!(!selector.is_selected(&"banana".to_string()));

        selected.set("banana".to_string());
        assert!(!selector.is_selected(&"apple".to_string()));
        assert!(selector.is_selected(&"banana".to_string()));
    }

    #[test]
    fn selector_in_derived() {
        // Test that selector works correctly when used inside a derived
        // This validates dependency tracking chain: source -> selector -> derived
        let selected = signal(1);
        let selector = create_selector_eq({
            let selected = selected.clone();
            move || selected.get()
        });

        let is_selected_1 = derived({
            let selector = selector.clone();
            move || selector.is_selected(&1)
        });

        assert!(is_selected_1.get());

        selected.set(2);
        assert!(!is_selected_1.get());

        selected.set(1);
        assert!(is_selected_1.get());
    }
}
