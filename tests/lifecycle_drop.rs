use spark_signals::{
    create_selector_eq, effect_scope, effect_sync, effect_sync_with_cleanup,
    linked_signal, signal, Effect, EffectScope
};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_effect_drop_runs_teardown() {
    let cleanup_called = Rc::new(Cell::new(false));
    let cleanup_clone = cleanup_called.clone();

    // Create an effect with NO dependencies and NO scope
    // This ensures that when we drop the dispose function, the EffectInner is dropped
    {
        let _dispose = effect_sync_with_cleanup(move || {
            let cc = cleanup_clone.clone();
            Some(Box::new(move || cc.set(true)))
        });
        // _dispose is dropped here. 
        // Since the effect has no deps and no scope, the Rc count goes to 0.
        // EffectInner::drop should run.
    }

    assert!(cleanup_called.get(), "Effect drop should run cleanup (fallback)");
}

#[test]
fn test_selector_drop_disposes_internal_effect() {
    let run_count = Rc::new(Cell::new(0));
    let run_count_clone = run_count.clone();
    
    let source = signal(0);
    let source_clone = source.clone();
    
    {
        let _selector = create_selector_eq(move || {
            run_count_clone.set(run_count_clone.get() + 1);
            source_clone.get()
        });
        
        assert_eq!(run_count.get(), 1);
        
        source.set(1);
        assert_eq!(run_count.get(), 2);
        
        // _selector drops here
    }
    
    // Should NOT run after drop
    source.set(2);
    assert_eq!(run_count.get(), 2, "Selector internal effect should stop after drop");
}

#[test]
fn test_linked_signal_drop_disposes_internal_effect() {
    let run_count = Rc::new(Cell::new(0));
    let run_count_clone = run_count.clone();
    
    let source = signal(0);
    let source_clone = source.clone();
    
    {
        let _linked = linked_signal(move || {
            run_count_clone.set(run_count_clone.get() + 1);
            source_clone.get()
        });
        
        // Known behavior: linked_signal computes twice on initialization
        // 1. To get initial value for value_signal
        // 2. To compute source_tracker derived
        assert_eq!(run_count.get(), 2);
        
        source.set(1);
        assert_eq!(run_count.get(), 3);
        
        // _linked drops here
    }
    
    // Should NOT run after drop
    source.set(2);
    assert_eq!(run_count.get(), 3, "LinkedSignal internal effect should stop after drop");
}

#[test]
fn test_scope_drop_runs_cleanup() {
    let cleanup_called = Rc::new(Cell::new(false));
    let cleanup_clone = cleanup_called.clone();

    {
        let scope = effect_scope(false);
        scope.run(|| {
            spark_signals::on_scope_dispose(move || {
                cleanup_clone.set(true);
            });
        });
        // Scope drops here
    }

    assert!(cleanup_called.get(), "Scope drop should run cleanups");
}

#[test]
fn test_scope_drop_stops_effects() {
    let run_count = Rc::new(Cell::new(0));
    let run_count_clone = run_count.clone();
    let count = signal(0);
    let count_clone = count.clone();

    {
        let scope = effect_scope(false);
        scope.run(|| {
            effect_sync(move || {
                let _ = count_clone.get();
                run_count_clone.set(run_count_clone.get() + 1);
            });
        });
        
        assert_eq!(run_count.get(), 1);
        count.set(1);
        assert_eq!(run_count.get(), 2);
        
        // Scope drops here
    }

    count.set(2);
    assert_eq!(run_count.get(), 2, "Effect should not run after scope drop");
}

#[test]
fn test_scope_clone_does_not_stop() {
    let run_count = Rc::new(Cell::new(0));
    let run_count_clone = run_count.clone();
    let count = signal(0);
    
    let scope1 = effect_scope(false);
    
    {
        let scope2 = scope1.clone();
        scope2.run(|| {
            let count = count.clone();
            let run_count = run_count_clone.clone();
            effect_sync(move || {
                let _ = count.get();
                run_count.set(run_count.get() + 1);
            });
        });
        // scope2 drops here
    }

    // Should still be active because scope1 exists
    count.set(1);
    assert_eq!(run_count.get(), 2, "Effect should run after clone drop");
    
    // Drop scope1
    drop(scope1);
    
    count.set(2);
    assert_eq!(run_count.get(), 2, "Effect should not run after last drop");
}

// For Selector deduplication, we need internal access or observe behavior.
// We can observe behavior: if we subscribe same reaction multiple times, 
// does it run multiple times?
// Actually, spark-signals runs effects once per update regardless of how many times they subscribed 
// (due to id/version checks).
// But the subscriber LIST grows.
// We can't easily check memory/list size from outside without internal API.
// However, we can check if weak ptr_eq works by creating a custom test in `src/primitives/selector.rs`
// I already updated `src/primitives/selector.rs`, let's trust the unit tests there or add one there.
