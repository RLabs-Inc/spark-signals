// ============================================================================
// spark-signals - ReactiveSet
// A HashSet with fine-grained per-item reactivity
// Based on Svelte 5's SvelteSet
// ============================================================================

use std::borrow::Borrow;
use std::collections::hash_set::Iter;
use std::collections::HashSet;
use std::hash::Hash;
use std::rc::Rc;

use crate::core::context::with_context;
use crate::core::types::{AnySource, SourceInner};
use crate::reactivity::tracking::{notify_write, track_read};

// =============================================================================
// REACTIVE SET
// =============================================================================

/// A reactive HashSet with per-item granularity.
///
/// Three levels of reactivity:
/// 1. Per-item signals: `set.contains(&item)` only tracks that specific item
/// 2. Version signal: Tracks structural changes (insert/remove)
/// 3. Size signal: Tracks set size changes
///
/// # Example
///
/// ```
/// use spark_signals::collections::ReactiveSet;
///
/// let mut tags: ReactiveSet<String> = ReactiveSet::new();
///
/// // Add some items
/// tags.insert("important".to_string());
/// tags.insert("todo".to_string());
///
/// // Check membership (tracks specific item)
/// assert!(tags.contains(&"important".to_string()));
/// assert!(!tags.contains(&"other".to_string()));
///
/// // Check size (tracks size signal)
/// assert_eq!(tags.len(), 2);
///
/// // Iterate (tracks version signal)
/// for tag in tags.iter() {
///     println!("{}", tag);
/// }
/// ```
pub struct ReactiveSet<T>
where
    T: Eq + Hash + Clone,
{
    /// The underlying data
    data: HashSet<T>,

    /// Per-item signals (true = present, false = deleted)
    item_signals: std::collections::HashMap<T, Rc<SourceInner<bool>>>,

    /// Version signal for structural changes
    version: Rc<SourceInner<i32>>,

    /// Size signal
    size: Rc<SourceInner<usize>>,
}

