// ============================================================================
// spark-signals - Effect System
// Side effects that re-run when dependencies change
// ============================================================================
//
// Effects are reactions that run side effects when their dependencies change.
// Unlike deriveds, effects don't produce values - they just run code.
//
// Key features:
// - Automatic dependency tracking (like deriveds)
// - Cleanup/teardown functions
// - Effect tree (parent/child relationships)
// - Scheduling (sync vs async)
// - RAII disposal
// ============================================================================

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::core::constants::*;
use crate::core::context::with_context;
use crate::core::types::{AnyReaction, AnySource};
use crate::primitives::scope::register_effect_with_scope;
use crate::reactivity::tracking::{remove_reactions, set_signal_status};

// =============================================================================
// TYPE ALIASES
// =============================================================================

/// Cleanup function returned by effects, runs before next execution
pub type CleanupFn = Box<dyn FnOnce()>;

/// Effect function signature - returns optional cleanup
pub type EffectFn = Box<dyn FnMut() -> Option<CleanupFn>>;

/// Dispose function returned when creating effects
pub type DisposeFn = Box<dyn FnOnce()>;

// =============================================================================
// EFFECT INNER
// =============================================================================

/// The inner effect implementation.
///
/// Implements AnyReaction (but NOT AnySource - effects are reactions only).
/// Holds the effect function, dependencies, teardown, and effect tree structure.
pub struct EffectInner {
    // =========================================================================
    // Core state (from Signal + Reaction)
    // =========================================================================
    /// Flags bitmask for state tracking
    flags: Cell<u32>,

    /// Write version - when this effect last ran
    write_version: Cell<u32>,

    /// The effect function
    func: RefCell<Option<EffectFn>>,

    /// Dependencies (sources/deriveds this effect reads)
    deps: RefCell<Vec<Rc<dyn AnySource>>>,

    /// Teardown/cleanup function from last run
    teardown: RefCell<Option<CleanupFn>>,

    // =========================================================================
    // Effect tree (parent/children/siblings)
    // =========================================================================
    /// Parent effect in the effect tree
    parent: RefCell<Option<Weak<EffectInner>>>,

    /// First child effect
    first_child: RefCell<Option<Rc<EffectInner>>>,

    /// Last child effect (Weak to avoid cycles)
    last_child: RefCell<Option<Weak<EffectInner>>>,

    /// Previous sibling (Weak to avoid cycles)
    prev_sibling: RefCell<Option<Weak<EffectInner>>>,

    /// Next sibling
    next_sibling: RefCell<Option<Rc<EffectInner>>>,

    // =========================================================================
    // Self-reference for trait object conversion
    // =========================================================================
    /// Weak reference to self (set after Rc creation)
    self_weak: RefCell<Weak<EffectInner>>,
}

impl EffectInner {
    /// Create a new effect inner
    pub fn new(effect_type: u32, func: Option<EffectFn>) -> Rc<Self> {
        let effect = Rc::new(Self {
            flags: Cell::new(effect_type | DIRTY),
            write_version: Cell::new(0),
            func: RefCell::new(func),
            deps: RefCell::new(Vec::new()),
            teardown: RefCell::new(None),
            parent: RefCell::new(None),
            first_child: RefCell::new(None),
            last_child: RefCell::new(None),
            prev_sibling: RefCell::new(None),
            next_sibling: RefCell::new(None),
            self_weak: RefCell::new(Weak::new()),
        });

        // Store weak self-reference
        *effect.self_weak.borrow_mut() = Rc::downgrade(&effect);

        effect
    }

    /// Get this effect as a weak reference to AnyReaction
    pub fn as_weak_reaction(&self) -> Weak<dyn AnyReaction> {
        // Upgrade self_weak to get Rc<EffectInner>, then convert to Rc<dyn AnyReaction>
        if let Some(rc) = self.self_weak.borrow().upgrade() {
            Rc::downgrade(&(rc as Rc<dyn AnyReaction>))
        } else {
            Weak::<EffectInner>::new() as Weak<dyn AnyReaction>
        }
    }

