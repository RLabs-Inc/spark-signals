// ============================================================================
// spark-signals - Effect Scope
//
// Group effects for batch disposal with pause/resume support.
// Based on Vue 3's effectScope pattern.
// ============================================================================
//
// An EffectScope groups effects so they can be disposed together.
// This is useful for component lifecycle - create a scope when mounting,
// dispose the scope when unmounting, all effects clean up automatically.
//
// Key features:
// - run(fn) - Execute function with this scope active
// - stop() - Dispose all effects and run cleanups
// - pause()/resume() - Temporarily disable effects
// - Nested scopes (child scopes auto-disposed with parent)
// - Detached scopes (opt out of parent collection)
// ============================================================================

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::core::constants::*;
use crate::core::types::AnyReaction;
use crate::primitives::effect::{destroy_effect, EffectInner};
use crate::reactivity::scheduling::{flush_sync, schedule_effect_inner};

// =============================================================================
// THREAD-LOCAL SCOPE STATE
// =============================================================================

thread_local! {
    /// Currently active scope (if any)
    static ACTIVE_SCOPE: RefCell<Option<Rc<EffectScopeInner>>> = const { RefCell::new(None) };
}

/// Get the currently active scope
fn get_active_scope() -> Option<Rc<EffectScopeInner>> {
    ACTIVE_SCOPE.with(|s| s.borrow().clone())
}

/// Set the active scope, returning the previous one
fn set_active_scope(scope: Option<Rc<EffectScopeInner>>) -> Option<Rc<EffectScopeInner>> {
    ACTIVE_SCOPE.with(|s| {
        let prev = s.borrow().clone();
        *s.borrow_mut() = scope;
        prev
    })
}

// =============================================================================
// CLEANUP TYPE
// =============================================================================

/// Cleanup function type for scope disposal
pub type ScopeCleanupFn = Box<dyn FnOnce()>;

// =============================================================================
// EFFECT SCOPE INNER
// =============================================================================

/// Internal scope implementation
pub struct EffectScopeInner {
    /// Whether the scope is still active (not stopped)
    active: Cell<bool>,

    /// Whether the scope is paused
    paused: Cell<bool>,

    /// Effects created within this scope
    effects: RefCell<Vec<Rc<EffectInner>>>,

    /// Cleanup functions to run on stop
    cleanups: RefCell<Vec<ScopeCleanupFn>>,

    /// Parent scope (for nested scopes)
    parent: RefCell<Option<Weak<EffectScopeInner>>>,

    /// Child scopes
    scopes: RefCell<Vec<Rc<EffectScopeInner>>>,

    /// Self-reference for returning from run()
    self_weak: RefCell<Weak<EffectScopeInner>>,
}

impl EffectScopeInner {
    /// Create a new scope
    fn new(detached: bool) -> Rc<Self> {
        let parent = if detached { None } else { get_active_scope() };

        let scope = Rc::new(Self {
            active: Cell::new(true),
            paused: Cell::new(false),
            effects: RefCell::new(Vec::new()),
            cleanups: RefCell::new(Vec::new()),
            parent: RefCell::new(parent.as_ref().map(Rc::downgrade)),
            scopes: RefCell::new(Vec::new()),
            self_weak: RefCell::new(Weak::new()),
        });

        // Store self-reference
        *scope.self_weak.borrow_mut() = Rc::downgrade(&scope);

        // Register with parent scope unless detached
        if let Some(ref parent_scope) = parent {
            parent_scope.scopes.borrow_mut().push(scope.clone());
        }

        scope
    }

    /// Check if scope is active
    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    /// Check if scope is paused
    pub fn is_paused(&self) -> bool {
        self.paused.get()
    }

    /// Run a function within this scope
    pub fn run<R, F: FnOnce() -> R>(&self, f: F) -> Option<R> {
        if !self.active.get() {
            return None;
        }

        // Get Rc to self
        let self_rc = self.self_weak.borrow().upgrade()?;

        let prev_scope = set_active_scope(Some(self_rc));
        let result = f();
        set_active_scope(prev_scope);

        Some(result)
    }

