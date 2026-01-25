// ============================================================================
// spark-signals - Core Module
// Fundamental types, traits, and context for the reactive system
// ============================================================================

pub mod constants;
pub mod context;
pub mod types;

// Re-export commonly used items
pub use constants::*;
pub use context::{is_batching, is_tracking, is_untracking, read_version, with_context, write_version, ReactiveContext};
pub use types::{default_equals, AnyReaction, AnySource, EqualsFn, SourceInner};
