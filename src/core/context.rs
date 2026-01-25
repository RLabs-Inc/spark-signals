// ============================================================================
// spark-signals - Reactive Context
// Thread-local state for tracking the current reaction context
// ============================================================================

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::types::{AnyReaction, AnySource};

// =============================================================================
// REACTIVE CONTEXT
// =============================================================================

/// Thread-local reactive context holding all global state for reactivity.
///
/// This mirrors the globals from the TypeScript implementation but uses
/// a struct + thread_local for better organization.
pub struct ReactiveContext {
    // =========================================================================
    // REACTION TRACKING
    // =========================================================================
    /// Currently executing reaction (effect or derived)
    pub active_reaction: RefCell<Option<Weak<dyn AnyReaction>>>,

    /// Currently executing effect (for effect tree management)
    pub active_effect: RefCell<Option<Weak<dyn AnyReaction>>>,

    /// Whether we're currently untracking (reading without creating dependencies)
    pub untracking: Cell<bool>,

    // =========================================================================
    // VERSION COUNTERS
    // =========================================================================
    /// Global write version - incremented on every signal write
    pub write_version: Cell<u32>,

    /// Global read version - incremented on every reaction run
    pub read_version: Cell<u32>,

    // =========================================================================
    // DEPENDENCY TRACKING (during reaction execution)
    // =========================================================================
    /// New dependencies collected during current reaction execution
    pub new_deps: RefCell<Vec<Rc<dyn AnySource>>>,

    /// Number of existing dependencies that matched (optimization)
    pub skipped_deps: Cell<usize>,

    /// Signals written to during current reaction (for self-invalidation detection)
    pub untracked_writes: RefCell<Vec<Rc<dyn AnySource>>>,

    // =========================================================================
    // BATCHING
    // =========================================================================
    /// Current batch depth (for nested batches)
    pub batch_depth: Cell<u32>,

    /// Pending reactions to run after batch completes
    pub pending_reactions: RefCell<Vec<Weak<dyn AnyReaction>>>,

    /// Queued root effects to process
    pub queued_root_effects: RefCell<Vec<Weak<dyn AnyReaction>>>,

    /// Whether we're currently flushing synchronously
    pub is_flushing_sync: Cell<bool>,
}

impl ReactiveContext {
    /// Create a new reactive context with default values
    pub fn new() -> Self {
        Self {
            active_reaction: RefCell::new(None),
            active_effect: RefCell::new(None),
            untracking: Cell::new(false),
            write_version: Cell::new(1),
            read_version: Cell::new(0),
            new_deps: RefCell::new(Vec::new()),
            skipped_deps: Cell::new(0),
            untracked_writes: RefCell::new(Vec::new()),
            batch_depth: Cell::new(0),
            pending_reactions: RefCell::new(Vec::new()),
            queued_root_effects: RefCell::new(Vec::new()),
            is_flushing_sync: Cell::new(false),
        }
    }

    // =========================================================================
    // REACTION TRACKING
    // =========================================================================

    /// Set the active reaction, returning the previous one
    pub fn set_active_reaction(
        &self,
        reaction: Option<Weak<dyn AnyReaction>>,
    ) -> Option<Weak<dyn AnyReaction>> {
        self.active_reaction.replace(reaction)
    }

    /// Get the active reaction
    pub fn get_active_reaction(&self) -> Option<Weak<dyn AnyReaction>> {
        self.active_reaction.borrow().clone()
    }

    /// Check if there's an active reaction
    pub fn has_active_reaction(&self) -> bool {
        self.active_reaction.borrow().is_some()
    }

    /// Set the active effect, returning the previous one
    pub fn set_active_effect(
        &self,
        effect: Option<Weak<dyn AnyReaction>>,
    ) -> Option<Weak<dyn AnyReaction>> {
        self.active_effect.replace(effect)
    }

    /// Get the active effect
    pub fn get_active_effect(&self) -> Option<Weak<dyn AnyReaction>> {
        self.active_effect.borrow().clone()
    }

    /// Set untracking mode, returning previous value
    pub fn set_untracking(&self, value: bool) -> bool {
        self.untracking.replace(value)
    }

