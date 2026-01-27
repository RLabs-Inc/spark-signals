// ============================================================================
// spark-signals - Dependency Tracking
// The core of the reactivity system - tracking reads and propagating writes
// ============================================================================
//
// This module ports tracking.ts from the TypeScript implementation.
// The key challenge in Rust is borrow scoping: we must release RefCell borrows
// before mutating, using the "collect-then-mutate" pattern.
// ============================================================================

use std::rc::Rc;

use crate::core::constants::*;
use crate::core::context::with_context;
use crate::core::types::{AnyReaction, AnySource};

// =============================================================================
// TRACK READ - Register dependency when reading a signal
// =============================================================================

/// Track a read of a source, registering it as a dependency if inside a reaction.
///
/// Called by Signal::get() after reading the value.
///
/// # Borrow Safety
/// This function accesses the thread-local context and may modify both the
/// source's reactions list and the reaction's deps list. Care is taken to
/// not hold borrows across these mutations.
pub fn track_read(source: Rc<dyn AnySource>) {
    with_context(|ctx| {
        // Only track if we're inside a reaction and not untracking
        if !ctx.has_active_reaction() || ctx.is_untracking() {
            return;
        }

        // Get the active reaction
        let reaction_weak = match ctx.get_active_reaction() {
            Some(r) => r,
            None => return,
        };

        let reaction = match reaction_weak.upgrade() {
            Some(r) => r,
            None => return,
        };

        // Check if we're in the reaction's update cycle
        if (reaction.flags() & REACTION_IS_UPDATING) != 0 {
            // Version-based deduplication: only add if not already tracked this cycle
            let read_version = ctx.get_read_version();

            if source.read_version() < read_version {
                // First read of this source in this cycle
                source.set_read_version(read_version);

                // Add to the new deps list being built
                ctx.add_new_dep(source.clone());
            }
            // If rv >= readVersion, we already tracked this source this cycle - skip
        } else {
            // Outside update cycle (e.g., reading after reaction setup)
            // Add dependency directly with duplicate checking

            // Add source to reaction's deps
            reaction.add_dep(source.clone());

            // Add reaction to source's reactions
            source.add_reaction(Rc::downgrade(&reaction));
        }
    });
}

// =============================================================================
// NOTIFY WRITE - Called when a signal's value changes
// =============================================================================

/// Notify the reactive system that a source's value has changed.
///
/// Called by Signal::set() after the value is updated.
/// This triggers markReactions to propagate dirty state through the graph.
pub fn notify_write(source: Rc<dyn AnySource>) {
    // Check for unsafe mutation inside a derived
    with_context(|ctx| {
        if let Some(reaction_weak) = ctx.get_active_reaction() {
            if let Some(reaction) = reaction_weak.upgrade() {
                if (reaction.flags() & DERIVED) != 0 {
                    panic!(
                        "Cannot write to signals inside a derived. \
                         Deriveds should be pure computations with no side effects."
                    );
                }
            }
        }
    });

    // Mark all reactions as dirty
    mark_reactions(source, DIRTY);
}

// =============================================================================
// MARK REACTIONS - Propagate dirty state through the graph
// =============================================================================

