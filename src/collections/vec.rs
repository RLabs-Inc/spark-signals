// ============================================================================
// spark-signals - ReactiveVec
// A Vec with fine-grained per-index reactivity
// Rust-specific addition (TypeScript uses array proxies instead)
// ============================================================================

use std::ops::{Index, IndexMut};
use std::rc::Rc;
use std::slice::{Iter, IterMut};

use crate::core::context::with_context;
use crate::core::types::{AnySource, SourceInner};
use crate::reactivity::tracking::{notify_write, track_read};

// =============================================================================
// REACTIVE VEC
// =============================================================================

/// A reactive Vec with per-index granularity.
///
/// Three levels of reactivity:
/// 1. Per-index signals: `vec.get(0)` only tracks that specific index
/// 2. Version signal: Tracks structural changes (push/pop/insert/remove/splice)
/// 3. Length signal: Tracks vec length changes
///
/// # Example
///
/// ```
/// use spark_signals::collections::ReactiveVec;
///
/// let mut items: ReactiveVec<String> = ReactiveVec::new();
///
/// // Push some items
/// items.push("first".to_string());
/// items.push("second".to_string());
///
/// // Get by index (tracks specific index)
/// assert_eq!(items.get(0), Some(&"first".to_string()));
///
/// // Check length (tracks length signal)
/// assert_eq!(items.len(), 2);
///
/// // Iterate (tracks version signal)
/// for item in items.iter() {
///     println!("{}", item);
/// }
///
/// // Modify
/// items.set(0, "updated".to_string());
/// assert_eq!(items.get(0), Some(&"updated".to_string()));
/// ```
pub struct ReactiveVec<T> {
    /// The underlying data
    data: Vec<T>,

    /// Per-index signals (version number incremented on change)
    /// We use a sparse representation - only create signals for accessed indices
    index_signals: std::collections::HashMap<usize, Rc<SourceInner<i32>>>,

    /// Version signal for structural changes
    version: Rc<SourceInner<i32>>,

    /// Length signal
    length: Rc<SourceInner<usize>>,
}

