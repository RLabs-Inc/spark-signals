// ============================================================================
// spark-signals - Type Definitions
// Type-erased traits and base types for the reactive graph
// ============================================================================

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::constants::*;

// =============================================================================
// TYPE-ERASED TRAITS
// =============================================================================
//
// These traits enable heterogeneous storage in the reactive graph.
// Key insight: graph operations (mark dirty, check version, track deps)
// don't need to know the value type T. Only reading/writing values needs T.
//
// So we can have:
// - Vec<Rc<dyn AnySource>> for dependency tracking
// - Vec<Weak<dyn AnyReaction>> for reaction notification
//
// The concrete Signal<T> and Derived<T> types hold the actual values
// and implement these traits for graph operations.
// =============================================================================

/// Type-erased source interface for reactive graph operations.
///
/// Implemented by both `SourceInner<T>` (signals) and `DerivedInner<T>` (deriveds).
/// Enables storing different signal types in the same collection.
pub trait AnySource: Any {
    /// Get the flags bitmask
    fn flags(&self) -> u32;

    /// Set the flags bitmask
    fn set_flags(&self, flags: u32);

    /// Get the write version (incremented when value changes)
    fn write_version(&self) -> u32;

    /// Set the write version
    fn set_write_version(&self, version: u32);

    /// Get the read version (for dependency deduplication)
    fn read_version(&self) -> u32;

    /// Set the read version
    fn set_read_version(&self, version: u32);

    /// Get the number of reactions depending on this source
    fn reaction_count(&self) -> usize;

    /// Add a reaction that depends on this source
    fn add_reaction(&self, reaction: Weak<dyn AnyReaction>);

    /// Remove dead (dropped) reactions from the list
    fn cleanup_dead_reactions(&self);

    /// Iterate over reactions, calling f for each live reaction.
    /// The callback receives the reaction and can return false to stop iteration.
    fn for_each_reaction(&self, f: &mut dyn FnMut(Rc<dyn AnyReaction>) -> bool);

    /// Remove a specific reaction from this source's reactions list.
    /// Used during dependency cleanup when a reaction no longer depends on this source.
    fn remove_reaction(&self, reaction: &Rc<dyn AnyReaction>);

    /// Clear all reactions from this source.
    /// Used when disconnecting a source from the reactive graph.
    fn clear_reactions(&self);

    /// Check if this is a derived (has DERIVED flag)
    fn is_derived(&self) -> bool {
        self.flags() & DERIVED != 0
    }

    /// Check if this source is dirty
    fn is_dirty(&self) -> bool {
        self.flags() & DIRTY != 0
    }

    /// Check if this source is maybe dirty
    fn is_maybe_dirty(&self) -> bool {
        self.flags() & MAYBE_DIRTY != 0
    }

    /// Check if this source is clean
    fn is_clean(&self) -> bool {
        self.flags() & CLEAN != 0
    }

    /// Mark as dirty (clear status bits, set DIRTY)
    fn mark_dirty(&self) {
        let flags = (self.flags() & STATUS_MASK) | DIRTY;
        self.set_flags(flags);
    }

    /// Mark as maybe dirty (clear status bits, set MAYBE_DIRTY)
    fn mark_maybe_dirty(&self) {
        let flags = (self.flags() & STATUS_MASK) | MAYBE_DIRTY;
        self.set_flags(flags);
    }

    /// Mark as clean (clear status bits, set CLEAN)
    fn mark_clean(&self) {
        let flags = (self.flags() & STATUS_MASK) | CLEAN;
        self.set_flags(flags);
    }

    /// Upcast to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// If this source is also a reaction (i.e., a Derived), return it as an AnyReaction.
    /// This enables the MAYBE_DIRTY optimization in updateDerivedChain.
    ///
    /// Returns None for Signals (which are not reactions).
    /// Returns Some for Deriveds (which are both sources and reactions).
    fn as_derived_reaction(&self) -> Option<Rc<dyn AnyReaction>> {
        None // Default: signals are not reactions
    }
}

/// Type-erased reaction interface for scheduling and updates.
///
/// Implemented by `EffectInner` and `DerivedInner<T>`.
/// A Reaction is something that can be notified when its dependencies change.
pub trait AnyReaction: Any {
    /// Get the flags bitmask
    fn flags(&self) -> u32;

    /// Set the flags bitmask
    fn set_flags(&self, flags: u32);

    /// Get the number of dependencies
    fn dep_count(&self) -> usize;

    /// Add a dependency (a source this reaction reads from)
    fn add_dep(&self, source: Rc<dyn AnySource>);

    /// Clear all dependencies (called before re-running to rebuild dep list)
    fn clear_deps(&self);

    /// Remove dependencies starting from index (for cleanup)
    fn remove_deps_from(&self, start: usize);

    /// Iterate over dependencies
    fn for_each_dep(&self, f: &mut dyn FnMut(&Rc<dyn AnySource>) -> bool);

    /// Remove a specific source from this reaction's deps list.
    /// Used when disconnecting a source from the reactive graph.
    fn remove_source(&self, source: &Rc<dyn AnySource>);

