// ============================================================================
// spark-signals - Reactive Collections
// Maps, Sets, and Vecs with fine-grained per-key/item/index reactivity
// ============================================================================
//
// Ports the TypeScript ReactiveMap and ReactiveSet, plus adds ReactiveVec.
// Each collection has three levels of reactivity:
//
// 1. Per-key/item/index signals: Only triggers when that specific element changes
// 2. Version signal: Triggers on structural changes (add/remove)
// 3. Size/length signal: Triggers when count changes
// ============================================================================

mod map;
mod set;
mod vec;

pub use map::ReactiveMap;
pub use set::ReactiveSet;
pub use vec::ReactiveVec;
