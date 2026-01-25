// ============================================================================
// spark-signals - Reactivity Module
// Core reactive tracking, dependency management, and dirty propagation
// ============================================================================

pub mod batching;
pub mod equality;
pub mod scheduling;
pub mod tracking;

// Re-export main tracking functions
pub use tracking::{
    is_dirty, mark_reactions, notify_write, remove_reactions, set_signal_status, track_read,
};

// Re-export scheduling functions
pub use scheduling::{flush_pending_reactions, flush_sync, schedule_effect_inner};

// Re-export batching functions
pub use batching::{batch, peek, tick, untrack};