    /// Stop the scope, disposing all tracked effects
    pub fn stop(&self) {
        if !self.active.get() {
            return;
        }

        // Flush any pending effects first to ensure clean state
        flush_sync();

        // Dispose all effects
        let effects: Vec<_> = self.effects.borrow_mut().drain(..).collect();
        for effect in effects {
            destroy_effect(effect, true);
        }

        // Run cleanups (in reverse order for proper nesting)
        let cleanups: Vec<_> = self.cleanups.borrow_mut().drain(..).collect();
        for cleanup in cleanups.into_iter().rev() {
            // Cleanup errors are silently ignored (like TypeScript)
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(cleanup));
        }

        // Stop child scopes
        let child_scopes: Vec<_> = self.scopes.borrow_mut().drain(..).collect();
        for child in child_scopes {
            child.stop();
        }

        // Remove from parent's scope list
        if let Some(parent) = self.parent.borrow().as_ref().and_then(|w| w.upgrade()) {
            if let Some(self_rc) = self.self_weak.borrow().upgrade() {
                parent.scopes.borrow_mut().retain(|s| !Rc::ptr_eq(s, &self_rc));
            }
        }

        self.active.set(false);
    }

    /// Pause all effects in this scope
    pub fn pause(&self) {
        if !self.active.get() || self.paused.get() {
            return;
        }

        self.paused.set(true);

        // Mark all effects as inert (paused)
        for effect in self.effects.borrow().iter() {
            let flags = effect.flags();
            effect.set_flags(flags | INERT);
        }

        // Pause child scopes
        for child in self.scopes.borrow().iter() {
            child.pause();
        }
    }

    /// Resume all paused effects in this scope
    pub fn resume(&self) {
        if !self.active.get() || !self.paused.get() {
            return;
        }

        self.paused.set(false);

        // Unmark effects and reschedule dirty ones
        for effect in self.effects.borrow().iter() {
            let flags = effect.flags();
            effect.set_flags(flags & !INERT);

            // If effect is dirty, reschedule it
            if (flags & DIRTY) != 0 {
                schedule_effect_inner(effect.clone());
            }
        }

        // Resume child scopes
        for child in self.scopes.borrow().iter() {
            child.resume();
        }
    }

    /// Add an effect to this scope
    pub fn add_effect(&self, effect: Rc<EffectInner>) {
        self.effects.borrow_mut().push(effect);
    }

    /// Add a cleanup function to this scope
    pub fn add_cleanup(&self, cleanup: ScopeCleanupFn) {
        self.cleanups.borrow_mut().push(cleanup);
    }
}

impl Drop for EffectScopeInner {
    fn drop(&mut self) {
        // Stop the scope if it's still active
        // This ensures all effects are disposed and cleanups run
        if self.active.get() {
            self.stop();
        }
    }
}

// =============================================================================
// EFFECT SCOPE (Public wrapper)
// =============================================================================

/// An effect scope that groups effects for batch disposal.
///
/// Effects created while a scope is active are automatically tracked by that scope.
/// When the scope is stopped, all tracked effects are disposed together.
///
/// # Example
///
/// ```ignore
/// let scope = effect_scope(false);
///
/// scope.run(|| {
///     // These effects are tracked by the scope
///     effect(|| println!("Effect A"));
///     effect(|| println!("Effect B"));
/// });
///
/// // Later, dispose all effects at once
/// scope.stop();
/// ```
#[derive(Clone)]
pub struct EffectScope {
    inner: Rc<EffectScopeInner>,
}

impl EffectScope {
    /// Create from inner
    fn from_inner(inner: Rc<EffectScopeInner>) -> Self {
        Self { inner }
    }

    /// Whether the scope is still active (not stopped)
    pub fn active(&self) -> bool {
        self.inner.is_active()
    }

    /// Whether the scope is paused
    pub fn paused(&self) -> bool {
        self.inner.is_paused()
    }