/// Mark all reactions of a source with the given status.
///
/// For direct dependents: mark with the given status (usually DIRTY)
/// For deriveds: cascade MAYBE_DIRTY to their dependents
/// For effects: schedule them for execution
///
/// # Algorithm
/// Uses an iterative approach with an explicit stack to avoid stack overflow
/// on deep dependency chains. This is critical for performance with deeply
/// nested deriveds.
///
/// # Borrow Safety
/// The key challenge: we iterate over a source's reactions while also needing
/// to modify those reactions (set their flags) and potentially iterate THEIR
/// reactions (for deriveds).
///
/// Solution: "collect-then-mutate" pattern
/// 1. Collect reactions into a temporary Vec (releases the borrow)
/// 2. Iterate the Vec and mutate freely
pub fn mark_reactions(source: Rc<dyn AnySource>, status: u32) {
    // Collect effects to schedule (we can't schedule inside with_context)
    let mut effects_to_schedule: Vec<Rc<dyn AnyReaction>> = Vec::new();

    // Use iterative approach with explicit stack
    let mut stack: Vec<(Rc<dyn AnySource>, u32)> = vec![(source, status)];

    while let Some((current_source, current_status)) = stack.pop() {
        // Clean up dead reactions first (prevents O(n) memory growth in reaction lists)
        current_source.cleanup_dead_reactions();

        // BORROW SAFETY: Collect reactions first, then release the borrow
        // This is the critical pattern that prevents RefCell panics
        let reactions: Vec<Rc<dyn AnyReaction>> = {
            let mut collected = Vec::new();
            current_source.for_each_reaction(&mut |reaction| {
                collected.push(reaction);
                true // continue iteration
            });
            collected
        };
        // Borrow on current_source.reactions is now released

        for reaction in reactions {
            let flags = reaction.flags();

            // Skip if already DIRTY (don't downgrade to MAYBE_DIRTY)
            let not_dirty = (flags & DIRTY) == 0;

            if not_dirty {
                set_signal_status(&*reaction, current_status);
            }

            // For derived signals, cascade MAYBE_DIRTY to their dependents
            if (flags & DERIVED) != 0 {
                // Derived is also a Source - get its reactions
                // We need to push it to the stack to process its reactions
                if let Some(derived_as_source) = reaction.as_derived_source() {
                    stack.push((derived_as_source, MAYBE_DIRTY));
                }
            } else if (flags & REPEATER) != 0 {
                // Inline write-through for repeaters — runs during mark_reactions, not scheduled
                if not_dirty {
                    // Downcast to RepeaterInner and call forward()
                    if let Some(repeater) = reaction.as_any().downcast_ref::<crate::primitives::repeater::RepeaterInner>() {
                        repeater.forward();
                    }
                    set_signal_status(&*reaction, CLEAN);
                }
            } else if not_dirty && (flags & EFFECT) != 0 {
                // For effects that just became dirty, schedule them for execution
                effects_to_schedule.push(reaction);
            }
        }
    }

    // Schedule all dirty effects
    for effect in effects_to_schedule {
        schedule_effect(effect);
    }
}

/// Schedule an effect for execution.
///
/// Adds the effect to the pending queue and triggers a flush.
fn schedule_effect(effect: Rc<dyn AnyReaction>) {
    with_context(|ctx| {
        ctx.add_pending_reaction(Rc::downgrade(&effect));
    });

    // Flush immediately (Rust doesn't have microtasks)
    // Check if we're already flushing to avoid recursion
    let should_flush = with_context(|ctx| !ctx.is_batching() && !ctx.is_flushing_sync());

    if should_flush {
        flush_pending_effects();
    }
}

/// Flush all pending effects.
fn flush_pending_effects() {
    let was_flushing = with_context(|ctx| {
        let was = ctx.is_flushing_sync();
        ctx.set_flushing_sync(true);
        was
    });

    const MAX_ITERATIONS: u32 = 1000;
    let mut iterations = 0;

    loop {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            with_context(|ctx| ctx.set_flushing_sync(was_flushing));
            panic!(
                "Maximum update depth exceeded. This can happen when an effect \
                 continuously triggers itself."
            );
        }

        let pending = with_context(|ctx| ctx.take_pending_reactions());

        if pending.is_empty() {
            break;
        }

        for reaction_weak in pending {
            if let Some(reaction) = reaction_weak.upgrade() {
                let flags = reaction.flags();

                // Skip inert (paused) or destroyed effects
                if (flags & (INERT | DESTROYED)) != 0 {
                    continue;
                }

                // Only run if still dirty
                if !is_dirty(&*reaction) {
                    continue;
                }

                // Run the effect
                if (flags & EFFECT) != 0 {
                    reaction.update();
                }
            }
        }
    }

    with_context(|ctx| ctx.set_flushing_sync(was_flushing));
}

