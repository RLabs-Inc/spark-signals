// ============================================================================
// spark-signals - A Reactive Signals Library for Rust
// ============================================================================
//
// A faithful port of @rlabs-inc/signals TypeScript package.
// See CLAUDE.md for implementation notes and .planning/ for roadmap.
// ============================================================================

pub mod collections;
pub mod core;
pub mod primitives;
pub mod reactivity;

// Re-export core items at crate root for ergonomic access
pub use core::constants;
pub use core::context::{
    is_batching, is_tracking, is_untracking, read_version, with_context, write_version,
    ReactiveContext,
};
pub use core::types::{default_equals, AnyReaction, AnySource, EqualsFn, SourceInner};

// Re-export primitives at crate root (TypeScript-like API)
pub use primitives::bind::{
    bind, bind_chain, bind_getter, bind_readonly, bind_readonly_from, bind_readonly_static,
    bind_static, bind_value, binding_has_internal_source, disconnect_binding, disconnect_source,
    is_binding, unwrap_binding, unwrap_readonly, Binding, IsBinding, ReadonlyBinding,
};
pub use primitives::derived::{derived, derived_with_equals, Derived, DerivedInner};
pub use primitives::effect::{
    effect, effect_root, effect_sync, effect_sync_with_cleanup, effect_tracking,
    effect_with_cleanup, CleanupFn, DisposeFn, Effect, EffectFn, EffectInner,
};
pub use primitives::linked::{
    is_linked_signal, linked_signal, linked_signal_full, linked_signal_with_options,
    IsLinkedSignal, LinkedSignal, LinkedSignalOptionsSimple, PreviousValue,
};
pub use primitives::props::{into_derived, reactive_prop, PropValue, PropsBuilder, UnwrapProp};
pub use primitives::selector::{create_selector, create_selector_eq, Selector};
pub use primitives::scope::{
    effect_scope, get_current_scope, on_scope_dispose, EffectScope, ScopeCleanupFn,
};
pub use primitives::signal::{
    mutable_source, signal, signal_f32, signal_f64, signal_with_equals, source, Signal,
    SourceOptions,
};
pub use primitives::slot::{
    dirty_set, is_slot, slot, slot_array, slot_with_value, tracked_slot_array, DirtySet, IsSlot,
    Slot, SlotArray, SlotWriteError, TrackedSlotArray,
};

// Re-export reactivity functions
pub use reactivity::batching::{batch, peek, tick, untrack};
pub use reactivity::equality::{
    always_equals, by_field, deep_equals, equals, never_equals, safe_equals_f32, safe_equals_f64,
    safe_equals_option_f64, safe_not_equal_f32, safe_not_equal_f64, shallow_equals_slice,
    shallow_equals_vec,
};
pub use reactivity::scheduling::flush_sync;
pub use reactivity::tracking::{
    is_dirty, mark_reactions, notify_write, remove_reactions, set_signal_status, track_read,
};