    /// Run a function within this scope.
    ///
    /// Effects created during execution are tracked by this scope.
    /// Returns None if the scope has been stopped.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let scope = effect_scope(false);
    ///
    /// let result = scope.run(|| {
    ///     effect(|| println!("Tracked by scope"));
    ///     42
    /// });
    ///
    /// assert_eq!(result, Some(42));
    /// ```
    pub fn run<R, F: FnOnce() -> R>(&self, f: F) -> Option<R> {
        self.inner.run(f)
    }

    /// Stop the scope, disposing all tracked effects.
    ///
    /// - All effects are destroyed
    /// - All cleanup callbacks are run (in reverse order)
    /// - All child scopes are stopped
    ///
    /// After stopping, `run()` will return None.
    pub fn stop(&self) {
        self.inner.stop();
    }

    /// Pause all effects in this scope.
    ///
    /// Paused effects won't run when their dependencies change.
    /// Changes are accumulated and effects will run when resumed.
    pub fn pause(&self) {
        self.inner.pause();
    }

    /// Resume all paused effects in this scope.
    ///
    /// Dirty effects (those whose dependencies changed while paused)
    /// will be scheduled for execution.
    pub fn resume(&self) {
        self.inner.resume();
    }
}

impl Drop for EffectScope {
    fn drop(&mut self) {
        // Auto-stop if this is the last strong reference
        // We check for 1 because we hold one reference in self.inner
        if Rc::strong_count(&self.inner) == 1 {
            self.inner.stop();
        }
    }
}

// =============================================================================
// PUBLIC API
// =============================================================================

/// Create an effect scope.
///
/// Effects created within the scope can be disposed together.
/// Child scopes are automatically disposed when the parent is stopped.
///
/// # Arguments
///
/// * `detached` - If true, scope won't be collected by parent scope
///
/// # Example
///
/// ```ignore
/// let scope = effect_scope(false);
///
/// scope.run(|| {
///     // These effects are tracked by the scope
///     effect(|| println!("count: {}", count.get()));
///     effect(|| println!("name: {}", name.get()));
/// });
///
/// // Later, dispose all effects at once
/// scope.stop();
/// ```
///
/// # Example: Pause/Resume
///
/// ```ignore
/// let scope = effect_scope(false);
///
/// scope.run(|| {
///     effect(|| println!("count: {}", count.get()));
/// });
///
/// // Pause effects (they won't run while paused)
/// scope.pause();
///
/// count.set(count.get() + 1); // Effect doesn't run
///
/// // Resume effects (pending updates will run)
/// scope.resume();
/// ```
///
/// # Example: Detached Scope
///
/// ```ignore
/// let parent = effect_scope(false);
///
/// parent.run(|| {
///     // This scope is NOT tracked by parent
///     let detached = effect_scope(true);
///     detached.run(|| {
///         effect(|| println!("I survive parent.stop()"));
///     });
/// });
///
/// parent.stop(); // Detached scope's effects still run!
/// ```
pub fn effect_scope(detached: bool) -> EffectScope {
    EffectScope::from_inner(EffectScopeInner::new(detached))
}

/// Get the currently active scope, if any.
///
/// Returns None if not inside a scope's `run()` call.
///
/// # Example
///
/// ```ignore
/// assert!(get_current_scope().is_none());
///
/// let scope = effect_scope(false);
/// scope.run(|| {
///     assert!(get_current_scope().is_some());
/// });
/// ```
pub fn get_current_scope() -> Option<EffectScope> {
    get_active_scope().map(EffectScope::from_inner)
}

/// Register a cleanup function on the current scope.
///
/// Will be called when the scope is stopped.
/// Does nothing if called outside of a scope context (with a warning).
///
/// # Example
///
/// ```ignore
/// scope.run(|| {
///     let timer = start_timer();
///
///     on_scope_dispose(move || {
///         stop_timer(timer);
///     });
/// });
///
/// // Later...
/// scope.stop(); // Timer is stopped
/// ```
pub fn on_scope_dispose<F: FnOnce() + 'static>(f: F) {
    if let Some(scope) = get_active_scope() {
        scope.add_cleanup(Box::new(f));
    } else {
        #[cfg(debug_assertions)]
        eprintln!("on_scope_dispose() called outside of scope context");
    }
}