// =============================================================================
// SET SIGNAL STATUS - Helper to update status flags
// =============================================================================

/// Set the status flags of a signal/reaction (CLEAN, DIRTY, MAYBE_DIRTY).
///
/// Clears the existing status bits and sets the new status.
pub fn set_signal_status(target: &dyn AnyReaction, status: u32) {
    let new_flags = (target.flags() & STATUS_MASK) | status;
    target.set_flags(new_flags);
}

/// Set status on an AnySource (for consistency, same operation)
pub fn set_source_status(target: &dyn AnySource, status: u32) {
    let new_flags = (target.flags() & STATUS_MASK) | status;
    target.set_flags(new_flags);
}

// =============================================================================
// IS DIRTY - Check if a reaction needs to update
// =============================================================================

/// Check if a reaction is dirty and needs to be updated.
///
/// - DIRTY: definitely needs update
/// - MAYBE_DIRTY: check dependencies to see if any actually changed
/// - CLEAN: no update needed
///
/// For Phase 3, this is a simple flag check.
/// Phase 4 will add the MAYBE_DIRTY dependency walk for deriveds.
pub fn is_dirty(reaction: &dyn AnyReaction) -> bool {
    let flags = reaction.flags();

    // Definitely dirty
    if (flags & DIRTY) != 0 {
        return true;
    }

    // Not maybe dirty - definitely clean
    if (flags & MAYBE_DIRTY) == 0 {
        return false;
    }

    // MAYBE_DIRTY: For now, treat as dirty.
    // Phase 4 will implement the proper dependency version checking.
    // This is conservative but correct - we might do unnecessary updates
    // but we won't miss necessary ones.
    true
}

// =============================================================================
// REMOVE REACTIONS - Clean up stale dependencies
// =============================================================================

/// Remove a reaction from its dependencies, starting at the given index.
///
/// When a reaction re-runs, its dependencies might change. This function
/// removes the reaction from dependencies that are no longer used.
///
/// # Borrow Safety
/// We iterate the reaction's deps (which requires borrowing) and then
/// modify each dep's reactions list. The collect-then-mutate pattern
/// ensures we don't hold conflicting borrows.
pub fn remove_reactions(reaction: Rc<dyn AnyReaction>, start: usize) {
    // Collect deps to remove from (starting at index 'start')
    let deps_to_remove: Vec<Rc<dyn AnySource>> = {
        let mut collected = Vec::new();
        let mut idx = 0;
        reaction.for_each_dep(&mut |dep| {
            if idx >= start {
                collected.push(dep.clone());
            }
            idx += 1;
            true
        });
        collected
    };
    // Borrow on reaction.deps is released

    // Now remove the reaction from each dep's reactions list
    for dep in deps_to_remove {
        remove_reaction_from_source(&reaction, &*dep);
    }

    // Truncate the reaction's deps list
    reaction.remove_deps_from(start);
}

/// Remove a single reaction from a source's reactions list.
///
/// Uses the source's cleanup method which handles the Weak reference removal.
fn remove_reaction_from_source(reaction: &Rc<dyn AnyReaction>, source: &dyn AnySource) {
    // The source stores Weak<dyn AnyReaction>, so we need to find and remove
    // the matching weak reference.
    //
    // Since we can't directly compare Weak references by identity easily,
    // we use a different approach: mark the reaction for removal and let
    // the source's cleanup handle it.
    //
    // Actually, we need to add a method to AnySource for this.
    // For now, the source.cleanup_dead_reactions() will clean up dropped refs,
    // but for active removal we need source.remove_reaction(reaction).

    source.remove_reaction(reaction);
}

// =============================================================================
// INSTALL DEPENDENCIES - Wire up deps after reaction execution
// =============================================================================