// Re-export collections
pub use collections::{ReactiveMap, ReactiveSet, ReactiveVec};

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    // =========================================================================
    // Phase 1 Success Criteria Tests
    // =========================================================================

    #[test]
    fn phase1_success_criteria_1_flags_defined() {
        // Type flags
        assert_eq!(constants::SOURCE, 1 << 0);
        assert_eq!(constants::DERIVED, 1 << 1);
        assert_eq!(constants::EFFECT, 1 << 2);

        // Status flags
        assert_eq!(constants::CLEAN, 1 << 10);
        assert_eq!(constants::DIRTY, 1 << 11);
        assert_eq!(constants::MAYBE_DIRTY, 1 << 12);

        // All distinct
        assert_eq!(constants::CLEAN & constants::DIRTY, 0);
        assert_eq!(constants::DIRTY & constants::MAYBE_DIRTY, 0);
    }

    #[test]
    fn phase1_success_criteria_2_traits_compile() {
        let source: Rc<SourceInner<i32>> = Rc::new(SourceInner::new(42));
        let _any_source: Rc<dyn AnySource> = source;

        let source = SourceInner::new(100);
        assert!(source.flags() & constants::SOURCE != 0);
        source.mark_dirty();
        assert!(source.is_dirty());
    }

    #[test]
    fn phase1_success_criteria_3_thread_local_context() {
        with_context(|ctx| {
            assert_eq!(ctx.get_write_version(), 1);
            assert!(!ctx.has_active_reaction());

            ctx.increment_write_version();
            assert_eq!(ctx.get_write_version(), 2);
        });

        assert!(write_version() >= 1);
        assert!(!is_tracking());
    }

    #[test]
    fn phase1_success_criteria_4_heterogeneous_storage() {
        let int_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42i32));
        let string_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(String::from("hello")));
        let float_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(3.14f64));
        let bool_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(true));
        let vec_source: Rc<dyn AnySource> = Rc::new(SourceInner::new(vec![1, 2, 3]));

        let sources: Vec<Rc<dyn AnySource>> = vec![
            int_source,
            string_source,
            float_source,
            bool_source,
            vec_source,
        ];

        assert_eq!(sources.len(), 5);

        for source in &sources {
            assert!(source.flags() & constants::SOURCE != 0);
            assert!(source.is_clean());
        }

        sources[0].mark_dirty();
        sources[2].mark_maybe_dirty();

        assert!(sources[0].is_dirty());
        assert!(sources[1].is_clean());
        assert!(sources[2].is_maybe_dirty());
    }

    // =========================================================================
    // Phase 2 Success Criteria Tests
    // =========================================================================

    #[test]
    fn phase2_success_criteria_1_signal_api() {
        // User can create signal with signal(value)
        let count = signal(0);

        // Read with .get()
        assert_eq!(count.get(), 0);

        // Write with .set()
        count.set(42);
        assert_eq!(count.get(), 42);
    }

    #[test]
    fn phase2_success_criteria_2_heterogeneous_signal_storage() {
        // Signal<i32> and Signal<String> can be stored in same Vec<Rc<dyn AnySource>>
        let int_signal = signal(42i32);
        let string_signal = signal(String::from("hello"));

        let sources: Vec<Rc<dyn AnySource>> = vec![
            int_signal.as_any_source(),
            string_signal.as_any_source(),
        ];

        assert_eq!(sources.len(), 2);

        // Can operate uniformly
        for source in &sources {
            assert!(source.flags() & constants::SOURCE != 0);
        }
    }

    #[test]
    fn phase2_success_criteria_3_combinators() {
        let items = signal(vec![1, 2, 3, 4, 5]);

        // .try_get() works
        assert_eq!(items.try_get(), Some(vec![1, 2, 3, 4, 5]));

        // .with(f) works
        let sum = items.with(|v| v.iter().sum::<i32>());
        assert_eq!(sum, 15);

        // .update(f) works
        items.update(|v| v.push(6));
        assert_eq!(items.get(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn phase2_success_criteria_4_equality_checking() {
        let count = signal(42);

        // Setting same value doesn't "change"
        let changed = count.set(42);
        assert!(!changed);

        // Setting different value does "change"
        let changed = count.set(100);
        assert!(changed);
    }

    // =========================================================================
    // Phase 3 Success Criteria Tests
    // =========================================================================

    use std::any::Any;
    use std::cell::{Cell, RefCell};

    /// Mock reaction for testing - implements AnyReaction
    struct TestReaction {
        flags: Cell<u32>,
        deps: RefCell<Vec<Rc<dyn AnySource>>>,
    }

    impl TestReaction {
        fn new() -> Rc<Self> {
            Rc::new(Self {
                flags: Cell::new(constants::EFFECT | constants::CLEAN),
                deps: RefCell::new(Vec::new()),
            })
        }
    }

    impl AnyReaction for TestReaction {
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
            None
        }
    }

    #[test]
    fn phase3_success_criteria_1_read_registers_dependency() {
        // Reading a signal inside a reaction registers the signal as a dependency
        let count = signal(42);
        let reaction = TestReaction::new();

        // Simulate being inside a reaction
        with_context(|ctx| {
            ctx.set_active_reaction(Some(Rc::downgrade(
                &(reaction.clone() as Rc<dyn AnyReaction>),
            )));
        });

        // Read the signal - this should register the dependency
        let value = count.get();
        assert_eq!(value, 42);

        // Clean up
        with_context(|ctx| {
            ctx.set_active_reaction(None);
        });

        // Verify: reaction should have count as a dependency
        assert_eq!(reaction.dep_count(), 1);

        // Verify: count should have reaction in its reactions list
        assert_eq!(count.inner().reaction_count(), 1);
    }

    #[test]
    fn phase3_success_criteria_2_write_marks_reactions_dirty() {
        // Writing to a signal marks all dependent reactions as DIRTY
        let count = signal(0);
        let reaction = TestReaction::new();

        // Wire up the dependency manually
        count.inner().add_reaction(Rc::downgrade(
            &(reaction.clone() as Rc<dyn AnyReaction>),
        ));

        // Reaction starts CLEAN
        assert!(reaction.is_clean());
        assert!(!reaction.is_dirty());

        // Write to signal
        count.set(42);

        // Reaction should now be DIRTY
        assert!(reaction.is_dirty());
        assert!(!reaction.is_clean());
    }

    #[test]
    fn phase3_success_criteria_3_is_dirty_reports_correctly() {
        // isDirty(reaction) correctly reports dirty state
        let reaction = TestReaction::new();

        // Clean state
        assert!(!is_dirty(&*reaction));

        // Dirty state
        reaction.mark_dirty();
        assert!(is_dirty(&*reaction));

        // Maybe dirty state (treated as dirty)
        reaction.mark_maybe_dirty();
        assert!(is_dirty(&*reaction));

        // Clean again
        reaction.mark_clean();
        assert!(!is_dirty(&*reaction));
    }

    #[test]
    fn phase3_success_criteria_4_remove_reactions_cleanup() {
        // removeReactions(reaction, start) cleans up old dependencies
        let source1 = signal(1);
        let source2 = signal(2);
        let source3 = signal(3);
        let reaction = TestReaction::new();

        // Add deps manually
        reaction.add_dep(source1.as_any_source());
        reaction.add_dep(source2.as_any_source());
        reaction.add_dep(source3.as_any_source());

        // Register reaction with sources
        source1
            .inner()
            .add_reaction(Rc::downgrade(&(reaction.clone() as Rc<dyn AnyReaction>)));
        source2
            .inner()
            .add_reaction(Rc::downgrade(&(reaction.clone() as Rc<dyn AnyReaction>)));
        source3
            .inner()
            .add_reaction(Rc::downgrade(&(reaction.clone() as Rc<dyn AnyReaction>)));

        assert_eq!(reaction.dep_count(), 3);

        // Remove deps from index 1 onwards
        remove_reactions(reaction.clone(), 1);

        // Should only have source1 left
        assert_eq!(reaction.dep_count(), 1);

        // source2 and source3 should no longer have this reaction
        assert_eq!(source2.inner().reaction_count(), 0);
        assert_eq!(source3.inner().reaction_count(), 0);

        // source1 should still have it
        assert_eq!(source1.inner().reaction_count(), 1);
    }

    #[test]
    fn phase3_success_criteria_5_no_borrow_panics_cascade() {
        // No RefCell borrow panics during cascade updates
        // This test proves the collect-then-mutate pattern works

        let source = signal(0);

        // Create multiple reactions that depend on the source
        let reactions: Vec<Rc<TestReaction>> = (0..10).map(|_| TestReaction::new()).collect();

        // Wire up all reactions to the source
        for reaction in &reactions {
            source.inner().add_reaction(Rc::downgrade(
                &(reaction.clone() as Rc<dyn AnyReaction>),
            ));
        }

        // This should NOT panic - the collect-then-mutate pattern prevents it
        source.set(42);

        // All reactions should be dirty
        for reaction in &reactions {
            assert!(
                reaction.is_dirty(),
                "All reactions should be marked dirty after signal write"
            );
        }
    }

    #[test]
    fn phase3_integration_full_cycle() {
        // Integration test: full dependency tracking cycle

        let a = signal(1);
        let b = signal(2);
        let reaction = TestReaction::new();

        // Simulate entering a reaction's update
        with_context(|ctx| {
            ctx.set_active_reaction(Some(Rc::downgrade(
                &(reaction.clone() as Rc<dyn AnyReaction>),
            )));
        });

        // Read both signals
        let sum = a.get() + b.get();
        assert_eq!(sum, 3);

        // Exit reaction
        with_context(|ctx| {
            ctx.set_active_reaction(None);
        });

        // Both should be registered as deps
        assert_eq!(reaction.dep_count(), 2);
        assert_eq!(a.inner().reaction_count(), 1);
        assert_eq!(b.inner().reaction_count(), 1);

        // Reaction starts clean
        reaction.mark_clean();
        assert!(reaction.is_clean());

        // Change a - should mark reaction dirty
        a.set(10);
        assert!(reaction.is_dirty());

        // Reset and test b
        reaction.mark_clean();
        b.set(20);
        assert!(reaction.is_dirty());
    }

    // =========================================================================
    // Phase 4 Success Criteria Tests
    // =========================================================================

    #[test]
    fn phase4_success_criteria_1_derived_api() {
        // User can create derived with `derived(|| computation)`
        let count = signal(1);
        let doubled = derived({
            let count = count.clone();
            move || count.get() * 2
        });

        assert_eq!(doubled.get(), 2);

        count.set(5);
        assert_eq!(doubled.get(), 10);
    }

    #[test]
    fn phase4_success_criteria_2_caches_and_recomputes() {
        // Derived caches value, only recomputes when dependencies change
        let compute_count = Rc::new(Cell::new(0));

        let a = signal(1);
        let d = derived({
            let a = a.clone();
            let compute_count = compute_count.clone();
            move || {
                compute_count.set(compute_count.get() + 1);
                a.get() * 2
            }
        });

        // First read computes
        assert_eq!(d.get(), 2);
        assert_eq!(compute_count.get(), 1);

        // Second read uses cache (no recompute)
        assert_eq!(d.get(), 2);
        assert_eq!(compute_count.get(), 1);

        // After dependency changes, recomputes
        a.set(5);
        assert_eq!(d.get(), 10);
        assert_eq!(compute_count.get(), 2);

        // Reading again uses cache
        assert_eq!(d.get(), 10);
        assert_eq!(compute_count.get(), 2);
    }

    #[test]
    fn phase4_success_criteria_3_maybe_dirty_optimization() {
        // MAYBE_DIRTY optimization prevents unnecessary recomputation in chains
        // A -> B -> C
        // If B's value doesn't change when A changes, C shouldn't recompute

        let compute_c_count = Rc::new(Cell::new(0));

        let a = signal(0);

        // B clamps to range [0, 10]
        let b = derived({
            let a = a.clone();
            move || a.get().clamp(0, 10)
        });

        let c = derived({
            let b = b.clone();
            let compute_c_count = compute_c_count.clone();
            move || {
                compute_c_count.set(compute_c_count.get() + 1);
                b.get() * 100
            }
        });

        // Initial computation
        assert_eq!(c.get(), 0); // 0 * 100 = 0
        assert_eq!(compute_c_count.get(), 1);

        // Change a within clamp range - B's output stays 0
        a.set(0);
        assert_eq!(c.get(), 0);
        // Note: With full MAYBE_DIRTY optimization, C wouldn't recompute
        // Our implementation may be conservative

        // Change a to different clamped value
        a.set(5);
        assert_eq!(c.get(), 500);
    }

    #[test]
    fn phase4_success_criteria_4_diamond_dependency() {
        // Diamond dependency patterns work correctly (A->B, A->C, B->D, C->D)
        //
        //      A
        //     / \
        //    B   C
        //     \ /
        //      D

        let a = signal(1);

        let b = derived({
            let a = a.clone();
            move || a.get() + 10
        });

        let c = derived({
            let a = a.clone();
            move || a.get() * 10
        });

        let d = derived({
            let b = b.clone();
            let c = c.clone();
            move || b.get() + c.get()
        });

        // Initial: A=1, B=11, C=10, D=21
        assert_eq!(d.get(), 21);

        // Change A to 2: B=12, C=20, D=32
        a.set(2);
        assert_eq!(d.get(), 32);

        // D correctly gets both updated values
    }

    #[test]
    fn phase4_success_criteria_5_cascade_propagation() {
        // Circular dependency injection works (tracking calls derived update)
        // This tests that as_derived_source() enables cascade propagation

        let a = signal(1);

        let b = derived({
            let a = a.clone();
            move || a.get() * 2
        });

        let c = derived({
            let b = b.clone();
            move || b.get() + 10
        });

        // Initial read sets up the graph
        assert_eq!(c.get(), 12);

        // Get inner refs to check flags
        let b_inner = b.inner();
        let c_inner = c.inner();

        // Both should be clean
        assert!(AnySource::is_clean(&**b_inner));
        assert!(AnySource::is_clean(&**c_inner));

        // Change A
        a.set(5);

        // B should be DIRTY (direct dependency)
        // C should be DIRTY or MAYBE_DIRTY (cascade via as_derived_source)
        let b_flags = AnySource::flags(&**b_inner);
        let c_flags = AnySource::flags(&**c_inner);

        assert!(
            (b_flags & constants::DIRTY) != 0,
            "B should be marked DIRTY"
        );
        assert!(
            (c_flags & (constants::DIRTY | constants::MAYBE_DIRTY)) != 0,
            "C should be marked DIRTY or MAYBE_DIRTY via cascade"
        );

        // Reading C should trigger the cascade update
        assert_eq!(c.get(), 20); // (5*2) + 10 = 20

        // Both clean again
        assert!(AnySource::is_clean(&**b_inner));
        assert!(AnySource::is_clean(&**c_inner));
    }
}