impl<T> ReactiveVec<T> {
    /// Create a new empty reactive vec.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            index_signals: std::collections::HashMap::new(),
            version: Rc::new(SourceInner::new(0)),
            length: Rc::new(SourceInner::new(0)),
        }
    }

    /// Create a reactive vec with initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            index_signals: std::collections::HashMap::with_capacity(capacity),
            version: Rc::new(SourceInner::new(0)),
            length: Rc::new(SourceInner::new(0)),
        }
    }

    /// Create a reactive vec from an existing vec.
    pub fn from_vec(data: Vec<T>) -> Self {
        let len = data.len();
        Self {
            data,
            index_signals: std::collections::HashMap::new(),
            version: Rc::new(SourceInner::new(0)),
            length: Rc::new(SourceInner::new(len)),
        }
    }

    /// Create a reactive vec from an iterator.
    pub fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let data: Vec<T> = iter.into_iter().collect();
        let len = data.len();
        Self {
            data,
            index_signals: std::collections::HashMap::new(),
            version: Rc::new(SourceInner::new(0)),
            length: Rc::new(SourceInner::new(len)),
        }
    }

    /// Get or create a signal for an index.
    fn get_index_signal(&mut self, index: usize) -> Rc<SourceInner<i32>> {
        if let Some(sig) = self.index_signals.get(&index) {
            sig.clone()
        } else {
            let sig = Rc::new(SourceInner::new(0));
            self.index_signals.insert(index, sig.clone());
            sig
        }
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

    /// Set length and notify.
    fn set_length(&self, new_len: usize) {
        self.length.set(new_len);

        with_context(|ctx| {
            let wv = ctx.increment_write_version();
            self.length.set_write_version(wv);
        });
        notify_write(self.length.clone() as Rc<dyn AnySource>);
    }

    /// Increment version and notify.
    fn increment_version(&self) {
        Self::increment(&self.version);
    }

    /// Notify that an index changed.
    fn notify_index(&mut self, index: usize) {
        let sig = self.get_index_signal(index);
        Self::increment(&sig);
    }

    /// Notify that indices changed from start onwards.
    fn notify_indices_from(&mut self, start: usize) {
        for (&idx, sig) in &self.index_signals {
            if idx >= start {
                Self::increment(sig);
            }
        }
    }

    // =========================================================================
    // LENGTH
    // =========================================================================

    /// Returns the number of elements in the vec.
    ///
    /// Reading length tracks the length signal.
    pub fn len(&self) -> usize {
        track_read(self.length.clone() as Rc<dyn AnySource>);
        self.data.len()
    }

    /// Returns true if the vec contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the capacity of the vec.
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    // =========================================================================
    // GET
    // =========================================================================

    /// Returns a reference to the element at the given index.
    ///
    /// If the index is valid, tracks the index signal.
    /// If the index is invalid, tracks the version signal (for future changes).
    pub fn get(&self, index: usize) -> Option<&T> {
        // Check if we have a signal for this index
        if let Some(sig) = self.index_signals.get(&index) {
            track_read(sig.clone() as Rc<dyn AnySource>);
            return self.data.get(index);
        }

        // No signal yet
        let val = self.data.get(index);

        if val.is_some() {
            // Index exists but no signal - track version
            track_read(self.version.clone() as Rc<dyn AnySource>);
        } else {
            // Index doesn't exist, track version for future changes
            track_read(self.version.clone() as Rc<dyn AnySource>);
        }

        val
    }

    /// Returns a reference to the element at the given index, creating an index signal.
    ///
    /// This is more efficient for repeated access to the same index.
    pub fn get_tracked(&mut self, index: usize) -> Option<&T> {
        if self.data.get(index).is_some() {
            let sig = self.get_index_signal(index);
            track_read(sig as Rc<dyn AnySource>);
            return self.data.get(index);
        }

        // Index doesn't exist, track version
        track_read(self.version.clone() as Rc<dyn AnySource>);
        None
    }

    /// Returns a mutable reference to the element at the given index.
    ///
    /// **Note**: Mutations through this reference won't automatically trigger updates.
    /// Use `set()` for reactive mutations.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index)
    }

    /// Returns the first element.
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    /// Returns the last element.
    pub fn last(&self) -> Option<&T> {
        if self.data.is_empty() {
            track_read(self.version.clone() as Rc<dyn AnySource>);
            None
        } else {
            self.get(self.data.len() - 1)
        }
    }

    // =========================================================================
    // SET
    // =========================================================================

    /// Sets the value at the given index.
    ///
    /// Returns the old value if the index was valid.
    /// Panics if the index is out of bounds.
    pub fn set(&mut self, index: usize, value: T) -> T
    where
        T: 'static,
    {
        let old = std::mem::replace(&mut self.data[index], value);
        self.notify_index(index);
        old
    }

    /// Sets the value at the given index if it exists.
    ///
    /// Returns the old value if the index was valid, None otherwise.
    pub fn try_set(&mut self, index: usize, value: T) -> Option<T>
    where
        T: 'static,
    {
        if index < self.data.len() {
            Some(self.set(index, value))
        } else {
            None
        }
    }

    // =========================================================================
    // PUSH / POP
    // =========================================================================

    /// Appends an element to the back of the vec.
    pub fn push(&mut self, value: T)
    where
        T: 'static,
    {
        let new_len = self.data.len() + 1;
        self.data.push(value);

        // Notify the new index
        self.notify_index(new_len - 1);
        self.set_length(new_len);
        self.increment_version();
    }

    /// Removes the last element and returns it, or `None` if empty.
    pub fn pop(&mut self) -> Option<T>
    where
        T: 'static,
    {
        if let Some(value) = self.data.pop() {
            let old_len = self.data.len() + 1;
            let new_len = self.data.len();

            // Notify and remove the index signal for the removed element
            if let Some(sig) = self.index_signals.remove(&(old_len - 1)) {
                Self::increment(&sig);
                // Signal is now removed from index_signals, and since we just
                // incremented it, any effects tracking it will rerun.
                // When they rerun, they'll see the index is now out of bounds
                // and start tracking version instead.
            }

            self.set_length(new_len);
            self.increment_version();

            Some(value)
        } else {
            None
        }
    }

    // =========================================================================
    // INSERT / REMOVE
    // =========================================================================

    /// Inserts an element at position `index`, shifting all elements after it to the right.
    ///
    /// # Panics
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: usize, value: T)
    where
        T: 'static,
    {
        self.data.insert(index, value);

        // Notify the inserted index and all shifted indices
        self.notify_indices_from(index);
        self.set_length(self.data.len());
        self.increment_version();
    }

    /// Removes and returns the element at position `index`, shifting all elements after it to the left.
    ///
    /// # Panics
    /// Panics if `index >= len`.
    pub fn remove(&mut self, index: usize) -> T
    where
        T: 'static,
    {
        let value = self.data.remove(index);

        // Notify the removed index and all shifted indices
        self.notify_indices_from(index);
        self.set_length(self.data.len());
        self.increment_version();

        value
    }

    /// Removes and returns the element at position `index` if it exists.
    pub fn try_remove(&mut self, index: usize) -> Option<T>
    where
        T: 'static,
    {
        if index < self.data.len() {
            Some(self.remove(index))
        } else {
            None
        }
    }

    // =========================================================================
    // SWAP REMOVE
    // =========================================================================

    /// Removes an element at position `index` and returns it, replacing it with the last element.
    ///
    /// This is O(1) but doesn't preserve ordering.
    ///
    /// # Panics
    /// Panics if `index >= len`.
    pub fn swap_remove(&mut self, index: usize) -> T
    where
        T: 'static,
    {
        let last_index = self.data.len() - 1;
        let value = self.data.swap_remove(index);

        // Notify the removed index and the moved element (if different)
        self.notify_index(index);
        if index != last_index {
            // Last element moved to index
            if let Some(sig) = self.index_signals.get(&last_index) {
                Self::increment(sig);
            }
        }

        self.set_length(self.data.len());
        self.increment_version();

        value
    }

    // =========================================================================
    // CLEAR / TRUNCATE
    // =========================================================================

    /// Clears the vec, removing all values.
    pub fn clear(&mut self) {
        if !self.data.is_empty() {
            // Notify and remove all tracked index signals
            for sig in self.index_signals.values() {
                Self::increment(sig);
            }
            self.index_signals.clear();

            self.data.clear();
            self.set_length(0);
            self.increment_version();
        }
    }

    /// Shortens the vec, keeping the first `len` elements and dropping the rest.
    pub fn truncate(&mut self, len: usize)
    where
        T: 'static,
    {
        if len < self.data.len() {
            // Notify and remove index signals for indices being removed
            let to_remove: Vec<usize> = self.index_signals.keys()
                .filter(|&&idx| idx >= len)
                .cloned()
                .collect();
            
            for idx in to_remove {
                if let Some(sig) = self.index_signals.remove(&idx) {
                    Self::increment(&sig);
                }
            }

            self.data.truncate(len);
            self.set_length(len);
            self.increment_version();
        }
    }

    // =========================================================================
    // RETAIN
    // =========================================================================

    /// Retains only the elements specified by the predicate.
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&T) -> bool,
        T: 'static,
    {
        let old_len = self.data.len();
        self.data.retain(f);
        let new_len = self.data.len();

        if new_len != old_len {
            // Some elements were removed - notify all indices
            // (We don't know which ones, so be conservative)
            for sig in self.index_signals.values() {
                Self::increment(sig);
            }

            self.set_length(new_len);
            self.increment_version();
        }
    }

    // =========================================================================
    // EXTEND / APPEND
    // =========================================================================

    /// Extends the vec with the contents of an iterator.
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I)
    where
        T: 'static,
    {
        let start_len = self.data.len();
        self.data.extend(iter);
        let new_len = self.data.len();

        if new_len != start_len {
            // Notify new indices
            for i in start_len..new_len {
                self.notify_index(i);
            }

            self.set_length(new_len);
            self.increment_version();
        }
    }

    /// Appends all elements from another vec.
    pub fn append(&mut self, other: &mut Vec<T>)
    where
        T: 'static,
    {
        if !other.is_empty() {
            let start_len = self.data.len();
            self.data.append(other);
            let new_len = self.data.len();

            // Notify new indices
            for i in start_len..new_len {
                self.notify_index(i);
            }

            self.set_length(new_len);
            self.increment_version();
        }
    }

    // =========================================================================
    // ITERATION (tracks version)
    // =========================================================================

    /// Returns an iterator over the elements.
    ///
    /// Tracks the version signal (re-runs effect if any structural change).
    pub fn iter(&self) -> Iter<'_, T> {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        self.data.iter()
    }

    /// Returns a mutable iterator over the elements.
    ///
    /// Tracks the version signal.
    /// **Note**: Mutations through this iterator won't automatically trigger updates.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        self.data.iter_mut()
    }

    /// Iterates over each element.
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
    // UTILITIES
    // =========================================================================

    /// Gets the underlying data without tracking.
    ///
    /// Use sparingly - this bypasses reactivity.
    pub fn raw(&self) -> &Vec<T> {
        &self.data
    }

    /// Gets mutable access to underlying data without tracking.
    ///
    /// **Warning**: Mutations here won't trigger reactive updates!
    pub fn raw_mut(&mut self) -> &mut Vec<T> {
        &mut self.data
    }

    /// Converts into the underlying Vec.
    pub fn into_inner(self) -> Vec<T> {
        self.data
    }

    /// Returns a slice of the underlying data.
    ///
    /// Tracks the version signal.
    pub fn as_slice(&self) -> &[T] {
        track_read(self.version.clone() as Rc<dyn AnySource>);
        self.data.as_slice()
    }

    /// Reverses the order of elements in the vec.
    pub fn reverse(&mut self)
    where
        T: 'static,
    {
        if self.data.len() > 1 {
            self.data.reverse();

            // Notify all tracked indices
            for sig in self.index_signals.values() {
                Self::increment(sig);
            }

            self.increment_version();
        }
    }

    /// Sorts the vec.
    pub fn sort(&mut self)
    where
        T: Ord + 'static,
    {
        if self.data.len() > 1 {
            self.data.sort();

            // Notify all tracked indices
            for sig in self.index_signals.values() {
                Self::increment(sig);
            }

            self.increment_version();
        }
    }

    /// Sorts the vec with a custom comparator.
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> std::cmp::Ordering,
        T: 'static,
    {
        if self.data.len() > 1 {
            self.data.sort_by(compare);

            // Notify all tracked indices
            for sig in self.index_signals.values() {
                Self::increment(sig);
            }

            self.increment_version();
        }
    }

    /// Sorts the vec by a key function.
    pub fn sort_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
        T: 'static,
    {
        if self.data.len() > 1 {
            self.data.sort_by_key(f);

            // Notify all tracked indices
            for sig in self.index_signals.values() {
                Self::increment(sig);
            }

            self.increment_version();
        }
    }
}