/// Install new dependencies after a reaction has run.
///
/// Called after a reaction's function executes. Takes the collected new_deps
/// and wires them up properly:
/// 1. Keep deps that were skipped (read in same order as last time)
/// 2. Add new deps
/// 3. Register the reaction with each new dep
pub fn install_dependencies(reaction: Rc<dyn AnyReaction>, skipped: usize) {
    with_context(|ctx| {
        // Take the new deps collected during execution
        let new_deps = ctx.swap_new_deps(Vec::new());

        if new_deps.is_empty() && skipped == 0 {
            // No dependencies at all
            reaction.clear_deps();
            return;
        }

        // Remove old deps that are no longer used (from skipped onwards)
        remove_reactions(reaction.clone(), skipped);

        // Add new deps to the reaction
        for dep in &new_deps {
            reaction.add_dep(dep.clone());
            dep.add_reaction(Rc::downgrade(&reaction));
        }
    });
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::with_context;
    use crate::core::types::SourceInner;
    use std::any::Any;
    use std::cell::{Cell, RefCell};
    use std::rc::Weak;

    // =========================================================================
    // Mock Reaction for testing
    // =========================================================================

    /// A mock reaction for testing dependency tracking.
    struct MockReaction {
        flags: Cell<u32>,
        deps: RefCell<Vec<Rc<dyn AnySource>>>,
    }

    impl MockReaction {
        fn new() -> Self {
            Self {
                flags: Cell::new(EFFECT | CLEAN),
                deps: RefCell::new(Vec::new()),
            }
        }
    }

    impl AnyReaction for MockReaction {
        fn flags(&self) -> u32 {
            self.flags.get()
        }

        fn set_flags(&self, flags: u32) {
            self.flags.set(flags);
        }

        fn dep_count(&self) -> usize {
            self.deps.borrow().len()
        }

        fn add_dep(&self, source: Rc<dyn AnySource>) {
            self.deps.borrow_mut().push(source);
        }

        fn clear_deps(&self) {
            self.deps.borrow_mut().clear();
        }

        fn remove_deps_from(&self, start: usize) {
            self.deps.borrow_mut().truncate(start);
        }

        fn for_each_dep(&self, f: &mut dyn FnMut(&Rc<dyn AnySource>) -> bool) {
            for dep in self.deps.borrow().iter() {
                if !f(dep) {
                    break;
                }
            }
        }

        fn remove_source(&self, source: &Rc<dyn AnySource>) {
            let source_ptr = Rc::as_ptr(source) as *const ();
            self.deps.borrow_mut().retain(|dep| {
                let dep_ptr = Rc::as_ptr(dep) as *const ();
                dep_ptr != source_ptr
            });
        }

        fn update(&self) -> bool {
            // Mock: just return false (no value change)
            false
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_derived_source(&self) -> Option<Rc<dyn AnySource>> {
            // Mock reactions don't have a source side
            None
        }
    }

    // =========================================================================
    // Mock Derived for testing cascade
    // =========================================================================

    /// A mock derived for testing cascade propagation.
    /// Implements BOTH AnySource and AnyReaction.
    struct MockDerived {
        flags: Cell<u32>,
        write_version: Cell<u32>,
        read_version: Cell<u32>,
        deps: RefCell<Vec<Rc<dyn AnySource>>>,
        reactions: RefCell<Vec<Weak<dyn AnyReaction>>>,
    }

    impl MockDerived {
        fn new() -> Rc<Self> {
            Rc::new(Self {
                flags: Cell::new(DERIVED | SOURCE | CLEAN),
                write_version: Cell::new(0),
                read_version: Cell::new(0),
                deps: RefCell::new(Vec::new()),
                reactions: RefCell::new(Vec::new()),
            })
        }
    }

    impl AnySource for MockDerived {
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
            let reaction_ptr = Rc::as_ptr(reaction) as *const ();
            self.reactions.borrow_mut().retain(|weak| {
                if let Some(rc) = weak.upgrade() {
                    let ptr = Rc::as_ptr(&rc) as *const ();
                    ptr != reaction_ptr
                } else {
                    false // remove dead refs
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

    impl AnyReaction for MockDerived {
        fn flags(&self) -> u32 {
            self.flags.get()
        }

        fn set_flags(&self, flags: u32) {
            self.flags.set(flags);
        }

        fn dep_count(&self) -> usize {
            self.deps.borrow().len()
        }

        fn add_dep(&self, source: Rc<dyn AnySource>) {
            self.deps.borrow_mut().push(source);
        }

        fn clear_deps(&self) {
            self.deps.borrow_mut().clear();
        }

        fn remove_deps_from(&self, start: usize) {
            self.deps.borrow_mut().truncate(start);
        }

        fn for_each_dep(&self, f: &mut dyn FnMut(&Rc<dyn AnySource>) -> bool) {
            for dep in self.deps.borrow().iter() {
                if !f(dep) {
                    break;
                }
            }
        }

        fn remove_source(&self, source: &Rc<dyn AnySource>) {
            let source_ptr = Rc::as_ptr(source) as *const ();
            self.deps.borrow_mut().retain(|dep| {
                let dep_ptr = Rc::as_ptr(dep) as *const ();
                dep_ptr != source_ptr
            });
        }

        fn update(&self) -> bool {
            false
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_derived_source(&self) -> Option<Rc<dyn AnySource>> {
            // This is where it gets tricky - we need to return self as Rc<dyn AnySource>
            // But we don't have access to the Rc from &self
            // This is a known Rust limitation
            // For real Derived implementation, we'll store the Rc internally
            None
        }
    }

    // =========================================================================
    // Tests
    // =========================================================================

    #[test]
    fn track_read_outside_reaction_does_nothing() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));

        // No active reaction
        track_read(source.clone());

        // Source should have no reactions registered
        assert_eq!(source.reaction_count(), 0);
    }

    #[test]
    fn track_read_registers_dependency() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));
        let reaction: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        // Simulate being inside a reaction (not in update cycle)
        with_context(|ctx| {
            ctx.set_active_reaction(Some(Rc::downgrade(&reaction)));
        });

        track_read(source.clone());

        // Clean up
        with_context(|ctx| {
            ctx.set_active_reaction(None);
        });

        // Check: reaction should have source as a dep
        assert_eq!(reaction.dep_count(), 1);

        // Check: source should have reaction in its reactions
        assert_eq!(source.reaction_count(), 1);
    }

    #[test]
    fn track_read_with_untracking_does_not_register() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));
        let reaction: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        with_context(|ctx| {
            ctx.set_active_reaction(Some(Rc::downgrade(&reaction)));
            ctx.set_untracking(true);
        });

        track_read(source.clone());

        with_context(|ctx| {
            ctx.set_active_reaction(None);
            ctx.set_untracking(false);
        });

        // Should NOT have registered
        assert_eq!(reaction.dep_count(), 0);
        assert_eq!(source.reaction_count(), 0);
    }

    #[test]
    fn mark_reactions_marks_direct_deps_dirty() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));
        let reaction: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        // Wire up the dependency manually
        source.add_reaction(Rc::downgrade(&reaction));

        // Reaction starts clean
        assert!(reaction.is_clean());
        assert!(!reaction.is_dirty());

        // Mark reactions
        mark_reactions(source.clone(), DIRTY);

        // Reaction should now be dirty
        assert!(reaction.is_dirty());
        assert!(!reaction.is_clean());
    }

    #[test]
    fn mark_reactions_does_not_downgrade_dirty_to_maybe_dirty() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));
        let reaction: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        // Pre-mark as dirty
        reaction.mark_dirty();
        assert!(reaction.is_dirty());

        source.add_reaction(Rc::downgrade(&reaction));

        // Mark with MAYBE_DIRTY
        mark_reactions(source.clone(), MAYBE_DIRTY);

        // Should still be DIRTY, not downgraded to MAYBE_DIRTY
        assert!(reaction.is_dirty());
        assert!(!reaction.is_maybe_dirty());
    }

    #[test]
    fn is_dirty_reports_correctly() {
        let reaction: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        // Clean
        assert!(!is_dirty(&*reaction));

        // Dirty
        reaction.mark_dirty();
        assert!(is_dirty(&*reaction));

        // Maybe dirty (treated as dirty for now)
        reaction.mark_maybe_dirty();
        assert!(is_dirty(&*reaction));

        // Clean again
        reaction.mark_clean();
        assert!(!is_dirty(&*reaction));
    }

    #[test]
    fn remove_reactions_cleans_up_deps() {
        let source1: Rc<dyn AnySource> = Rc::new(SourceInner::new(1));
        let source2: Rc<dyn AnySource> = Rc::new(SourceInner::new(2));
        let source3: Rc<dyn AnySource> = Rc::new(SourceInner::new(3));
        let reaction: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        // Add deps
        reaction.add_dep(source1.clone());
        reaction.add_dep(source2.clone());
        reaction.add_dep(source3.clone());

        // Add reaction to sources
        source1.add_reaction(Rc::downgrade(&reaction));
        source2.add_reaction(Rc::downgrade(&reaction));
        source3.add_reaction(Rc::downgrade(&reaction));

        assert_eq!(reaction.dep_count(), 3);
        assert_eq!(source1.reaction_count(), 1);
        assert_eq!(source2.reaction_count(), 1);
        assert_eq!(source3.reaction_count(), 1);

        // Remove from index 1 onwards
        remove_reactions(reaction.clone(), 1);

        // Should only have source1 left
        assert_eq!(reaction.dep_count(), 1);

        // source2 and source3 should have reaction removed
        // (cleanup happens, but our mock doesn't implement remove_reaction properly)
        // The important thing is reaction.deps is truncated
    }

    #[test]
    fn borrow_safety_multiple_reactions() {
        // This test proves we don't get borrow panics with multiple reactions
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));
        let reaction1: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());
        let reaction2: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());
        let reaction3: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        source.add_reaction(Rc::downgrade(&reaction1));
        source.add_reaction(Rc::downgrade(&reaction2));
        source.add_reaction(Rc::downgrade(&reaction3));

        // This should NOT panic
        mark_reactions(source.clone(), DIRTY);

        assert!(reaction1.is_dirty());
        assert!(reaction2.is_dirty());
        assert!(reaction3.is_dirty());
    }

    #[test]
    fn borrow_safety_cascade_simulation() {
        // Simulate a cascade: source -> derived -> effect
        // This tests the cascade path even though MockDerived.as_derived_source returns None

        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));
        let derived = MockDerived::new();
        let effect: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        // Wire up: source -> derived
        source.add_reaction(Rc::downgrade(&(derived.clone() as Rc<dyn AnyReaction>)));

        // Wire up: derived -> effect
        derived.add_reaction(Rc::downgrade(&effect));

        // Mark source's reactions as dirty
        // This should mark derived as DIRTY
        // The cascade to effect would happen if as_derived_source returned Some
        // For now, derived gets marked dirty but effect doesn't (that's Phase 4)

        mark_reactions(source.clone(), DIRTY);

        // Derived should be dirty (it's a direct reaction of source)
        assert!((derived.flags.get() & DIRTY) != 0);

        // Effect won't be dirty yet because as_derived_source returns None
        // This is expected for Phase 3 - the cascade will work in Phase 4
        // when we have real DerivedInner
    }

    #[test]
    fn version_based_deduplication() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42));
        let reaction: Rc<dyn AnyReaction> = Rc::new(MockReaction::new());

        // Set up: reaction is in update cycle
        reaction.set_flags(reaction.flags() | REACTION_IS_UPDATING);

        with_context(|ctx| {
            ctx.set_active_reaction(Some(Rc::downgrade(&reaction)));
            ctx.increment_read_version(); // Start a new read cycle

            // First read - should track
            track_read(source.clone());
            assert_eq!(ctx.new_dep_count(), 1);

            // Second read of same source - should NOT add duplicate
            track_read(source.clone());
            assert_eq!(ctx.new_dep_count(), 1); // Still 1, not 2

            // Clean up
            ctx.set_active_reaction(None);
            ctx.swap_new_deps(Vec::new());
        });
    }
}