impl<T> ReactiveSet<T>
where
    T: Eq + Hash + Clone,
{
    /// Create a new empty reactive set.
    pub fn new() -> Self {
        Self {
            data: HashSet::new(),
            item_signals: std::collections::HashMap::new(),
            version: Rc::new(SourceInner::new(0)),
            size: Rc::new(SourceInner::new(0)),
        }
    }

    /// Create a reactive set with initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: HashSet::with_capacity(capacity),
            item_signals: std::collections::HashMap::with_capacity(capacity),
            version: Rc::new(SourceInner::new(0)),
            size: Rc::new(SourceInner::new(0)),
        }
    }

    /// Create a reactive set from an iterator.
    pub fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let data: HashSet<T> = iter.into_iter().collect();
        let len = data.len();
        Self {
            data,
            item_signals: std::collections::HashMap::new(),
            version: Rc::new(SourceInner::new(0)),
            size: Rc::new(SourceInner::new(len)),
        }
    }

    /// Get or create a signal for an item.
    fn get_item_signal(&mut self, item: &T) -> Rc<SourceInner<bool>> {
        if let Some(sig) = self.item_signals.get(item) {
            sig.clone()
        } else {
            let exists = self.data.contains(item);
            let sig = Rc::new(SourceInner::new(exists));
            self.item_signals.insert(item.clone(), sig.clone());
            sig
        }
    }

    /// Set a signal's value and notify.
    fn set_and_notify_bool(sig: &Rc<SourceInner<bool>>, value: bool) {
        sig.set(value);

        with_context(|ctx| {
            let wv = ctx.increment_write_version();
            sig.set_write_version(wv);
        });
        notify_write(sig.clone() as Rc<dyn AnySource>);
    }

    /// Increment a signal's value and notify.
    fn increment(sig: &Rc<SourceInner<i32>>) {
        let new_val = sig.get() + 1;
        sig.set(new_val);

        with_context(|ctx| {
            let wv = ctx.increment_write_version();
            sig.set_write_version(wv);
        });
        notify_write(sig.clone() as Rc<dyn AnySource>);
    }

    /// Set size and notify.
    fn set_size(&self, new_size: usize) {
        self.size.set(new_size);

        with_context(|ctx| {
            let wv = ctx.increment_write_version();
            self.size.set_write_version(wv);
        });
        notify_write(self.size.clone() as Rc<dyn AnySource>);
    }

    /// Increment version and notify.
    fn increment_version(&self) {
        Self::increment(&self.version);
    }

    // =========================================================================
    // SIZE
    // =========================================================================

    /// Returns the number of elements in the set.
    ///
    /// Reading size tracks the size signal.
    pub fn len(&self) -> usize {
        track_read(self.size.clone() as Rc<dyn AnySource>);
        self.data.len()
    }

    /// Returns true if the set contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // =========================================================================
    // CONTAINS (has)
    // =========================================================================

    /// Returns true if the set contains the specified value.
    ///
    /// If the item exists, tracks the item signal.
    /// If the item doesn't exist, tracks the version signal (for future adds).
    pub fn contains<Q>(&self, item: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        // Check if we have a signal for this item
        if let Some(sig) = self.item_signals.get(item) {
            track_read(sig.clone() as Rc<dyn AnySource>);
            return self.data.contains(item);
        }

        // No signal yet
        let exists = self.data.contains(item);

        if exists {
            // Item exists but no signal - track version
            track_read(self.version.clone() as Rc<dyn AnySource>);
        } else {
            // Item doesn't exist, track version for future adds
            track_read(self.version.clone() as Rc<dyn AnySource>);
        }

        exists
    }

    // =========================================================================
    // CONTAINS_TRACKED - Creates signal if item exists
    // =========================================================================

    /// Returns true if the set contains the item, creating an item signal if needed.
    ///
    /// This is more efficient for repeated checks of the same item.
    pub fn contains_tracked(&mut self, item: &T) -> bool {
        if let Some(sig) = self.item_signals.get(item) {
            track_read(sig.clone() as Rc<dyn AnySource>);
            return self.data.contains(item);
        }

        let exists = self.data.contains(item);

        if exists {
            // Create signal for future tracking
            let sig = self.get_item_signal(item);
            track_read(sig as Rc<dyn AnySource>);
        } else {
            // Item doesn't exist, track version
            track_read(self.version.clone() as Rc<dyn AnySource>);
        }

        exists
    }

    // =========================================================================
    // INSERT (add)
    // =========================================================================

    /// Adds a value to the set.
    ///
    /// Returns true if the value was newly inserted.
    pub fn insert(&mut self, item: T) -> bool {
        let is_new = self.data.insert(item.clone());

        if is_new {
            let sig = self.get_item_signal(&item);
            Self::set_and_notify_bool(&sig, true);
            self.set_size(self.data.len());
            self.increment_version();
        }

        is_new
    }

    // =========================================================================
    // REMOVE (delete)
    // =========================================================================

    /// Removes a value from the set.
    ///
    /// Returns true if the value was present.
    pub fn remove<Q>(&mut self, item: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let existed = self.data.remove(item);

        if existed {
            // Mark item signal as deleted and remove it
            if let Some(sig) = self.item_signals.remove(item) {
                Self::set_and_notify_bool(&sig, false);
            }

            self.set_size(self.data.len());
            self.increment_version();
        }

        existed
    }

    /// Removes a value from the set with exact type.
    pub fn remove_exact(&mut self, item: &T) -> bool {
        let existed = self.data.remove(item);

        if existed {
            // Mark item signal as deleted
            if let Some(sig) = self.item_signals.remove(item) {
                Self::set_and_notify_bool(&sig, false);
            }

            self.set_size(self.data.len());
            self.increment_version();
        }

        existed
    }

    // =========================================================================
    // CLEAR
    // =========================================================================

    /// Clears the set, removing all values.
    pub fn clear(&mut self) {
        if !self.data.is_empty() {
            // Mark all item signals as deleted
            for sig in self.item_signals.values() {
                Self::set_and_notify_bool(sig, false);
            }
            self.item_signals.clear();

            self.data.clear();

            self.set_size(0);
            self.increment_version();
        }
    }

    // =========================================================================
    // ITERATION (tracks version)
    // =========================================================================

    /// Returns an iterator over the items.
    ///
    /// Tracks the version signal (re-runs effect if any structural change).
    pub fn iter(&self) -> Iter<'_, T> {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        self.data.iter()
    }

    /// Iterates over each item.
    ///
    /// Tracks the version signal.
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&T),
    {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        for item in &self.data {
            f(item);
        }
    }

    // =========================================================================
    // SET OPERATIONS
    // =========================================================================

    /// Returns true if self is a subset of other.
    ///
    /// Tracks the version signal.
    pub fn is_subset(&self, other: &ReactiveSet<T>) -> bool {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        track_read(other.version.clone() as Rc<dyn AnySource>);
        self.data.is_subset(&other.data)
    }

    /// Returns true if self is a superset of other.
    ///
    /// Tracks the version signal.
    pub fn is_superset(&self, other: &ReactiveSet<T>) -> bool {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        track_read(other.version.clone() as Rc<dyn AnySource>);
        self.data.is_superset(&other.data)
    }

    /// Returns true if self has no elements in common with other.
    ///
    /// Tracks the version signal.
    pub fn is_disjoint(&self, other: &ReactiveSet<T>) -> bool {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        track_read(other.version.clone() as Rc<dyn AnySource>);
        self.data.is_disjoint(&other.data)
    }

    // =========================================================================
    // UTILITIES
    // =========================================================================

    /// Gets the underlying data without tracking.
    ///
    /// Use sparingly - this bypasses reactivity.
    pub fn raw(&self) -> &HashSet<T> {
        &self.data
    }

    /// Gets mutable access to underlying data without tracking.
    ///
    /// **Warning**: Mutations here won't trigger reactive updates!
    pub fn raw_mut(&mut self) -> &mut HashSet<T> {
        &mut self.data
    }
}