    /// Check if currently untracking
    pub fn is_untracking(&self) -> bool {
        self.untracking.get()
    }

    // =========================================================================
    // VERSION COUNTERS
    // =========================================================================

    /// Increment and return the write version
    pub fn increment_write_version(&self) -> u32 {
        let v = self.write_version.get() + 1;
        self.write_version.set(v);
        v
    }

    /// Get the current write version
    pub fn get_write_version(&self) -> u32 {
        self.write_version.get()
    }

    /// Increment and return the read version
    pub fn increment_read_version(&self) -> u32 {
        let v = self.read_version.get() + 1;
        self.read_version.set(v);
        v
    }

    /// Get the current read version
    pub fn get_read_version(&self) -> u32 {
        self.read_version.get()
    }

    // =========================================================================
    // DEPENDENCY TRACKING
    // =========================================================================

    /// Swap out the new_deps list, returning the old one
    pub fn swap_new_deps(&self, deps: Vec<Rc<dyn AnySource>>) -> Vec<Rc<dyn AnySource>> {
        self.new_deps.replace(deps)
    }

    /// Add a dependency to the new_deps list
    pub fn add_new_dep(&self, source: Rc<dyn AnySource>) {
        self.new_deps.borrow_mut().push(source);
    }

    /// Get the number of new deps collected
    pub fn new_dep_count(&self) -> usize {
        self.new_deps.borrow().len()
    }

    /// Set skipped_deps count, returning previous
    pub fn set_skipped_deps(&self, count: usize) -> usize {
        self.skipped_deps.replace(count)
    }

    /// Get skipped_deps count
    pub fn get_skipped_deps(&self) -> usize {
        self.skipped_deps.get()
    }

    /// Increment skipped_deps
    pub fn increment_skipped_deps(&self) {
        self.skipped_deps.set(self.skipped_deps.get() + 1);
    }

    /// Add an untracked write
    pub fn add_untracked_write(&self, source: Rc<dyn AnySource>) {
        self.untracked_writes.borrow_mut().push(source);
    }

    /// Clear untracked writes, returning them
    pub fn take_untracked_writes(&self) -> Vec<Rc<dyn AnySource>> {
        self.untracked_writes.replace(Vec::new())
    }

    // =========================================================================
    // BATCHING
    // =========================================================================

    /// Increment batch depth, returns new depth
    pub fn enter_batch(&self) -> u32 {
        let depth = self.batch_depth.get() + 1;
        self.batch_depth.set(depth);
        depth
    }

    /// Decrement batch depth, returns new depth
    pub fn exit_batch(&self) -> u32 {
        let depth = self.batch_depth.get().saturating_sub(1);
        self.batch_depth.set(depth);
        depth
    }

    /// Get current batch depth
    pub fn get_batch_depth(&self) -> u32 {
        self.batch_depth.get()
    }

    /// Check if currently in a batch
    pub fn is_batching(&self) -> bool {
        self.batch_depth.get() > 0
    }

    /// Add a pending reaction to run after batch
    pub fn add_pending_reaction(&self, reaction: Weak<dyn AnyReaction>) {
        self.pending_reactions.borrow_mut().push(reaction);
    }

    /// Take all pending reactions
    pub fn take_pending_reactions(&self) -> Vec<Weak<dyn AnyReaction>> {
        self.pending_reactions.replace(Vec::new())
    }

    /// Add a queued root effect
    pub fn add_queued_root_effect(&self, effect: Weak<dyn AnyReaction>) {
        self.queued_root_effects.borrow_mut().push(effect);
    }

    /// Take all queued root effects
    pub fn take_queued_root_effects(&self) -> Vec<Weak<dyn AnyReaction>> {
        self.queued_root_effects.replace(Vec::new())
    }

    /// Set flushing sync mode, returning previous
    pub fn set_flushing_sync(&self, value: bool) -> bool {
        self.is_flushing_sync.replace(value)
    }

    /// Check if currently flushing synchronously
    pub fn is_flushing_sync(&self) -> bool {
        self.is_flushing_sync.get()
    }
}

