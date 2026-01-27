// ============================================================================
// spark-signals - Primitives Module
// Core reactive primitives: signal, derived, effect, bind, linked, scope
// ============================================================================

pub mod bind;
pub mod derived;
pub mod effect;
pub mod linked;
pub mod props;
pub mod repeater;
pub mod scope;
pub mod selector;
pub mod signal;
pub mod slot;

// Re-export for convenience
pub use bind::{
    bind, bind_chain, bind_getter, bind_readonly, bind_readonly_from, bind_readonly_static,
    bind_static, bind_value, binding_has_internal_source, disconnect_binding, disconnect_source,
    is_binding, unwrap_binding, unwrap_readonly, Binding, IsBinding, ReadonlyBinding,
};
pub use derived::{derived, derived_with_equals, Derived, DerivedInner};
pub use effect::{
    destroy_effect, update_effect, CleanupFn, DisposeFn, Effect, EffectFn, EffectInner,
};
pub use linked::{
    is_linked_signal, linked_signal, linked_signal_full, linked_signal_with_options,
    IsLinkedSignal, LinkedSignal, LinkedSignalOptionsSimple, PreviousValue,
};
pub use scope::{
    effect_scope, get_current_scope, on_scope_dispose, register_effect_with_scope, EffectScope,
    ScopeCleanupFn,
};
pub use signal::{signal, signal_with_equals, source, Signal, SourceOptions};
pub use slot::{
    is_slot, slot, slot_array, slot_with_value, tracked_slot, IsSlot, Slot, SlotArray,
    SlotWriteError, TrackedSlot,
};