    /// Execute the reaction (recompute derived, run effect)
    /// Returns true if the reaction's value changed (for deriveds)
    fn update(&self) -> bool;

    /// Check if this is a derived
    fn is_derived(&self) -> bool {
        self.flags() & DERIVED != 0
    }

    /// Check if this is an effect
    fn is_effect(&self) -> bool {
        self.flags() & EFFECT != 0
    }

    /// Check if this reaction is dirty
    fn is_dirty(&self) -> bool {
        self.flags() & DIRTY != 0
    }

    /// Check if this reaction is maybe dirty
    fn is_maybe_dirty(&self) -> bool {
        self.flags() & MAYBE_DIRTY != 0
    }

    /// Check if this reaction is clean
    fn is_clean(&self) -> bool {
        self.flags() & CLEAN != 0
    }

    /// Check if this reaction is destroyed
    fn is_destroyed(&self) -> bool {
        self.flags() & DESTROYED != 0
    }

    /// Mark as dirty
    fn mark_dirty(&self) {
        let flags = (self.flags() & STATUS_MASK) | DIRTY;
        self.set_flags(flags);
    }

    /// Mark as maybe dirty
    fn mark_maybe_dirty(&self) {
        let flags = (self.flags() & STATUS_MASK) | MAYBE_DIRTY;
        self.set_flags(flags);
    }

    /// Mark as clean
    fn mark_clean(&self) {
        let flags = (self.flags() & STATUS_MASK) | CLEAN;
        self.set_flags(flags);
    }

    /// Mark as destroyed
    fn mark_destroyed(&self) {
        self.set_flags(self.flags() | DESTROYED);
    }

    /// Upcast to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// If this reaction is also a source (i.e., a Derived), return it as an AnySource.
    /// This enables cascade propagation in markReactions.
    ///
    /// Returns None for Effects (which are not sources).
    /// Returns Some for Deriveds (which are both sources and reactions).
    fn as_derived_source(&self) -> Option<Rc<dyn AnySource>>;
}

// =============================================================================
// SOURCE INNER (the data behind Signal<T>)
// =============================================================================

/// Equality function type for comparing signal values
pub type EqualsFn<T> = fn(&T, &T) -> bool;

/// Default equality using PartialEq
pub fn default_equals<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

/// The internal data for a signal source.
///
/// This is separate from Signal<T> so we can implement AnySource on it
/// and store Rc<SourceInner<T>> as Rc<dyn AnySource>.
pub struct SourceInner<T> {
    /// Flags bitmask (type + status)
    flags: Cell<u32>,

    /// The current value
    value: RefCell<T>,

    /// Write version - incremented when value changes
    write_version: Cell<u32>,

    /// Read version - for dependency deduplication during tracking
    read_version: Cell<u32>,

    /// Reactions that depend on this source (weak refs to avoid cycles)
    reactions: RefCell<Vec<Weak<dyn AnyReaction>>>,

    /// Equality function for comparing values
    equals: EqualsFn<T>,
}

impl<T> SourceInner<T> {
    /// Create a new source with the given value
    pub fn new(value: T) -> Self
    where
        T: PartialEq,
    {
        Self::new_with_equals(value, default_equals)
    }

    /// Create a new source with a custom equality function
    pub fn new_with_equals(value: T, equals: EqualsFn<T>) -> Self {
        Self {
            flags: Cell::new(SOURCE | CLEAN),
            value: RefCell::new(value),
            write_version: Cell::new(0),
            read_version: Cell::new(0),
            reactions: RefCell::new(Vec::new()),
            equals,
        }
    }

    /// Get the current value (cloning)
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.value.borrow().clone()
    }

    /// Get the current value with a closure (avoids clone)
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.value.borrow())
    }

    /// Set the value, returning true if it changed
    pub fn set(&self, value: T) -> bool {
        let changed = {
            let current = self.value.borrow();
            !(self.equals)(&current, &value)
        };

        if changed {
            *self.value.borrow_mut() = value;
            self.write_version.set(self.write_version.get() + 1);
        }

        changed
    }

    /// Update the value in place using a closure.
    /// Returns true if there are reactions listening (value may have changed).
    pub fn update(&self, f: impl FnOnce(&mut T)) -> bool {
        {
            let mut current = self.value.borrow_mut();
            f(&mut current);
        }

        // We mutated in place, so mark as changed if someone is listening
        let has_reactions = !self.reactions.borrow().is_empty();
        if has_reactions {
            self.write_version.set(self.write_version.get() + 1);
        }
        has_reactions
    }

    /// Get the equality function
    pub fn equals_fn(&self) -> EqualsFn<T> {
        self.equals
    }
}

