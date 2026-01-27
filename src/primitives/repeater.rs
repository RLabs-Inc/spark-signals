// ============================================================================
// spark-signals - Repeater
//
// A new reactive graph node — NOT an effect, NOT a derived.
// A purpose-built forwarding node that runs INLINE during mark_reactions.
//
// Connects any reactive source to a SharedSlotBuffer position.
// When source changes → repeater forwards value to target.
// Zero scheduling overhead.
//
// This is Layer 2 of the Cross-Language Reactive Shared Memory architecture.
// ============================================================================

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::core::constants::*;
use crate::core::types::{AnyReaction, AnySource};

// =============================================================================
// REPEATER INNER
// =============================================================================

/// Internal state of a repeater node.
///
/// Implements AnyReaction so it can be stored in a Source's reactions list.
/// The REPEATER flag causes mark_reactions to call `forward()` inline
/// instead of scheduling the reaction.
pub struct RepeaterInner {
    flags: Cell<u32>,
    deps: RefCell<Vec<Rc<dyn AnySource>>>,
    /// The function to read the current value and write it to the target.
    /// Encapsulates both the read and the write in a single closure.
    forward_fn: Box<dyn Fn()>,
}

impl RepeaterInner {
    /// Create a new repeater.
    ///
    /// `source` — the reactive source to watch (will be stored as a dep)
    /// `forward_fn` — called inline during mark_reactions to read source + write target
    pub fn new(source: Rc<dyn AnySource>, forward_fn: impl Fn() + 'static) -> Rc<Self> {
        let inner = Rc::new(Self {
            flags: Cell::new(REPEATER | CLEAN),
            deps: RefCell::new(vec![source.clone()]),
            forward_fn: Box::new(forward_fn),
        });

        // Register with source's reactions
        source.add_reaction(Rc::downgrade(&inner) as Weak<dyn AnyReaction>);

        inner
    }

    /// Execute the forward operation.
    /// Called inline during mark_reactions when this repeater is encountered.
    pub fn forward(&self) {
        if (self.flags.get() & DESTROYED) != 0 {
            return;
        }
        (self.forward_fn)();
    }
}

impl AnyReaction for RepeaterInner {
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
        // Repeaters don't use the standard update path.
        // They forward inline during mark_reactions.
        self.forward();
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_derived_source(&self) -> Option<Rc<dyn AnySource>> {
        None // Repeaters are not deriveds
    }
}

// =============================================================================
// REPEAT FACTORY
// =============================================================================

/// Create a repeater: forwards a reactive source to a target via `forward_fn`.
///
/// The `forward_fn` is called inline during `mark_reactions` whenever the source
/// changes. It should read the current value and write it to the target.
///
/// Returns a dispose function that removes the repeater from the source's reactions.
///
/// # Example
///
/// ```ignore
/// let source = signal(42.0f32);
/// let buf = SharedSlotBuffer::new(...);
/// let dispose = repeat(
///     source.as_any_source(),
///     move || { buf.set(0, source.get()); }
/// );
/// ```
pub fn repeat(
    source: Rc<dyn AnySource>,
    forward_fn: impl Fn() + 'static,
) -> Box<dyn FnOnce()> {
    let inner = RepeaterInner::new(source.clone(), forward_fn);

    // Return dispose function
    let weak = Rc::downgrade(&inner);
    Box::new(move || {
        if let Some(strong) = weak.upgrade() {
            strong.set_flags(strong.flags() | DESTROYED);
            // Remove from source's reactions
            source.remove_reaction(&(strong as Rc<dyn AnyReaction>));
        }
        // Drop the Rc — if no one else holds it, the repeater is deallocated
        drop(inner);
    })
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::SourceInner;
    use crate::reactivity::tracking::mark_reactions;
    use std::cell::Cell as StdCell;

    #[test]
    fn repeater_creation() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(42i32));

        let call_count = Rc::new(StdCell::new(0u32));
        let cc = call_count.clone();

        let dispose = repeat(source.clone(), move || {
            cc.set(cc.get() + 1);
        });

        // Source should have one reaction (the repeater)
        assert_eq!(source.reaction_count(), 1);

        // Dispose
        dispose();
        // After cleanup, source should have the reaction cleaned up
        source.cleanup_dead_reactions();
        assert_eq!(source.reaction_count(), 0);
    }

    #[test]
    fn repeater_has_correct_flags() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(0i32));

        let inner = RepeaterInner::new(source.clone(), || {});

        assert!((inner.flags() & REPEATER) != 0);
        assert!((inner.flags() & CLEAN) != 0);
        assert!((inner.flags() & EFFECT) == 0);
        assert!((inner.flags() & DERIVED) == 0);
    }

    #[test]
    fn repeater_forward_calls_fn() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(0i32));
        let called = Rc::new(StdCell::new(false));
        let c = called.clone();

        let inner = RepeaterInner::new(source, move || {
            c.set(true);
        });

        assert!(!called.get());
        inner.forward();
        assert!(called.get());
    }

    #[test]
    fn repeater_destroyed_does_not_forward() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(0i32));
        let called = Rc::new(StdCell::new(false));
        let c = called.clone();

        let inner = RepeaterInner::new(source, move || {
            c.set(true);
        });

        inner.set_flags(inner.flags() | DESTROYED);
        inner.forward();
        assert!(!called.get());
    }

    #[test]
    fn mark_reactions_triggers_repeater_inline() {
        let source: Rc<dyn AnySource> = Rc::new(SourceInner::new(0i32));
        let forwarded = Rc::new(StdCell::new(false));
        let f = forwarded.clone();

        let _inner = RepeaterInner::new(source.clone(), move || {
            f.set(true);
        });

        assert!(!forwarded.get());

        // mark_reactions should trigger the repeater inline
        mark_reactions(source.clone(), DIRTY);

        assert!(forwarded.get(), "Repeater should have been forwarded inline during mark_reactions");
    }
}