impl<T> Default for ReactiveVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for ReactiveVec<T> {
    fn clone(&self) -> Self {
        // Create a new reactive vec with same data but fresh signals
        Self::from_vec(self.data.clone())
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for ReactiveVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReactiveVec")
            .field("data", &self.data)
            .field("len", &self.data.len())
            .finish()
    }
}

impl<T> Index<usize> for ReactiveVec<T> {
    type Output = T;

    /// Index access (non-reactive).
    ///
    /// For reactive access, use `get()`.
    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T> IndexMut<usize> for ReactiveVec<T> {
    /// Mutable index access (non-reactive).
    ///
    /// For reactive mutations, use `set()`.
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
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
    fn create_empty_vec() {
        let vec: ReactiveVec<i32> = ReactiveVec::new();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn create_from_vec() {
        let vec = ReactiveVec::from_vec(vec![1, 2, 3]);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.get(0), Some(&1));
        assert_eq!(vec.get(1), Some(&2));
        assert_eq!(vec.get(2), Some(&3));
    }

    #[test]
    fn push_and_pop() {
        let mut vec: ReactiveVec<i32> = ReactiveVec::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);
        assert_eq!(vec.len(), 3);

        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.len(), 1);
    }

    #[test]
    fn insert_and_remove() {
        let mut vec = ReactiveVec::from_vec(vec![1, 3, 4]);

        vec.insert(1, 2);
        assert_eq!(vec.raw(), &vec![1, 2, 3, 4]);

        let removed = vec.remove(2);
        assert_eq!(removed, 3);
        assert_eq!(vec.raw(), &vec![1, 2, 4]);
    }

    #[test]
    fn set() {
        let mut vec = ReactiveVec::from_vec(vec![1, 2, 3]);

        let old = vec.set(1, 20);
        assert_eq!(old, 2);
        assert_eq!(vec.get(1), Some(&20));
    }

    #[test]
    fn first_and_last() {
        let vec = ReactiveVec::from_vec(vec![1, 2, 3]);
        assert_eq!(vec.first(), Some(&1));
        assert_eq!(vec.last(), Some(&3));

        let empty: ReactiveVec<i32> = ReactiveVec::new();
        assert_eq!(empty.first(), None);
        assert_eq!(empty.last(), None);
    }

    #[test]
    fn clear() {
        let mut vec = ReactiveVec::from_vec(vec![1, 2, 3]);
        vec.clear();
        assert!(vec.is_empty());
    }

    #[test]
    fn truncate() {
        let mut vec = ReactiveVec::from_vec(vec![1, 2, 3, 4, 5]);
        vec.truncate(3);
        assert_eq!(vec.raw(), &vec![1, 2, 3]);
    }

    #[test]
    fn swap_remove() {
        let mut vec = ReactiveVec::from_vec(vec![1, 2, 3, 4, 5]);
        let removed = vec.swap_remove(1);
        assert_eq!(removed, 2);
        // 5 moved to index 1
        assert_eq!(vec.raw(), &vec![1, 5, 3, 4]);
    }

    #[test]
    fn retain() {
        let mut vec = ReactiveVec::from_vec(vec![1, 2, 3, 4, 5]);
        vec.retain(|&x| x % 2 == 1); // Keep odd numbers
        assert_eq!(vec.raw(), &vec![1, 3, 5]);
    }

    #[test]
    fn extend_and_append() {
        let mut vec = ReactiveVec::from_vec(vec![1, 2]);

        vec.extend([3, 4]);
        assert_eq!(vec.raw(), &vec![1, 2, 3, 4]);

        let mut other = vec![5, 6];
        vec.append(&mut other);
        assert_eq!(vec.raw(), &vec![1, 2, 3, 4, 5, 6]);
        assert!(other.is_empty());
    }

    #[test]
    fn iteration() {
        let vec = ReactiveVec::from_vec(vec![1, 2, 3, 4, 5]);
        let sum: i32 = vec.iter().sum();
        assert_eq!(sum, 15);
    }

    #[test]
    fn reverse_and_sort() {
        let mut vec = ReactiveVec::from_vec(vec![3, 1, 4, 1, 5]);

        vec.sort();
        assert_eq!(vec.raw(), &vec![1, 1, 3, 4, 5]);

        vec.reverse();
        assert_eq!(vec.raw(), &vec![5, 4, 3, 1, 1]);
    }

    #[test]
    fn effect_tracks_length() {
        use crate::batch;

        let vec: ReactiveVec<i32> = ReactiveVec::new();
        let vec_rc: Rc<RefCell<ReactiveVec<i32>>> = Rc::new(RefCell::new(vec));

        let lengths: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
        let lengths_clone = lengths.clone();
        let vec_clone = vec_rc.clone();

        // Keep the effect alive
        let _effect = effect_sync(move || {
            let len = (*vec_clone).borrow().len();
            (*lengths_clone).borrow_mut().push(len);
        });

        // Initial: 0
        assert_eq!(*(*lengths).borrow(), vec![0]);

        // Use batch to defer effect until borrow is released
        batch(|| {
            (*vec_rc).borrow_mut().push(1);
        });
        assert_eq!(*(*lengths).borrow(), vec![0, 1]);

        batch(|| {
            (*vec_rc).borrow_mut().push(2);
        });
        assert_eq!(*(*lengths).borrow(), vec![0, 1, 2]);

        batch(|| {
            (*vec_rc).borrow_mut().pop();
        });
        assert_eq!(*(*lengths).borrow(), vec![0, 1, 2, 1]);
    }

    #[test]
    fn effect_tracks_iteration() {
        use crate::batch;

        let vec: ReactiveVec<i32> = ReactiveVec::new();
        let vec_rc: Rc<RefCell<ReactiveVec<i32>>> = Rc::new(RefCell::new(vec));

        let call_count = Rc::new(Cell::new(0));
        let call_count_clone = call_count.clone();
        let vec_clone = vec_rc.clone();

        // Keep the effect alive
        let _effect = effect_sync(move || {
            call_count_clone.set(call_count_clone.get() + 1);
            // Iterate (tracks version)
            for _ in (*vec_clone).borrow().iter() {}
        });

        assert_eq!(call_count.get(), 1);

        // Use batch to defer effect until borrow is released
        batch(|| {
            (*vec_rc).borrow_mut().push(1);
        });
        assert_eq!(call_count.get(), 2);

        batch(|| {
            (*vec_rc).borrow_mut().pop();
        });
        assert_eq!(call_count.get(), 3);

        // Multiple operations in one batch
        batch(|| {
            (*vec_rc).borrow_mut().push(3);
            (*vec_rc).borrow_mut().push(1);
        });
        // Batch runs effect only once after all operations
        assert_eq!(call_count.get(), 4);

        batch(|| {
            (*vec_rc).borrow_mut().sort();
        });
        assert_eq!(call_count.get(), 5);
    }

    #[test]
    fn clone_gets_independent_reactivity() {
        let vec1 = ReactiveVec::from_vec(vec![1, 2, 3]);
        let vec2 = vec1.clone();

        // They have the same data
        assert_eq!(vec1.raw(), vec2.raw());

        // But independent signals (verified by modifying one)
        // Can't easily test without shared refs, but the clone creates fresh signals
    }

    #[test]
    fn index_access() {
        let vec = ReactiveVec::from_vec(vec![1, 2, 3]);
        assert_eq!(vec[0], 1);
        assert_eq!(vec[1], 2);
        assert_eq!(vec[2], 3);
    }

    #[test]
    fn debug_format() {
        let vec = ReactiveVec::from_vec(vec![1, 2, 3]);
        let debug = format!("{:?}", vec);
        assert!(debug.contains("ReactiveVec"));
        assert!(debug.contains("[1, 2, 3]"));
    }
}