impl<T> Default for ReactiveSet<T>
where
    T: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for ReactiveSet<T>
where
    T: Eq + Hash + Clone,
{
    fn clone(&self) -> Self {
        // Create a new reactive set with same data but fresh signals
        Self::from_iter(self.data.clone())
    }
}

impl<T> std::fmt::Debug for ReactiveSet<T>
where
    T: Eq + Hash + Clone + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReactiveSet")
            .field("data", &self.data)
            .field("size", &self.data.len())
            .finish()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_sync;
    use std::cell::{Cell, RefCell};

    #[test]
    fn create_empty_set() {
        let set: ReactiveSet<String> = ReactiveSet::new();
        assert_eq!(set.len(), 0);
        assert!(set.is_empty());
    }

    #[test]
    fn create_from_iter() {
        let set = ReactiveSet::from_iter(["a".to_string(), "b".to_string()]);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&"a".to_string()));
        assert!(set.contains(&"b".to_string()));
    }

    #[test]
    fn insert_and_contains() {
        let mut set: ReactiveSet<String> = ReactiveSet::new();

        let inserted = set.insert("item".to_string());
        assert!(inserted);
        assert!(set.contains(&"item".to_string()));

        // Insert same item again
        let inserted = set.insert("item".to_string());
        assert!(!inserted);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn remove() {
        let mut set: ReactiveSet<String> = ReactiveSet::new();
        set.insert("item".to_string());

        let removed = set.remove(&"item".to_string());
        assert!(removed);
        assert!(!set.contains(&"item".to_string()));
        assert_eq!(set.len(), 0);

        // Remove non-existent
        let removed = set.remove(&"item".to_string());
        assert!(!removed);
    }

    #[test]
    fn clear() {
        let mut set: ReactiveSet<String> = ReactiveSet::new();
        set.insert("a".to_string());
        set.insert("b".to_string());

        set.clear();
        assert!(set.is_empty());
    }

    #[test]
    fn iteration() {
        let mut set: ReactiveSet<i32> = ReactiveSet::new();
        set.insert(1);
        set.insert(2);
        set.insert(3);

        let sum: i32 = set.iter().sum();
        assert_eq!(sum, 6);
    }

    #[test]
    fn effect_tracks_specific_item() {
        let set: ReactiveSet<String> = ReactiveSet::new();
        let set_rc: Rc<RefCell<ReactiveSet<String>>> = Rc::new(RefCell::new(set));

        let call_count = Rc::new(Cell::new(0));
        let call_count_clone = call_count.clone();
        let set_clone = set_rc.clone();

        // Keep the effect alive
        let _effect = effect_sync(move || {
            call_count_clone.set(call_count_clone.get() + 1);
            // Check "tracked" item
            (*set_clone).borrow().contains(&"tracked".to_string());
        });

        // Initial run
        assert_eq!(call_count.get(), 1);
    }

    #[test]
    fn effect_tracks_size() {
        use crate::batch;

        let set: ReactiveSet<String> = ReactiveSet::new();
        let set_rc: Rc<RefCell<ReactiveSet<String>>> = Rc::new(RefCell::new(set));

        let sizes: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
        let sizes_clone = sizes.clone();
        let set_clone = set_rc.clone();

        // Keep the effect alive
        let _effect = effect_sync(move || {
            let len = (*set_clone).borrow().len();
            (*sizes_clone).borrow_mut().push(len);
        });

        // Initial: 0
        assert_eq!(*(*sizes).borrow(), vec![0]);

        // Use batch to defer effect until borrow is released
        batch(|| {
            (*set_rc).borrow_mut().insert("a".to_string());
        });
        assert_eq!(*(*sizes).borrow(), vec![0, 1]);

        batch(|| {
            (*set_rc).borrow_mut().insert("b".to_string());
        });
        assert_eq!(*(*sizes).borrow(), vec![0, 1, 2]);

        batch(|| {
            (*set_rc).borrow_mut().remove(&"a".to_string());
        });
        assert_eq!(*(*sizes).borrow(), vec![0, 1, 2, 1]);
    }

    #[test]
    fn effect_tracks_iteration() {
        use crate::batch;

        let set: ReactiveSet<String> = ReactiveSet::new();
        let set_rc: Rc<RefCell<ReactiveSet<String>>> = Rc::new(RefCell::new(set));

        let call_count = Rc::new(Cell::new(0));
        let call_count_clone = call_count.clone();
        let set_clone = set_rc.clone();

        // Keep the effect alive
        let _effect = effect_sync(move || {
            call_count_clone.set(call_count_clone.get() + 1);
            // Iterate (tracks version)
            for _ in (*set_clone).borrow().iter() {}
        });

        assert_eq!(call_count.get(), 1);

        // Use batch to defer effect until borrow is released
        batch(|| {
            (*set_rc).borrow_mut().insert("a".to_string());
        });
        assert_eq!(call_count.get(), 2);

        batch(|| {
            (*set_rc).borrow_mut().remove(&"a".to_string());
        });
        assert_eq!(call_count.get(), 3);
    }

    #[test]
    fn set_operations() {
        let set1 = ReactiveSet::from_iter([1, 2, 3]);
        let set2 = ReactiveSet::from_iter([2, 3, 4]);
        let set3 = ReactiveSet::from_iter([1, 2]);
        let set4 = ReactiveSet::from_iter([5, 6]);

        assert!(set3.is_subset(&set1));
        assert!(set1.is_superset(&set3));
        assert!(set1.is_disjoint(&set4));
        assert!(!set1.is_disjoint(&set2));
    }

    #[test]
    fn clone_gets_independent_reactivity() {
        let mut set1: ReactiveSet<String> = ReactiveSet::new();
        set1.insert("item".to_string());

        let set2 = set1.clone();

        // Remove from set1
        set1.remove(&"item".to_string());

        // set2 still has it (deep clone)
        assert!(set2.contains(&"item".to_string()));
    }

    #[test]
    fn debug_format() {
        let mut set: ReactiveSet<String> = ReactiveSet::new();
        set.insert("item".to_string());

        let debug = format!("{:?}", set);
        assert!(debug.contains("ReactiveSet"));
        assert!(debug.contains("item"));
    }
}