    /// Get parent effect
    pub fn parent(&self) -> Option<Rc<EffectInner>> {
        self.parent.borrow().as_ref().and_then(|w| w.upgrade())
    }

    /// Set parent effect
    pub fn set_parent(&self, parent: Option<Weak<EffectInner>>) {
        *self.parent.borrow_mut() = parent;
    }

    /// Get first child effect
    pub fn first_child(&self) -> Option<Rc<EffectInner>> {
        self.first_child.borrow().clone()
    }

    /// Get last child effect
    pub fn last_child(&self) -> Option<Rc<EffectInner>> {
        self.last_child.borrow().as_ref().and_then(|w| w.upgrade())
    }
}

impl Drop for EffectInner {
    fn drop(&mut self) {
        // Run teardown if present
        if let Some(cleanup) = self.teardown.borrow_mut().take() {
            cleanup();
        }
    }
}

// =============================================================================
// AnyReaction IMPLEMENTATION
// =============================================================================

impl AnyReaction for EffectInner {
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
        // Effects don't return a value change indicator in the same way deriveds do.
        // The update() method runs the effect function.
        // Returns false since effects don't have a "value changed" concept.

        // Skip if destroyed
        if (self.flags.get() & DESTROYED) != 0 {
            return false;
        }

        // Get Rc<Self> from the stored weak reference
        if let Some(rc_self) = self.self_weak.borrow().upgrade() {
            update_effect(&rc_self);
        }

        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_derived_source(&self) -> Option<Rc<dyn AnySource>> {
        // Effects are NOT sources - they don't have dependents
        None
    }
}

// =============================================================================
// EFFECT WRAPPER
// =============================================================================

/// Public effect wrapper providing the user API.
///
/// Holds an Rc<EffectInner> and provides methods for disposal.
pub struct Effect {
    inner: Rc<EffectInner>,
}

impl Effect {
    /// Create a new effect from an EffectInner
    #[allow(dead_code)]
    pub(crate) fn from_inner(inner: Rc<EffectInner>) -> Self {
        Self { inner }
    }

    /// Get access to the inner effect
    pub fn inner(&self) -> &Rc<EffectInner> {
        &self.inner
    }

    /// Check if this effect is destroyed
    pub fn is_destroyed(&self) -> bool {
        (self.inner.flags.get() & DESTROYED) != 0
    }

    /// Dispose/destroy this effect
    pub fn dispose(&self) {
        destroy_effect(self.inner.clone(), true);
    }
}

impl Drop for Effect {
    fn drop(&mut self) {
        // Auto-destroy if this is the last strong reference to the inner effect
        // Note: Effects in the graph have weak parent references but strong child references
        // However, if the user drops the handle, and it's a root effect or detached,
        // we might want to stop it.
        // If it has a parent, the parent holds it strongly, so strong_count > 1.
        // If it's a root effect, strong_count == 1 (this handle).
        if Rc::strong_count(&self.inner) == 1 {
            self.dispose();
        }
    }
}