impl<T: 'static> AnySource for SourceInner<T> {
    fn flags(&self) -> u32 {
        self.flags.get()
    }

    fn set_flags(&self, flags: u32) {
        self.flags.set(flags);
    }

    fn write_version(&self) -> u32 {
        self.write_version.get()
    }

    fn set_write_version(&self, version: u32) {
        self.write_version.set(version);
    }

    fn read_version(&self) -> u32 {
        self.read_version.get()
    }

    fn set_read_version(&self, version: u32) {
        self.read_version.set(version);
    }

    fn reaction_count(&self) -> usize {
        self.reactions.borrow().len()
    }

    fn add_reaction(&self, reaction: Weak<dyn AnyReaction>) {
        self.reactions.borrow_mut().push(reaction);
    }

    fn cleanup_dead_reactions(&self) {
        self.reactions.borrow_mut().retain(|w| w.strong_count() > 0);
    }

    fn for_each_reaction(&self, f: &mut dyn FnMut(Rc<dyn AnyReaction>) -> bool) {
        let reactions = self.reactions.borrow();
        for weak in reactions.iter() {
            if let Some(rc) = weak.upgrade() {
                if !f(rc) {
                    break;
                }
            }
        }
    }

    fn remove_reaction(&self, reaction: &Rc<dyn AnyReaction>) {
        // Compare by pointer identity: the Rc points to the same allocation
        let reaction_ptr = Rc::as_ptr(reaction) as *const ();
        self.reactions.borrow_mut().retain(|weak| {
            if let Some(rc) = weak.upgrade() {
                // Cast to raw pointers for comparison
                let weak_ptr = Rc::as_ptr(&rc) as *const ();
                weak_ptr != reaction_ptr
            } else {
                // Remove dead weak references while we're at it
                false
            }
        });
    }

    fn clear_reactions(&self) {
        self.reactions.borrow_mut().clear();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_inner_creation() {
        let source = SourceInner::new(42);
        assert_eq!(source.get(), 42);
        assert!(source.flags() & SOURCE != 0);
        assert!(source.flags() & CLEAN != 0);
    }

    #[test]
    fn source_inner_set() {
        let source = SourceInner::new(1);
        assert_eq!(source.get(), 1);

        let changed = source.set(2);
        assert!(changed);
        assert_eq!(source.get(), 2);
        assert_eq!(source.write_version(), 1);

        // Setting same value shouldn't change
        let changed = source.set(2);
        assert!(!changed);
        assert_eq!(source.write_version(), 1);
    }

    #[test]
    fn source_inner_with() {
        let source = SourceInner::new(vec![1, 2, 3]);
        let sum = source.with(|v| v.iter().sum::<i32>());
        assert_eq!(sum, 6);
    }

    #[test]
    fn source_as_any_source_trait() {
        let source: Rc<SourceInner<i32>> = Rc::new(SourceInner::new(42));

        // Can coerce to Rc<dyn AnySource>
        let any_source: Rc<dyn AnySource> = source.clone();

        assert!(any_source.flags() & SOURCE != 0);
        assert!(any_source.is_clean());
        assert!(!any_source.is_dirty());
        assert!(!any_source.is_derived());
    }

    #[test]
    fn heterogeneous_source_storage() {
        // THE KEY TEST: Different T types in same Vec
        let int_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42i32));
        let str_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(String::from("hello")));
        let bool_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(true));

        let sources: Vec<Rc<dyn AnySource>> = vec![int_source, str_source, bool_source];

        assert_eq!(sources.len(), 3);

        // All have SOURCE flag
        for source in &sources {
            assert!(source.flags() & SOURCE != 0);
        }

        // Can mark dirty
        sources[0].mark_dirty();
        assert!(sources[0].is_dirty());
        assert!(!sources[0].is_clean());

        // Others still clean
        assert!(sources[1].is_clean());
        assert!(sources[2].is_clean());
    }

    #[test]
    fn source_flag_operations() {
        let source = SourceInner::new(42);

        // Start clean
        assert!(source.is_clean());
        assert!(!source.is_dirty());
        assert!(!source.is_maybe_dirty());

        // Mark dirty
        source.mark_dirty();
        assert!(!source.is_clean());
        assert!(source.is_dirty());
        assert!(!source.is_maybe_dirty());

        // Mark maybe dirty
        source.mark_maybe_dirty();
        assert!(!source.is_clean());
        assert!(!source.is_dirty());
        assert!(source.is_maybe_dirty());

        // Mark clean
        source.mark_clean();
        assert!(source.is_clean());
        assert!(!source.is_dirty());
        assert!(!source.is_maybe_dirty());
    }

    #[test]
    fn custom_equality_function() {
        // Always consider values equal (never triggers updates)
        fn never_equal<T>(_: &T, _: &T) -> bool {
            false
        }

        let source = SourceInner::new_with_equals(42, never_equal);

        // Even setting same value should "change" with never_equal
        let changed = source.set(42);
        assert!(changed);
    }

    #[test]
    fn downcast_from_any_source() {
        let source: Rc<SourceInner<i32>> = Rc::new(SourceInner::new(42));
        let any_source: Rc<dyn AnySource> = source.clone();

        // Can downcast back to concrete type
        let inner = any_source.as_any().downcast_ref::<SourceInner<i32>>().unwrap();
        assert_eq!(inner.get(), 42);
    }
}