/// Register an effect with the current scope.
///
/// Called internally when an effect is created.
/// This is what allows scopes to track and dispose effects.
pub fn register_effect_with_scope(effect: &Rc<EffectInner>) {
    if let Some(scope) = get_active_scope() {
        scope.add_effect(effect.clone());
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::effect::effect_sync;
    use crate::primitives::signal::signal;
    use std::cell::Cell;

    // =========================================================================
    // PHASE 8 SUCCESS CRITERIA TESTS
    // =========================================================================

    #[test]
    fn phase8_criteria_1_effect_scope_groups_effects() {
        // effectScope(fn) groups effects for collective disposal
        let effect_a_runs = Rc::new(Cell::new(0));
        let effect_b_runs = Rc::new(Cell::new(0));
        let effect_a_clone = effect_a_runs.clone();
        let effect_b_clone = effect_b_runs.clone();

        let count = signal(0);
        let count_a = count.clone();
        let count_b = count.clone();

        let scope = effect_scope(false);

        scope.run(|| {
            let _ = effect_sync(move || {
                let _ = count_a.get();
                effect_a_clone.set(effect_a_clone.get() + 1);
            });
            let _ = effect_sync(move || {
                let _ = count_b.get();
                effect_b_clone.set(effect_b_clone.get() + 1);
            });
        });

        // Both effects ran on creation
        assert_eq!(effect_a_runs.get(), 1);
        assert_eq!(effect_b_runs.get(), 1);

        // Change signal - both effects run
        count.set(1);
        assert_eq!(effect_a_runs.get(), 2);
        assert_eq!(effect_b_runs.get(), 2);

        // Stop scope - effects should be disposed
        scope.stop();

        // Change signal - effects should NOT run
        count.set(2);
        assert_eq!(effect_a_runs.get(), 2, "Effect A should not run after scope stop");
        assert_eq!(effect_b_runs.get(), 2, "Effect B should not run after scope stop");
    }

    #[test]
    fn phase8_criteria_2_get_current_scope() {
        // getCurrentScope() returns active scope
        assert!(get_current_scope().is_none(), "Should be None outside scope");

        let scope = effect_scope(false);
        let mut inside_scope = false;

        scope.run(|| {
            inside_scope = get_current_scope().is_some();
        });

        assert!(inside_scope, "Should be Some inside scope.run()");
        assert!(get_current_scope().is_none(), "Should be None after scope.run()");
    }

    #[test]
    fn phase8_criteria_3_on_scope_dispose() {
        // onScopeDispose(fn) registers cleanup callback
        let cleanup_called = Rc::new(Cell::new(false));
        let cleanup_clone = cleanup_called.clone();

        let scope = effect_scope(false);

        scope.run(|| {
            on_scope_dispose(move || {
                cleanup_clone.set(true);
            });
        });

        assert!(!cleanup_called.get(), "Cleanup should not run yet");

        scope.stop();

        assert!(cleanup_called.get(), "Cleanup should run on scope.stop()");
    }

    #[test]
    fn phase8_criteria_4_disposing_scope_disposes_effects() {
        // Disposing scope disposes all contained effects
        let effect_runs = Rc::new(Cell::new(0));
        let effect_clone = effect_runs.clone();

        let count = signal(0);
        let count_clone = count.clone();

        let scope = effect_scope(false);

        scope.run(|| {
            let _ = effect_sync(move || {
                let _ = count_clone.get();
                effect_clone.set(effect_clone.get() + 1);
            });
        });

        assert_eq!(effect_runs.get(), 1);

        // Dispose scope
        scope.stop();
        assert!(!scope.active(), "Scope should be inactive after stop");

        // Effect should not run
        count.set(1);
        assert_eq!(effect_runs.get(), 1, "Effect should not run after scope disposed");
    }

    // =========================================================================
    // ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn scope_run_returns_value() {
        let scope = effect_scope(false);

        let result = scope.run(|| 42);

        assert_eq!(result, Some(42));
    }

    #[test]
    fn stopped_scope_run_returns_none() {
        let scope = effect_scope(false);
        scope.stop();

        let result = scope.run(|| 42);

        assert_eq!(result, None);
    }

    #[test]
    fn nested_scopes() {
        let outer_cleanup = Rc::new(Cell::new(false));
        let inner_cleanup = Rc::new(Cell::new(false));
        let outer_clone = outer_cleanup.clone();
        let inner_clone = inner_cleanup.clone();

        let outer = effect_scope(false);

        outer.run(|| {
            on_scope_dispose(move || outer_clone.set(true));

            let inner = effect_scope(false);
            inner.run(|| {
                on_scope_dispose(move || inner_clone.set(true));
            });
        });

        // Stop outer - should stop inner too
        outer.stop();

        assert!(outer_cleanup.get(), "Outer cleanup should run");
        assert!(inner_cleanup.get(), "Inner cleanup should run");
    }

    #[test]
    fn detached_scope_not_stopped_by_parent() {
        let detached_cleanup = Rc::new(Cell::new(false));
        let detached_clone = detached_cleanup.clone();

        let parent = effect_scope(false);

        let detached = parent.run(|| {
            let detached = effect_scope(true); // detached = true
            detached.run(|| {
                on_scope_dispose(move || detached_clone.set(true));
            });
            detached
        }).unwrap();

        // Stop parent
        parent.stop();

        assert!(!detached_cleanup.get(), "Detached cleanup should NOT run");
        assert!(detached.active(), "Detached scope should still be active");

        // Stop detached manually
        detached.stop();
        assert!(detached_cleanup.get(), "Detached cleanup should run now");
    }

    #[test]
    fn scope_pause_resume() {
        let effect_runs = Rc::new(Cell::new(0));
        let effect_clone = effect_runs.clone();

        let count = signal(0);
        let count_clone = count.clone();

        let scope = effect_scope(false);

        scope.run(|| {
            let _ = effect_sync(move || {
                let _ = count_clone.get();
                effect_clone.set(effect_clone.get() + 1);
            });
        });

        assert_eq!(effect_runs.get(), 1);

        // Pause
        scope.pause();
        assert!(scope.paused());

        // Changes don't trigger effect while paused
        count.set(1);
        assert_eq!(effect_runs.get(), 1, "Effect should not run while paused");

        // Resume - dirty effect should run
        scope.resume();
        assert!(!scope.paused());
        assert_eq!(effect_runs.get(), 2, "Effect should run on resume");
    }

    #[test]
    fn multiple_cleanups_run_in_reverse_order() {
        let order = Rc::new(RefCell::new(Vec::new()));
        let order1 = order.clone();
        let order2 = order.clone();
        let order3 = order.clone();

        let scope = effect_scope(false);

        scope.run(|| {
            on_scope_dispose(move || order1.borrow_mut().push(1));
            on_scope_dispose(move || order2.borrow_mut().push(2));
            on_scope_dispose(move || order3.borrow_mut().push(3));
        });

        scope.stop();

        // Cleanups run in reverse order (LIFO)
        assert_eq!(*order.borrow(), vec![3, 2, 1]);
    }

    #[test]
    fn scope_active_and_paused_flags() {
        let scope = effect_scope(false);

        assert!(scope.active());
        assert!(!scope.paused());

        scope.pause();
        assert!(scope.active());
        assert!(scope.paused());

        scope.resume();
        assert!(scope.active());
        assert!(!scope.paused());

        scope.stop();
        assert!(!scope.active());
    }

    #[test]
    fn effect_cleanup_runs_on_scope_stop() {
        let effect_cleanup = Rc::new(Cell::new(false));
        let effect_clone = effect_cleanup.clone();

        let count = signal(0);
        let count_clone = count.clone();

        let scope = effect_scope(false);

        scope.run(|| {
            let _ = crate::primitives::effect::effect_sync_with_cleanup(move || {
                let _ = count_clone.get();
                let ec = effect_clone.clone();
                Some(Box::new(move || ec.set(true)))
            });
        });

        assert!(!effect_cleanup.get());

        scope.stop();

        assert!(effect_cleanup.get(), "Effect cleanup should run on scope stop");
    }
}