impl Clone for Effect {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// =============================================================================
// PUSH EFFECT - Add to parent's child list
// =============================================================================

/// Add an effect to its parent's child list
pub(crate) fn push_effect(effect: &Rc<EffectInner>, parent: &Rc<EffectInner>) {
    let parent_last = parent.last_child();

    if parent_last.is_none() {
        // First child
        *parent.first_child.borrow_mut() = Some(effect.clone());
        *parent.last_child.borrow_mut() = Some(Rc::downgrade(effect));
    } else {
        // Append to end
        let last = parent_last.unwrap();
        *last.next_sibling.borrow_mut() = Some(effect.clone());
        *effect.prev_sibling.borrow_mut() = Some(Rc::downgrade(&last));
        *parent.last_child.borrow_mut() = Some(Rc::downgrade(effect));
    }
}

// =============================================================================
// UNLINK EFFECT - Remove from parent's child list
// =============================================================================

/// Remove an effect from its parent's child list
fn unlink_effect(effect: &Rc<EffectInner>) {
    let prev = effect.prev_sibling.borrow().as_ref().and_then(|w| w.upgrade());
    let next = effect.next_sibling.borrow().clone();

    // Update prev's next pointer
    if let Some(ref prev_rc) = prev {
        *prev_rc.next_sibling.borrow_mut() = next.clone();
    }

    // Update next's prev pointer
    if let Some(ref next_rc) = next {
        *next_rc.prev_sibling.borrow_mut() = prev.as_ref().map(Rc::downgrade);
    }

    // Update parent's first/last pointers
    if let Some(parent) = effect.parent() {
        // Check if we're the first child
        if let Some(ref first) = *parent.first_child.borrow() {
            if Rc::ptr_eq(first, effect) {
                *parent.first_child.borrow_mut() = next.clone();
            }
        }

        // Check if we're the last child
        if let Some(last_weak) = parent.last_child.borrow().as_ref() {
            if let Some(last) = last_weak.upgrade() {
                if Rc::ptr_eq(&last, effect) {
                    *parent.last_child.borrow_mut() = prev.as_ref().map(Rc::downgrade);
                }
            }
        }
    }

    // Clear our own pointers
    *effect.prev_sibling.borrow_mut() = None;
    *effect.next_sibling.borrow_mut() = None;
}

// =============================================================================
// EXECUTE TEARDOWN
// =============================================================================

/// Run an effect's teardown function
pub(crate) fn execute_teardown(effect: &EffectInner) {
    let teardown = effect.teardown.borrow_mut().take();
    if let Some(cleanup) = teardown {
        cleanup();
    }
}

// =============================================================================
// DESTROY EFFECT CHILDREN
// =============================================================================

/// Destroy all children of an effect
pub(crate) fn destroy_effect_children(effect: &Rc<EffectInner>) {
    let mut child = effect.first_child.borrow_mut().take();
    *effect.last_child.borrow_mut() = None;

    // Collect all children first to avoid structural modifications during iteration
    // (e.g. if a teardown triggers unlinking of a sibling)
    let mut children = Vec::new();
    while let Some(c) = child {
        child = c.next_sibling.borrow_mut().take();
        // Clear prev sibling too to fully detach
        *c.prev_sibling.borrow_mut() = None;
        children.push(c);
    }

    for child_rc in children {
        // Don't destroy preserved or root effects
        let flags = child_rc.flags.get();
        if (flags & (EFFECT_PRESERVED | ROOT_EFFECT)) == 0 {
            destroy_effect(child_rc, false);
        }
    }
}

// =============================================================================
// DESTROY EFFECT
// =============================================================================

/// Destroy an effect and all its children
pub fn destroy_effect(effect: Rc<EffectInner>, remove_from_parent: bool) {
    // Recursively destroy children
    destroy_effect_children(&effect);

    // Remove from all dependencies
    remove_reactions(effect.clone() as Rc<dyn AnyReaction>, 0);

    // Mark as destroyed
    set_signal_status(&*effect, DESTROYED);

    // Run teardown
    execute_teardown(&*effect);

    // Remove from parent's child list
    if remove_from_parent && effect.parent().is_some() {
        unlink_effect(&effect);
    }

    // Clear parent reference
    *effect.parent.borrow_mut() = None;

    // Nullify for cleanup (let Rc drop handles do their job)
    *effect.func.borrow_mut() = None;
    *effect.teardown.borrow_mut() = None;
    effect.deps.borrow_mut().clear();
    *effect.first_child.borrow_mut() = None;
    *effect.last_child.borrow_mut() = None;
    *effect.prev_sibling.borrow_mut() = None;
    *effect.next_sibling.borrow_mut() = None;
}

// =============================================================================
// UPDATE EFFECT - Run an effect
// =============================================================================

/// Run an effect and track its dependencies.
///
/// This is the core function that:
/// 1. Sets up the reaction context
/// 2. Destroys child effects from previous run
/// 3. Runs teardown from previous run
/// 4. Executes the effect function with dependency tracking
/// 5. Stores new teardown if returned
pub fn update_effect(effect: &Rc<EffectInner>) {
    // Skip if destroyed
    if (effect.flags.get() & DESTROYED) != 0 {
        return;
    }

    // Mark as clean
    set_signal_status(&**effect, CLEAN);

    // Destroy child effects from previous run
    destroy_effect_children(effect);

    // Run teardown from previous run
    execute_teardown(&**effect);

    // Set up reaction context and run the effect function
    let (prev_reaction, prev_effect) = with_context(|ctx| {
        let prev_r = ctx.set_active_reaction(Some(effect.as_weak_reaction()));
        let prev_e = ctx.set_active_effect(Some(effect.as_weak_reaction()));

        // Start new read cycle
        ctx.increment_read_version();

        // Set up for dependency collection
        ctx.set_skipped_deps(0);
        ctx.swap_new_deps(Vec::new());

        // Mark as updating
        effect.set_flags(effect.flags() | REACTION_IS_UPDATING);

        (prev_r, prev_e)
    });

    // Run the effect function
    let teardown = {
        let mut func_borrow = effect.func.borrow_mut();
        if let Some(ref mut func) = *func_borrow {
            func()
        } else {
            None
        }
    };

    // Restore context and install dependencies
    with_context(|ctx| {
        // Clear updating flag
        effect.set_flags(effect.flags() & !REACTION_IS_UPDATING);

        // Get skipped count before restoring
        let skipped = ctx.get_skipped_deps();

        // Take collected deps
        let new_deps = ctx.swap_new_deps(Vec::new());

        // Restore previous reaction and effect
        ctx.set_active_reaction(prev_reaction);
        ctx.set_active_effect(prev_effect);

        // Install dependencies: remove old, add new
        // First remove deps from skipped onwards
        remove_reactions(effect.clone() as Rc<dyn AnyReaction>, skipped);

        // Add new deps
        for dep in new_deps {
            effect.add_dep(dep.clone());
            dep.add_reaction(Rc::downgrade(&(effect.clone() as Rc<dyn AnyReaction>)));
        }

        // Update write version
        effect.write_version.set(ctx.increment_write_version());
    });

    // Store teardown if returned
    *effect.teardown.borrow_mut() = teardown;
}

// =============================================================================
// PUBLIC API
// =============================================================================

/// Create an effect that runs when dependencies change.
///
/// The effect function is tracked for dependencies - any signals read inside
/// will be registered as dependencies. When those signals change, the effect
/// will re-run.
///
/// Returns a dispose function that destroys the effect when called.
///
/// # Example
///
/// ```ignore
/// let count = signal(0);
///
/// let dispose = effect(|| {
///     println!("Count: {}", count.get());
/// });
///
/// count.set(1); // Effect runs: "Count: 1"
/// count.set(2); // Effect runs: "Count: 2"
///
/// dispose(); // Effect is destroyed
/// count.set(3); // Effect does NOT run
/// ```
pub fn effect<F>(mut f: F) -> impl FnOnce()
where
    F: FnMut() + 'static,
{
    effect_with_cleanup(move || {
        f();
        None
    })
}

/// Create an effect that can return a cleanup function.
///
/// The cleanup function runs before each re-execution and when disposed.
///
/// # Example
///
/// ```ignore
/// let count = signal(0);
///
/// let dispose = effect_with_cleanup(|| {
///     let id = subscribe_to_something();
///     println!("Count: {}", count.get());
///
///     Some(Box::new(move || {
///         unsubscribe(id);
///     }))
/// });
/// ```
pub fn effect_with_cleanup<F>(f: F) -> impl FnOnce()
where
    F: FnMut() -> Option<CleanupFn> + 'static,
{
    let effect = create_effect(EFFECT | USER_EFFECT, Box::new(f), false, true);
    let effect_clone = effect.clone();
    move || destroy_effect(effect_clone, true)
}

/// Create a synchronous effect that runs immediately when dependencies change.
///
/// Unlike regular `effect()` which may be batched (in environments with
/// microtasks), sync effects execute immediately after each signal write.
///
/// In Rust without microtasks, this behaves identically to `effect()`.
///
/// # Example
///
/// ```ignore
/// let count = signal(0);
/// effect_sync(|| println!("Count: {}", count.get())); // Runs immediately
///
/// count.set(1); // Effect runs immediately
/// count.set(2); // Effect runs immediately
/// ```
pub fn effect_sync<F>(mut f: F) -> impl FnOnce()
where
    F: FnMut() + 'static,
{
    effect_sync_with_cleanup(move || {
        f();
        None
    })
}

/// Create a sync effect that can return a cleanup function.
pub fn effect_sync_with_cleanup<F>(f: F) -> impl FnOnce()
where
    F: FnMut() -> Option<CleanupFn> + 'static,
{
    // NOTE: Must include EFFECT flag for mark_reactions scheduling to work!
    let effect = create_effect(EFFECT | RENDER_EFFECT | USER_EFFECT, Box::new(f), true, true);
    let effect_clone = effect.clone();
    move || destroy_effect(effect_clone, true)
}

/// Create a root effect scope.
///
/// A root effect creates a scope for child effects. When the root is disposed,
/// all child effects are also disposed.
///
/// Returns a dispose function that destroys the root and all its children.
///
/// # Example
///
/// ```ignore
/// let dispose = effect_root(|| {
///     effect(|| println!("Effect A"));
///     effect(|| println!("Effect B"));
/// });
///
/// // Later, clean up all effects at once
/// dispose();
/// ```
pub fn effect_root<F>(f: F) -> impl FnOnce()
where
    F: FnOnce() + 'static,
{
    // Root effects run their function once (FnOnce), not repeatedly
    let f_cell = std::cell::Cell::new(Some(f));

    let effect = create_effect(
        ROOT_EFFECT | EFFECT_PRESERVED,
        Box::new(move || {
            if let Some(func) = f_cell.take() {
                func();
            }
            None
        }),
        true, // Run synchronously
        true,
    );

    let effect_clone = effect.clone();
    move || destroy_effect(effect_clone, true)
}

/// Check if we're currently inside a tracking context.
///
/// Returns true if code is running inside an effect or derived,
/// meaning signal reads will be tracked as dependencies.
///
/// # Example
///
/// ```ignore
/// assert!(!effect_tracking()); // Not in tracking context
///
/// effect(|| {
///     assert!(effect_tracking()); // Inside effect = tracking
/// });
/// ```
pub fn effect_tracking() -> bool {
    with_context(|ctx| ctx.has_active_reaction())
}

// =============================================================================
// CREATE EFFECT (Internal)
// =============================================================================

/// Create an effect (internal).
///
/// # Arguments
///
/// * `effect_type` - Effect type flags (EFFECT, RENDER_EFFECT, ROOT_EFFECT, etc.)
/// * `func` - The effect function
/// * `sync` - Whether to run synchronously (immediately)
/// * `push` - Whether to add to parent's child list
fn create_effect(
    effect_type: u32,
    func: EffectFn,
    sync: bool,
    push: bool,
) -> Rc<EffectInner> {
    let effect = EffectInner::new(effect_type, Some(func));

    // Register with current scope (if any)
    register_effect_with_scope(&effect);

    // Get parent effect if we're inside one
    let parent = with_context(|ctx| {
        ctx.get_active_effect().and_then(|w| w.upgrade())
    });

    // Set parent on the new effect
    if let Some(ref parent_rc) = parent {
        // Try to downcast to EffectInner
        if let Some(parent_inner) = parent_rc.as_any().downcast_ref::<EffectInner>() {
            // Get the parent's Rc from its self_weak
            if let Some(parent_effect) = parent_inner.self_weak.borrow().upgrade() {
                effect.set_parent(Some(Rc::downgrade(&parent_effect)));

                // Add to parent's child list if push is true
                if push {
                    push_effect(&effect, &parent_effect);
                }
            }
        }
    }

    // Run immediately if sync, otherwise schedule
    if sync {
        update_effect(&effect);
        // Mark as having run
        effect.set_flags(effect.flags() | EFFECT_RAN);
    } else {
        // Schedule for later execution
        crate::reactivity::scheduling::schedule_effect_inner(effect.clone());
    }

    effect
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::signal::signal;

    // =========================================================================
    // PHASE 5 SUCCESS CRITERIA TESTS
    // =========================================================================

    #[test]
    fn phase5_criteria_1_effect_runs_on_dependency_change() {
        // User can create effect with `effect(|| side_effect)` that runs on dependency change
        let run_count = Rc::new(Cell::new(0));
        let run_count_clone = run_count.clone();

        let count = signal(0);
        let count_clone = count.clone();

        let _dispose = effect(move || {
            let _ = count_clone.get(); // Create dependency
            run_count_clone.set(run_count_clone.get() + 1);
        });

        // Effect should have run once on creation
        assert_eq!(run_count.get(), 1, "Effect should run on creation");

        // Change signal - effect should run again
        count.set(1);
        assert_eq!(run_count.get(), 2, "Effect should run when dependency changes");

        // Change signal again
        count.set(2);
        assert_eq!(run_count.get(), 3, "Effect should run on each change");
    }

    #[test]
    fn phase5_criteria_2_cleanup_function_called_before_rerun() {
        // Effects support cleanup functions (returned value is called before re-run)
        let cleanup_count = Rc::new(Cell::new(0));
        let cleanup_clone = cleanup_count.clone();

        let count = signal(0);
        let count_clone = count.clone();

        let _dispose = effect_with_cleanup(move || {
            let _ = count_clone.get();
            let cc = cleanup_clone.clone();
            Some(Box::new(move || {
                cc.set(cc.get() + 1);
            }) as CleanupFn)
        });

        // Cleanup hasn't run yet (effect just created)
        assert_eq!(cleanup_count.get(), 0);

        // Change signal - cleanup from previous run should execute
        count.set(1);
        assert_eq!(cleanup_count.get(), 1, "Cleanup should run before re-run");

        // Change again
        count.set(2);
        assert_eq!(cleanup_count.get(), 2, "Cleanup should run each time");
    }

    #[test]
    fn phase5_criteria_3_effect_sync_runs_immediately() {
        // effect.sync() runs immediately without scheduling
        let run_order = Rc::new(RefCell::new(Vec::new()));
        let run_order_clone = run_order.clone();

        run_order.borrow_mut().push("before");

        let count = signal(0);
        let count_clone = count.clone();

        let _dispose = effect_sync(move || {
            let _ = count_clone.get();
            run_order_clone.borrow_mut().push("effect");
        });

        run_order.borrow_mut().push("after");

        // Effect should have run synchronously, between before and after
        assert_eq!(
            *run_order.borrow(),
            vec!["before", "effect", "after"],
            "Sync effect should run immediately"
        );
    }

    #[test]
    fn phase5_criteria_4_effect_root_creates_scope() {
        // effect.root() creates unparented effect that groups children
        let effect_a_runs = Rc::new(Cell::new(0));
        let effect_b_runs = Rc::new(Cell::new(0));
        let effect_a_runs_clone = effect_a_runs.clone();
        let effect_b_runs_clone = effect_b_runs.clone();

        let count = signal(0);
        let count_a = count.clone();
        let count_b = count.clone();

        let dispose = effect_root(move || {
            // Child effects - their dispose functions are ignored since root manages them
            let _dispose_a = effect(move || {
                let _ = count_a.get();
                effect_a_runs_clone.set(effect_a_runs_clone.get() + 1);
            });
            let _dispose_b = effect(move || {
                let _ = count_b.get();
                effect_b_runs_clone.set(effect_b_runs_clone.get() + 1);
            });
        });

        // Both effects should have run
        assert_eq!(effect_a_runs.get(), 1);
        assert_eq!(effect_b_runs.get(), 1);

        // Dispose the root - children should be destroyed
        dispose();

        // Change signal - effects should NOT run (they're disposed)
        count.set(1);
        assert_eq!(effect_a_runs.get(), 1, "Effect A should not run after root disposed");
        assert_eq!(effect_b_runs.get(), 1, "Effect B should not run after root disposed");
    }

    #[test]
    fn phase5_criteria_5_dispose_function_destroys_effect() {
        // Dispose function destroys effect (RAII-like cleanup)
        let run_count = Rc::new(Cell::new(0));
        let run_count_clone = run_count.clone();

        let count = signal(0);
        let count_clone = count.clone();

        let dispose = effect(move || {
            let _ = count_clone.get();
            run_count_clone.set(run_count_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        // Dispose the effect
        dispose();

        // Change signal - effect should NOT run
        count.set(1);
        assert_eq!(run_count.get(), 1, "Effect should not run after dispose");

        count.set(2);
        assert_eq!(run_count.get(), 1, "Effect should remain disposed");
    }

    #[test]
    fn phase5_criteria_6_dispose_runs_cleanup() {
        // When disposed, cleanup function should run
        let cleanup_called = Rc::new(Cell::new(false));
        let cleanup_called_clone = cleanup_called.clone();

        let count = signal(0);
        let count_clone = count.clone();

        let dispose = effect_with_cleanup(move || {
            let _ = count_clone.get();
            let cc = cleanup_called_clone.clone();
            Some(Box::new(move || {
                cc.set(true);
            }) as CleanupFn)
        });

        assert!(!cleanup_called.get());

        // Dispose should trigger cleanup
        dispose();

        assert!(cleanup_called.get(), "Cleanup should run on dispose");
    }

    #[test]
    fn phase5_effect_tracking_function() {
        // effect_tracking() returns true inside effects
        assert!(!effect_tracking(), "Should be false outside effect");

        let was_tracking = Rc::new(Cell::new(false));
        let was_tracking_clone = was_tracking.clone();

        let _dispose = effect_sync(move || {
            was_tracking_clone.set(effect_tracking());
        });

        assert!(was_tracking.get(), "Should be true inside effect");
    }

    #[test]
    #[should_panic(expected = "Maximum update depth exceeded")]
    fn phase5_criteria_7_infinite_loop_detection() {
        // Infinite loop detection prevents self-invalidating effects
        let count = signal(0);
        let count_clone = count.clone();

        // This effect reads AND writes the same signal - infinite loop!
        let _dispose = effect(move || {
            let current = count_clone.get();
            count_clone.set(current + 1); // Triggers effect again...
        });

        // After the first run, the effect is registered as a dependency of count.
        // Now trigger the effect by writing to count - this creates an infinite loop
        // because the effect will keep writing to count, triggering itself.
        count.set(0);

        // Should panic with "Maximum update depth exceeded" before reaching here
    }

    // =========================================================================
    // UNIT TESTS
    // =========================================================================

    #[test]
    fn effect_inner_creation() {
        let effect = EffectInner::new(EFFECT | USER_EFFECT, None);

        // Should have EFFECT and USER_EFFECT flags plus DIRTY
        let flags = effect.flags.get();
        assert!((flags & EFFECT) != 0);
        assert!((flags & USER_EFFECT) != 0);
        assert!((flags & DIRTY) != 0);
    }

    #[test]
    fn effect_inner_implements_any_reaction() {
        let effect = EffectInner::new(EFFECT, None);

        // Test AnyReaction methods
        assert_eq!(effect.dep_count(), 0);
        assert!(!effect.is_clean());
        assert!(effect.is_dirty());

        effect.mark_clean();
        assert!(effect.is_clean());
    }

    #[test]
    fn effect_tree_structure() {
        let parent = EffectInner::new(ROOT_EFFECT, None);
        let child1 = EffectInner::new(EFFECT, None);
        let child2 = EffectInner::new(EFFECT, None);

        // Set parent on children
        child1.set_parent(Some(Rc::downgrade(&parent)));
        child2.set_parent(Some(Rc::downgrade(&parent)));

        // Push children to parent
        push_effect(&child1, &parent);
        push_effect(&child2, &parent);

        // Verify tree structure
        assert!(parent.first_child().is_some());
        assert!(Rc::ptr_eq(&parent.first_child().unwrap(), &child1));
        assert!(Rc::ptr_eq(&parent.last_child().unwrap(), &child2));

        // Verify sibling links
        assert!(child1.next_sibling.borrow().is_some());
        assert!(child2.prev_sibling.borrow().as_ref().unwrap().upgrade().is_some());
    }

    #[test]
    fn effect_teardown() {
        use std::cell::Cell;
        use std::rc::Rc;

        let teardown_called = Rc::new(Cell::new(false));
        let teardown_called_clone = teardown_called.clone();

        let effect = EffectInner::new(EFFECT, None);
        *effect.teardown.borrow_mut() = Some(Box::new(move || {
            teardown_called_clone.set(true);
        }));

        assert!(!teardown_called.get());
        execute_teardown(&*effect);
        assert!(teardown_called.get());

        // Teardown should be consumed
        assert!(effect.teardown.borrow().is_none());
    }

    #[test]
    fn destroy_effect_marks_destroyed() {
        let effect = EffectInner::new(EFFECT, None);

        assert!((effect.flags.get() & DESTROYED) == 0);

        destroy_effect(effect.clone(), false);

        assert!((effect.flags.get() & DESTROYED) != 0);
    }

    #[test]
    fn destroy_effect_runs_teardown() {
        let teardown_called = Rc::new(Cell::new(false));
        let teardown_called_clone = teardown_called.clone();

        let effect = EffectInner::new(EFFECT, None);
        *effect.teardown.borrow_mut() = Some(Box::new(move || {
            teardown_called_clone.set(true);
        }));

        destroy_effect(effect.clone(), false);

        assert!(teardown_called.get());
    }

    #[test]
    fn destroy_effect_destroys_children() {
        let parent = EffectInner::new(EFFECT, None);
        let child = EffectInner::new(EFFECT, None);

        child.set_parent(Some(Rc::downgrade(&parent)));
        push_effect(&child, &parent);

        // Verify child is linked
        assert!(parent.first_child().is_some());

        // Destroy parent
        destroy_effect(parent.clone(), false);

        // Parent should have no children
        assert!(parent.first_child().is_none());

        // Child should be destroyed
        assert!((child.flags.get() & DESTROYED) != 0);
    }

    #[test]
    fn update_effect_runs_function() {
        let run_count = Rc::new(Cell::new(0));
        let run_count_clone = run_count.clone();

        let effect = EffectInner::new(
            EFFECT,
            Some(Box::new(move || {
                run_count_clone.set(run_count_clone.get() + 1);
                None
            })),
        );

        assert_eq!(run_count.get(), 0);

        update_effect(&effect);

        assert_eq!(run_count.get(), 1);
    }

    #[test]
    fn update_effect_stores_teardown() {
        let effect = EffectInner::new(
            EFFECT,
            Some(Box::new(|| {
                Some(Box::new(|| {}) as CleanupFn)
            })),
        );

        assert!(effect.teardown.borrow().is_none());

        update_effect(&effect);

        assert!(effect.teardown.borrow().is_some());
    }

    #[test]
    fn update_effect_runs_previous_teardown() {
        let teardown_run = Rc::new(Cell::new(0));
        let teardown_run_clone = teardown_run.clone();

        let effect = EffectInner::new(
            EFFECT,
            Some(Box::new(move || {
                let tr = teardown_run_clone.clone();
                Some(Box::new(move || {
                    tr.set(tr.get() + 1);
                }) as CleanupFn)
            })),
        );

        // First run - no teardown yet
        update_effect(&effect);
        assert_eq!(teardown_run.get(), 0);

        // Second run - previous teardown should run
        update_effect(&effect);
        assert_eq!(teardown_run.get(), 1);

        // Third run - teardown runs again
        update_effect(&effect);
        assert_eq!(teardown_run.get(), 2);
    }

    #[test]
    fn update_effect_marks_clean() {
        let effect = EffectInner::new(EFFECT, Some(Box::new(|| None)));

        // Starts dirty
        assert!(effect.is_dirty());

        update_effect(&effect);

        // Should be clean after running
        assert!(effect.is_clean());
    }

    #[test]
    fn update_effect_skips_destroyed() {
        let run_count = Rc::new(Cell::new(0));
        let run_count_clone = run_count.clone();

        let effect = EffectInner::new(
            EFFECT,
            Some(Box::new(move || {
                run_count_clone.set(run_count_clone.get() + 1);
                None
            })),
        );

        // Destroy the effect
        effect.set_flags(effect.flags.get() | DESTROYED);

        update_effect(&effect);

        // Should not have run
        assert_eq!(run_count.get(), 0);
    }
}