impl Default for ReactiveContext {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// THREAD-LOCAL ACCESS
// =============================================================================

thread_local! {
    /// The thread-local reactive context
    static CONTEXT: ReactiveContext = ReactiveContext::new();
}

/// Access the thread-local reactive context.
///
/// # Example
///
/// ```ignore
/// with_context(|ctx| {
///     ctx.increment_write_version();
/// });
/// ```
pub fn with_context<R>(f: impl FnOnce(&ReactiveContext) -> R) -> R {
    CONTEXT.with(f)
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================
//
// These provide direct access to common operations without needing
// to go through with_context for every call.
// =============================================================================

/// Check if currently tracking dependencies (inside a reaction, not untracking)
pub fn is_tracking() -> bool {
    with_context(|ctx| ctx.has_active_reaction() && !ctx.is_untracking())
}

/// Check if currently untracking
pub fn is_untracking() -> bool {
    with_context(|ctx| ctx.is_untracking())
}

/// Check if currently in a batch
pub fn is_batching() -> bool {
    with_context(|ctx| ctx.is_batching())
}

/// Get the current write version
pub fn write_version() -> u32 {
    with_context(|ctx| ctx.get_write_version())
}

/// Get the current read version
pub fn read_version() -> u32 {
    with_context(|ctx| ctx.get_read_version())
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_creation() {
        with_context(|ctx| {
            assert_eq!(ctx.get_write_version(), 1);
            assert_eq!(ctx.get_read_version(), 0);
            assert!(!ctx.has_active_reaction());
            assert!(!ctx.is_untracking());
            assert_eq!(ctx.get_batch_depth(), 0);
        });
    }

    #[test]
    fn version_counters() {
        with_context(|ctx| {
            assert_eq!(ctx.get_write_version(), 1);
            assert_eq!(ctx.increment_write_version(), 2);
            assert_eq!(ctx.increment_write_version(), 3);
            assert_eq!(ctx.get_write_version(), 3);

            assert_eq!(ctx.get_read_version(), 0);
            assert_eq!(ctx.increment_read_version(), 1);
            assert_eq!(ctx.get_read_version(), 1);
        });
    }

    #[test]
    fn batch_depth() {
        with_context(|ctx| {
            assert_eq!(ctx.get_batch_depth(), 0);
            assert!(!ctx.is_batching());

            assert_eq!(ctx.enter_batch(), 1);
            assert!(ctx.is_batching());

            assert_eq!(ctx.enter_batch(), 2);
            assert!(ctx.is_batching());

            assert_eq!(ctx.exit_batch(), 1);
            assert!(ctx.is_batching());

            assert_eq!(ctx.exit_batch(), 0);
            assert!(!ctx.is_batching());
        });
    }

    #[test]
    fn untracking_flag() {
        with_context(|ctx| {
            assert!(!ctx.is_untracking());

            let prev = ctx.set_untracking(true);
            assert!(!prev);
            assert!(ctx.is_untracking());

            let prev = ctx.set_untracking(false);
            assert!(prev);
            assert!(!ctx.is_untracking());
        });
    }

    #[test]
    fn skipped_deps_counter() {
        with_context(|ctx| {
            assert_eq!(ctx.get_skipped_deps(), 0);

            ctx.increment_skipped_deps();
            assert_eq!(ctx.get_skipped_deps(), 1);

            ctx.increment_skipped_deps();
            assert_eq!(ctx.get_skipped_deps(), 2);

            let prev = ctx.set_skipped_deps(0);
            assert_eq!(prev, 2);
            assert_eq!(ctx.get_skipped_deps(), 0);
        });
    }

    #[test]
    fn convenience_functions() {
        // Not tracking when no active reaction
        assert!(!is_tracking());
        assert!(!is_untracking());
        assert!(!is_batching());

        // Write version starts at 1
        assert_eq!(write_version(), 1);
        assert_eq!(read_version(), 0);
    }

    #[test]
    fn flushing_sync_flag() {
        with_context(|ctx| {
            assert!(!ctx.is_flushing_sync());

            let prev = ctx.set_flushing_sync(true);
            assert!(!prev);
            assert!(ctx.is_flushing_sync());

            let prev = ctx.set_flushing_sync(false);
            assert!(prev);
            assert!(!ctx.is_flushing_sync());
        });
    }
}
