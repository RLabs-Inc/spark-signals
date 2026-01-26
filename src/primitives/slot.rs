// ============================================================================
// spark-signals - Reactive Slot
//
// A Slot is a reactive cell that can point to different sources.
// Unlike bind(), a Slot is STABLE - you mutate its source, not replace it.
//
// This solves the "binding replacement" problem where deriveds track
// the binding object itself, missing updates when it's replaced.
//
// Think of it as "bind() on steroids" with bulletproof tracking.
// ============================================================================

use std::cell::{Cell, RefCell};
use std::fmt::Debug;
use std::rc::Rc;

use crate::core::constants::*;
use crate::core::types::{AnySource, SourceInner};
use crate::primitives::signal::Signal;
use crate::primitives::props::PropValue;
use crate::reactivity::tracking::{mark_reactions, notify_write, track_read};

// =============================================================================
// SOURCE TYPE CONSTANTS
// =============================================================================

const SOURCE_STATIC: u8 = 0; // Holds a static value
const SOURCE_SIGNAL: u8 = 1; // Points to a signal/source
const SOURCE_GETTER: u8 = 2; // Points to a getter function

// =============================================================================
// SLOT INNER
// =============================================================================

/// Internal slot state.
///
/// A slot has its own SourceInner for tracking purposes, plus
/// a reference to what it actually points to.
struct SlotInner<T: Clone + PartialEq + 'static> {
    /// The internal source for version tracking (what deriveds track)
    /// Stores Option<T> to support uninitialized state
    source: Rc<SourceInner<Option<T>>>,

    /// Source type: static, signal, or getter
    source_type: Cell<u8>,

    /// The actual signal reference (for write-through)
    signal_ref: RefCell<Option<Signal<T>>>,

    /// Getter function
    getter: RefCell<Option<Box<dyn Fn() -> T>>>,
}

impl<T: Clone + PartialEq + 'static> SlotInner<T> {
    /// Create a new slot with an optional initial value
    fn new(initial: Option<T>) -> Self {
        Self {
            source: Rc::new(SourceInner::new(initial)),
            source_type: Cell::new(SOURCE_STATIC),
            signal_ref: RefCell::new(None),
            getter: RefCell::new(None),
        }
    }

    /// Read the current value with tracking
    fn get(&self) -> Option<T> {
        // Track the slot itself
        track_read(self.source.clone() as Rc<dyn AnySource>);

        match self.source_type.get() {
            SOURCE_STATIC => self.source.get(),
            SOURCE_SIGNAL => {
                // Read through to signal - this creates dependency on the signal too!
                if let Some(ref sig) = *self.signal_ref.borrow() {
                    track_read(sig.as_any_source());
                    Some(sig.get())
                } else {
                    self.source.get()
                }
            }
            SOURCE_GETTER => {
                // Call getter - dependencies tracked inside the getter
                if let Some(ref getter) = *self.getter.borrow() {
                    Some(getter())
                } else {
                    self.source.get()
                }
            }
            _ => self.source.get(),
        }
    }

    /// Read without tracking (peek)
    fn peek(&self) -> Option<T> {
        match self.source_type.get() {
            SOURCE_STATIC => self.source.get(),
            SOURCE_SIGNAL => {
                if let Some(ref sig) = *self.signal_ref.borrow() {
                    // Use inner().get() to read without tracking
                    Some(sig.inner().get())
                } else {
                    self.source.get()
                }
            }
            SOURCE_GETTER => {
                if let Some(ref getter) = *self.getter.borrow() {
                    Some(getter())
                } else {
                    self.source.get()
                }
            }
            _ => self.source.get(),
        }
    }

    /// Set a static value as the source
    fn set_static(&self, value: T) {
        self.source_type.set(SOURCE_STATIC);
        *self.signal_ref.borrow_mut() = None;
        *self.getter.borrow_mut() = None;
        self.source.set(Some(value));

        // Notify dependents
        notify_write(self.source.clone() as Rc<dyn AnySource>);
    }

    /// Set a signal as the source
    fn set_signal(&self, signal: Signal<T>) {
        self.source_type.set(SOURCE_SIGNAL);
        *self.signal_ref.borrow_mut() = Some(signal);
        *self.getter.borrow_mut() = None;

        // Notify dependents that source changed
        self.notify_source_changed();
    }

    /// Set a getter function as the source
    fn set_getter(&self, getter: Box<dyn Fn() -> T>) {
        self.source_type.set(SOURCE_GETTER);
        *self.signal_ref.borrow_mut() = None;
        *self.getter.borrow_mut() = Some(getter);

        // Notify dependents that source changed
        self.notify_source_changed();
    }

    /// Write a value (writes through if pointing to writable source)
    fn set(&self, value: T) -> Result<(), SlotWriteError> {
        match self.source_type.get() {
            SOURCE_STATIC => {
                // Update static value
                if self.source.set(Some(value)) {
                    notify_write(self.source.clone() as Rc<dyn AnySource>);
                }
                Ok(())
            }
            SOURCE_SIGNAL => {
                // Write through to signal
                if let Some(ref sig) = *self.signal_ref.borrow() {
                    sig.set(value);
                    Ok(())
                } else {
                    Err(SlotWriteError::NoSource)
                }
            }
            SOURCE_GETTER => {
                // Can't write to getter
                Err(SlotWriteError::ReadOnlyGetter)
            }
            _ => Err(SlotWriteError::NoSource),
        }
    }

    /// Notify dependents that the slot's source reference changed
    fn notify_source_changed(&self) {
        // Increment write version and mark reactions dirty
        let new_version = self.source.write_version() + 1;
        self.source.set_write_version(new_version);
        mark_reactions(self.source.clone() as Rc<dyn AnySource>, DIRTY);
    }

    /// Clear the slot
    fn clear(&self) {
        self.source_type.set(SOURCE_STATIC);
        *self.signal_ref.borrow_mut() = None;
        *self.getter.borrow_mut() = None;
        self.source.set(None);
        notify_write(self.source.clone() as Rc<dyn AnySource>);
    }
}

// =============================================================================
// SLOT WRITE ERROR
// =============================================================================

/// Error returned when writing to a slot fails
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotWriteError {
    /// Slot is pointing to a getter function (read-only)
    ReadOnlyGetter,
    /// Slot has no source configured
    NoSource,
}

impl std::fmt::Display for SlotWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotWriteError::ReadOnlyGetter => {
                write!(f, "Cannot write to a slot pointing to a getter function")
            }
            SlotWriteError::NoSource => write!(f, "Slot has no source configured"),
        }
    }
}

impl std::error::Error for SlotWriteError {}

// =============================================================================
// SLOT
// =============================================================================

/// A reactive slot that can hold any value or point to a reactive source.
///
/// Key features:
/// - Stable identity (never replaced, only mutated)
/// - Tracks BOTH the slot version AND underlying source
/// - Supports two-way binding (read and write through)
///
/// # Example
///
/// ```
/// use spark_signals::{signal, slot};
///
/// let my_slot = slot::<String>(Some("initial".into()));
///
/// // Read value
/// assert_eq!(my_slot.get(), Some("initial".into()));
///
/// // Set static value
/// my_slot.set_value("hello".into());
/// assert_eq!(my_slot.get(), Some("hello".into()));
///
/// // Point to a signal
/// let name = signal("world".to_string());
/// my_slot.set_signal(&name);
/// assert_eq!(my_slot.get(), Some("world".to_string()));
///
/// // Write through to signal
/// my_slot.set("universe".into()).unwrap();
/// assert_eq!(name.get(), "universe".to_string());
/// ```
pub struct Slot<T: Clone + PartialEq + 'static> {
    inner: Rc<SlotInner<T>>,
}

impl<T: Clone + PartialEq + 'static> Slot<T> {
    /// Read the current value with dependency tracking.
    ///
    /// When called inside a reactive context (effect/derived), creates
    /// a dependency on both the slot AND the underlying source.
    pub fn get(&self) -> Option<T> {
        self.inner.get()
    }

    /// Read without tracking (peek).
    ///
    /// Returns the current value without creating any dependencies.
    pub fn peek(&self) -> Option<T> {
        self.inner.peek()
    }

    /// Set a static value as the slot's source.
    ///
    /// This clears any signal or getter reference and stores the value directly.
    pub fn set_value(&self, value: T) {
        self.inner.set_static(value);
    }

    /// Point the slot to a signal.
    ///
    /// Reading the slot will read through to the signal.
    /// Writing to the slot will write through to the signal.
    pub fn set_signal(&self, signal: &Signal<T>) {
        self.inner.set_signal(signal.clone());
    }

    /// Point the slot to a getter function.
    ///
    /// Reading the slot will call the getter (with dependency tracking inside).
    /// Writing to the slot will fail with `SlotWriteError::ReadOnlyGetter`.
    pub fn set_getter<F: Fn() -> T + 'static>(&self, getter: F) {
        self.inner.set_getter(Box::new(getter));
    }

    /// Bind a PropValue to the slot.
    ///
    /// This is the primary way to connect component props to FlexNode slots.
    /// It automatically handles static values, signals, and getters.
    pub fn bind(&self, prop: PropValue<T>) {
        match prop {
            PropValue::Static(v) => self.set_value(v),
            PropValue::Signal(s) => self.set_signal(&s),
            PropValue::Getter(g) => self.set_getter(move || g()),
        }
    }

    /// Write a value to the slot's source.
    ///
    /// - If pointing to a static value: updates the static value
    /// - If pointing to a signal: writes through to the signal
    /// - If pointing to a getter: returns `Err(SlotWriteError::ReadOnlyGetter)`
    pub fn set(&self, value: T) -> Result<(), SlotWriteError> {
        self.inner.set(value)
    }

    /// Clear the slot (reset to None for static value).
    pub fn clear(&self) {
        self.inner.clear();
    }

    /// Check if the slot is pointing to a signal
    pub fn is_signal(&self) -> bool {
        self.inner.source_type.get() == SOURCE_SIGNAL
    }

    /// Check if the slot is pointing to a getter
    pub fn is_getter(&self) -> bool {
        self.inner.source_type.get() == SOURCE_GETTER
    }

    /// Check if the slot is holding a static value
    pub fn is_static(&self) -> bool {
        self.inner.source_type.get() == SOURCE_STATIC
    }
}

impl<T: Clone + PartialEq + 'static> Clone for Slot<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Clone + PartialEq + Debug + 'static> Debug for Slot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source_type = match self.inner.source_type.get() {
            SOURCE_STATIC => "static",
            SOURCE_SIGNAL => "signal",
            SOURCE_GETTER => "getter",
            _ => "unknown",
        };
        f.debug_struct("Slot")
            .field("type", &source_type)
            .field("value", &self.peek())
            .finish()
    }
}

// =============================================================================
// SLOT CONSTRUCTOR
// =============================================================================

/// Create a reactive slot.
///
/// A Slot is a stable reactive cell that can point to different sources:
/// - Static values (primitives or objects)
/// - Signals (one-way or two-way binding)
/// - Getter functions (computed, auto-tracks)
///
/// When you read `slot.get()`:
/// 1. Tracks the slot itself (notified when source changes)
/// 2. Tracks through to the underlying source (notified when value changes)
///
/// # Arguments
///
/// * `initial` - Optional initial value
///
/// # Example
///
/// ```
/// use spark_signals::slot;
///
/// // Create with static value
/// let text = slot(Some("hello".to_string()));
/// assert_eq!(text.get(), Some("hello".to_string()));
///
/// // Create empty
/// let empty: spark_signals::primitives::slot::Slot<i32> = slot(None);
/// assert_eq!(empty.get(), None);
/// ```
pub fn slot<T: Clone + PartialEq + 'static>(initial: Option<T>) -> Slot<T> {
    Slot {
        inner: Rc::new(SlotInner::new(initial)),
    }
}

/// Create a slot initialized with a value (convenience).
///
/// Equivalent to `slot(Some(value))`.
pub fn slot_with_value<T: Clone + PartialEq + 'static>(value: T) -> Slot<T> {
    slot(Some(value))
}

// =============================================================================
// TRACKED SLOT
// =============================================================================

/// A Slot that automatically reports changes to a dirty set.
///
/// This is useful for optimizing expensive computations (like layout) where
/// you only want to process items that have actually changed.
///
/// # Example
///
/// ```
/// use spark_signals::{tracked_slot, dirty_set, slot};
///
/// let dirty = dirty_set();
/// let width = tracked_slot(Some(10), dirty.clone(), 0);
///
/// width.set_value(20);
/// assert!(dirty.borrow().contains(&0));
/// ```
pub struct TrackedSlot<T: Clone + PartialEq + 'static> {
    inner: Slot<T>,
    dirty: DirtySet,
    id: usize,
}

impl<T: Clone + PartialEq + 'static> TrackedSlot<T> {
    /// Read the current value with dependency tracking.
    pub fn get(&self) -> Option<T> {
        self.inner.get()
    }

    /// Read without tracking (peek).
    pub fn peek(&self) -> Option<T> {
        self.inner.peek()
    }

    /// Set a static value (marks id as dirty).
    pub fn set_value(&self, value: T) {
        self.inner.set_value(value);
        self.dirty.borrow_mut().insert(self.id);
    }

    /// Point to a signal (marks id as dirty).
    pub fn set_signal(&self, signal: &Signal<T>) {
        self.inner.set_signal(signal);
        self.dirty.borrow_mut().insert(self.id);
    }

    /// Point to a getter (marks id as dirty).
    pub fn set_getter<F: Fn() -> T + 'static>(&self, getter: F) {
        self.inner.set_getter(getter);
        self.dirty.borrow_mut().insert(self.id);
    }

    /// Bind a PropValue (marks id as dirty).
    pub fn bind(&self, prop: PropValue<T>) {
        self.inner.bind(prop);
        self.dirty.borrow_mut().insert(self.id);
    }

    /// Write a value (marks id as dirty).
    pub fn set(&self, value: T) -> Result<(), SlotWriteError> {
        let result = self.inner.set(value);
        if result.is_ok() {
            self.dirty.borrow_mut().insert(self.id);
        }
        result
    }

    /// Clear the slot (marks id as dirty).
    pub fn clear(&self) {
        self.inner.clear();
        self.dirty.borrow_mut().insert(self.id);
    }
}

impl<T: Clone + PartialEq + 'static> Clone for TrackedSlot<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            dirty: self.dirty.clone(),
            id: self.id,
        }
    }
}

impl<T: Clone + PartialEq + Debug + 'static> Debug for TrackedSlot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackedSlot")
            .field("id", &self.id)
            .field("value", &self.peek())
            .finish()
    }
}

/// Create a tracked slot.
pub fn tracked_slot<T: Clone + PartialEq + 'static>(
    initial: Option<T>,
    dirty: DirtySet,
    id: usize,
) -> TrackedSlot<T> {
    TrackedSlot {
        inner: slot(initial),
        dirty,
        id,
    }
}

// =============================================================================
// SLOT ARRAY
// =============================================================================

/// A growable array of slots with convenient access patterns.
///
/// Features:
/// - Auto-expands when accessing indices
/// - Type-safe slot access
/// - Iteration support
///
/// # Example
///
/// ```
/// use spark_signals::{signal, slot_array};
///
/// let texts = slot_array::<String>(None);
///
/// // Set values at indices (auto-expands)
/// texts.set_value(0, "first".into());
/// texts.set_value(1, "second".into());
///
/// assert_eq!(texts.get(0), Some("first".into()));
/// assert_eq!(texts.get(1), Some("second".into()));
/// assert_eq!(texts.len(), 2);
///
/// // Point slot to a signal
/// let dynamic = signal("dynamic".to_string());
/// texts.set_signal(2, &dynamic);
/// assert_eq!(texts.get(2), Some("dynamic".to_string()));
/// ```
pub struct SlotArray<T: Clone + PartialEq + 'static> {
    slots: RefCell<Vec<Slot<T>>>,
    default_value: Option<T>,
}

impl<T: Clone + PartialEq + 'static> SlotArray<T> {
    /// Get the number of slots
    pub fn len(&self) -> usize {
        self.slots.borrow().len()
    }

    /// Check if the array is empty
    pub fn is_empty(&self) -> bool {
        self.slots.borrow().is_empty()
    }

    /// Ensure capacity for at least n slots
    pub fn ensure_capacity(&self, n: usize) {
        let mut slots = self.slots.borrow_mut();
        while slots.len() < n {
            slots.push(slot(self.default_value.clone()));
        }
    }

    /// Get value at index (auto-expands, with tracking)
    pub fn get(&self, index: usize) -> Option<T> {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].get()
    }

    /// Peek value at index (auto-expands, without tracking)
    pub fn peek(&self, index: usize) -> Option<T> {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].peek()
    }

    /// Set a static value at index
    pub fn set_value(&self, index: usize, value: T) {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].set_value(value);
    }

    /// Point slot at index to a signal
    pub fn set_signal(&self, index: usize, signal: &Signal<T>) {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].set_signal(signal);
    }

    /// Point slot at index to a getter
    pub fn set_getter<F: Fn() -> T + 'static>(&self, index: usize, getter: F) {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].set_getter(getter);
    }

    /// Write through to slot at index
    pub fn set(&self, index: usize, value: T) -> Result<(), SlotWriteError> {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].set(value)
    }

    /// Get the raw slot at index
    pub fn slot(&self, index: usize) -> Slot<T> {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].clone()
    }

    /// Clear slot at index (reset to default)
    pub fn clear(&self, index: usize) {
        if index < self.len() {
            let slot = &self.slots.borrow()[index];
            if let Some(ref default) = self.default_value {
                slot.set_value(default.clone());
            } else {
                slot.clear();
            }
        }
    }

    /// Check if a slot exists at the given index
    pub fn has(&self, index: usize) -> bool {
        index < self.len()
    }

    /// Bind a PropValue to the slot at the given index.
    pub fn bind(&self, index: usize, prop: PropValue<T>) {
        self.ensure_capacity(index + 1);
        self.slots.borrow()[index].bind(prop);
    }
}

impl<T: Clone + PartialEq + Debug + 'static> Debug for SlotArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotArray")
            .field("len", &self.len())
            .field("default", &self.default_value)
            .finish()
    }
}

// =============================================================================
// SLOT ARRAY CONSTRUCTOR
// =============================================================================

/// Create a reactive slot array.
///
/// A SlotArray provides array-like access to slots with automatic
/// expansion and convenient methods for setting sources and values.
///
/// # Arguments
///
/// * `default_value` - Default value for new slots
///
/// # Example
///
/// ```
/// use spark_signals::slot_array;
///
/// let numbers = slot_array::<i32>(Some(0));
///
/// // Auto-expands when accessing
/// numbers.set_value(5, 42);
/// assert_eq!(numbers.len(), 6); // Indices 0-5 created
/// assert_eq!(numbers.get(5), Some(42));
/// assert_eq!(numbers.get(0), Some(0)); // Default value
/// ```
pub fn slot_array<T: Clone + PartialEq + 'static>(default_value: Option<T>) -> SlotArray<T> {
    SlotArray {
        slots: RefCell::new(Vec::new()),
        default_value,
    }
}

// =============================================================================
// TRACKED SLOT ARRAY
// =============================================================================

use std::collections::HashSet;

/// Shared dirty set for TrackedSlotArray - uses interior mutability for sharing.
pub type DirtySet = Rc<RefCell<HashSet<usize>>>;

/// Create a new shared dirty set.
pub fn dirty_set() -> DirtySet {
    Rc::new(RefCell::new(HashSet::new()))
}

/// A SlotArray that automatically tracks which indices have been modified.
///
/// When `set_value()`, `set_signal()`, `set_getter()`, or `set()` is called,
/// the index is automatically added to the provided dirty set. This enables
/// incremental computation patterns where deriveds can check the dirty set
/// to skip unchanged work.
///
/// # Example
///
/// ```
/// use spark_signals::{tracked_slot_array, dirty_set};
///
/// let dirty_indices = dirty_set();
/// let values = tracked_slot_array::<i32>(Some(0), dirty_indices.clone());
///
/// // Setting a value marks the index as dirty
/// values.set_value(5, 42);
/// assert!(dirty_indices.borrow().contains(&5));
///
/// // Process dirty indices and clear
/// for idx in dirty_indices.borrow().iter() {
///     println!("Index {} was modified", idx);
/// }
/// dirty_indices.borrow_mut().clear();
/// ```
pub struct TrackedSlotArray<T: Clone + PartialEq + 'static> {
    inner: SlotArray<T>,
    dirty: DirtySet,
}

impl<T: Clone + PartialEq + 'static> TrackedSlotArray<T> {
    /// Get the number of slots
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the array is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Ensure capacity for at least n slots
    pub fn ensure_capacity(&self, n: usize) {
        self.inner.ensure_capacity(n);
    }

    /// Get value at index (auto-expands, with tracking)
    pub fn get(&self, index: usize) -> Option<T> {
        self.inner.get(index)
    }

    /// Peek value at index (auto-expands, without tracking)
    pub fn peek(&self, index: usize) -> Option<T> {
        self.inner.peek(index)
    }

    /// Set a static value at index (marks index as dirty)
    pub fn set_value(&self, index: usize, value: T) {
        self.inner.set_value(index, value);
        self.dirty.borrow_mut().insert(index);
    }

    /// Point slot at index to a signal (marks index as dirty)
    pub fn set_signal(&self, index: usize, signal: &Signal<T>) {
        self.inner.set_signal(index, signal);
        self.dirty.borrow_mut().insert(index);
    }

    /// Point slot at index to a getter (marks index as dirty)
    pub fn set_getter<F: Fn() -> T + 'static>(&self, index: usize, getter: F) {
        self.inner.set_getter(index, getter);
        self.dirty.borrow_mut().insert(index);
    }

    /// Write through to slot at index (marks index as dirty)
    pub fn set(&self, index: usize, value: T) -> Result<(), SlotWriteError> {
        let result = self.inner.set(index, value);
        if result.is_ok() {
            self.dirty.borrow_mut().insert(index);
        }
        result
    }

    /// Get the raw slot at index
    pub fn slot(&self, index: usize) -> Slot<T> {
        self.inner.slot(index)
    }

    /// Clear slot at index (marks index as dirty)
    pub fn clear(&self, index: usize) {
        let was_present = index < self.len();
        self.inner.clear(index);
        if was_present {
            self.dirty.borrow_mut().insert(index);
        }
    }

    /// Check if a slot exists at the given index
    pub fn has(&self, index: usize) -> bool {
        self.inner.has(index)
    }

    /// Bind a PropValue to the slot at the given index (marks index as dirty).
    pub fn bind(&self, index: usize, prop: PropValue<T>) {
        self.inner.bind(index, prop);
        self.dirty.borrow_mut().insert(index);
    }

    /// Get the dirty set for manual inspection/clearing
    pub fn dirty(&self) -> &DirtySet {
        &self.dirty
    }

    /// Get the inner SlotArray (for advanced use)
    pub fn inner(&self) -> &SlotArray<T> {
        &self.inner
    }
}

impl<T: Clone + PartialEq + Debug + 'static> Debug for TrackedSlotArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackedSlotArray")
            .field("len", &self.len())
            .field("dirty_count", &self.dirty.borrow().len())
            .finish()
    }
}

/// Create a tracked slot array with dirty index tracking.
///
/// A TrackedSlotArray is a SlotArray that automatically records which
/// indices have been modified to a shared dirty set. This enables efficient
/// incremental computations where only changed indices need processing.
///
/// **Note:** The dirty set is NOT automatically cleared. The consumer is responsible
/// for clearing the dirty set after processing changes (usually via `dirty.borrow_mut().clear()`).
///
/// # Arguments
///
/// * `default_value` - Default value for new slots
/// * `dirty` - Shared dirty set (use `dirty_set()` to create one)
///
/// # Example
///
/// ```
/// use spark_signals::{tracked_slot_array, dirty_set, derived};
///
/// let dirty = dirty_set();
/// let values = tracked_slot_array::<i32>(Some(0), dirty.clone());
///
/// // Modifications automatically track dirty indices
/// values.set_value(0, 10);
/// values.set_value(5, 42);
///
/// // Check dirty indices
/// assert!(dirty.borrow().contains(&0));
/// assert!(dirty.borrow().contains(&5));
///
/// // Clear after processing
/// dirty.borrow_mut().clear();
/// ```
pub fn tracked_slot_array<T: Clone + PartialEq + 'static>(
    default_value: Option<T>,
    dirty: DirtySet,
) -> TrackedSlotArray<T> {
    TrackedSlotArray {
        inner: slot_array(default_value),
        dirty,
    }
}

// =============================================================================
// IS SLOT
// =============================================================================

/// Check if a value is a Slot.
///
/// This is a marker trait for slot identification.
pub trait IsSlot {}

impl<T: Clone + PartialEq + 'static> IsSlot for Slot<T> {}

/// Check if a value implements IsSlot
pub fn is_slot<T: IsSlot>(_: &T) -> bool {
    true
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::derived::derived;
    use crate::primitives::effect::effect_sync;
    use crate::primitives::signal::signal;
    use std::cell::Cell;

    // =========================================================================
    // PHASE 8 SUCCESS CRITERIA TESTS (SLOTS)
    // =========================================================================

    #[test]
    fn phase8_criteria_5_slot_creates_typed_storage() {
        // slot<T>() creates typed storage slot
        let text_slot = slot::<String>(Some("hello".into()));

        assert_eq!(text_slot.get(), Some("hello".to_string()));
        assert!(text_slot.is_static());
    }

    #[test]
    fn phase8_criteria_6_slot_array_creates_growable_array() {
        // slotArray<T>() creates growable slot array
        let numbers = slot_array::<i32>(Some(0));

        // Auto-expands
        numbers.set_value(5, 42);
        assert_eq!(numbers.len(), 6);

        // Reads work
        assert_eq!(numbers.get(5), Some(42));
        assert_eq!(numbers.get(0), Some(0)); // Default
    }

    // =========================================================================
    // SLOT UNIT TESTS
    // =========================================================================

    #[test]
    fn slot_static_value() {
        let s = slot(Some(42));

        assert_eq!(s.get(), Some(42));
        assert!(s.is_static());

        s.set_value(100);
        assert_eq!(s.get(), Some(100));
    }

    #[test]
    fn slot_points_to_signal() {
        let sig = signal(42);
        let s = slot::<i32>(None);

        s.set_signal(&sig);
        assert!(s.is_signal());
        assert_eq!(s.get(), Some(42));

        // Signal changes, slot sees it
        sig.set(100);
        assert_eq!(s.get(), Some(100));
    }

    #[test]
    fn slot_write_through_to_signal() {
        let sig = signal(42);
        let s = slot::<i32>(None);

        s.set_signal(&sig);

        // Write through slot
        s.set(100).unwrap();

        // Signal was updated
        assert_eq!(sig.get(), 100);
    }

    #[test]
    fn slot_getter_readonly() {
        let count = signal(5);
        let count_clone = count.clone();

        let s = slot::<i32>(None);
        s.set_getter(move || count_clone.get() * 2);

        assert!(s.is_getter());
        assert_eq!(s.get(), Some(10));

        // Getter changes with source
        count.set(10);
        assert_eq!(s.get(), Some(20));

        // Can't write to getter
        let result = s.set(42);
        assert_eq!(result, Err(SlotWriteError::ReadOnlyGetter));
    }

    #[test]
    fn slot_peek_no_tracking() {
        let s = slot(Some(42));

        // Peek outside any reactive context
        assert_eq!(s.peek(), Some(42));
    }

    #[test]
    fn slot_clear() {
        let s = slot(Some(42));

        s.clear();
        assert_eq!(s.get(), None);
        assert!(s.is_static());
    }

    #[test]
    fn slot_clone_shares_inner() {
        let s1 = slot(Some(42));
        let s2 = s1.clone();

        s1.set_value(100);
        assert_eq!(s2.get(), Some(100));
    }

    #[test]
    fn slot_creates_dependency() {
        let s = slot(Some(0));
        let s_clone = s.clone();

        let run_count = Rc::new(Cell::new(0));
        let run_clone = run_count.clone();

        let _dispose = effect_sync(move || {
            let _ = s_clone.get();
            run_clone.set(run_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        // Changing slot triggers effect
        s.set_value(42);
        assert_eq!(run_count.get(), 2);
    }

    #[test]
    fn slot_signal_creates_dual_dependency() {
        let sig = signal(0);
        let s = slot::<i32>(None);
        s.set_signal(&sig);

        let s_clone = s.clone();

        let run_count = Rc::new(Cell::new(0));
        let run_clone = run_count.clone();

        let _dispose = effect_sync(move || {
            let _ = s_clone.get();
            run_clone.set(run_clone.get() + 1);
        });

        assert_eq!(run_count.get(), 1);

        // Changing signal triggers effect (through slot)
        sig.set(42);
        assert_eq!(run_count.get(), 2);

        // Changing slot source triggers effect
        let new_sig = signal(100);
        s.set_signal(&new_sig);
        assert_eq!(run_count.get(), 3);
    }

    #[test]
    fn slot_in_derived() {
        let s = slot(Some(10));
        let s_clone = s.clone();

        let doubled = derived(move || s_clone.get().unwrap_or(0) * 2);

        assert_eq!(doubled.get(), 20);

        s.set_value(5);
        assert_eq!(doubled.get(), 10);
    }

    #[test]
    fn tracked_slot_basic() {
        let dirty = dirty_set();
        let ts = tracked_slot(Some(10), dirty.clone(), 5);

        assert_eq!(ts.get(), Some(10));
        assert!(dirty.borrow().is_empty());

        ts.set_value(20);
        assert!(dirty.borrow().contains(&5));
        assert_eq!(ts.get(), Some(20));
    }

    #[test]
    fn tracked_slot_bind() {
        use crate::primitives::props::PropValue;
        let dirty = dirty_set();
        let ts = tracked_slot::<i32>(None, dirty.clone(), 1);

        ts.bind(PropValue::Static(42));
        assert!(dirty.borrow().contains(&1));
        assert_eq!(ts.get(), Some(42));
    }

    // =========================================================================
    // SLOT ARRAY TESTS
    // =========================================================================

    #[test]
    fn slot_array_basic() {
        let arr = slot_array::<i32>(Some(0));

        assert!(arr.is_empty());

        arr.set_value(0, 10);
        arr.set_value(1, 20);

        assert_eq!(arr.len(), 2);
        assert_eq!(arr.get(0), Some(10));
        assert_eq!(arr.get(1), Some(20));
    }

    #[test]
    fn slot_array_auto_expand() {
        let arr = slot_array::<i32>(Some(-1));

        // Accessing index 5 creates slots 0-5
        arr.set_value(5, 42);

        assert_eq!(arr.len(), 6);
        assert_eq!(arr.get(0), Some(-1)); // Default
        assert_eq!(arr.get(5), Some(42));
    }

    #[test]
    fn slot_array_signal() {
        let arr = slot_array::<i32>(None);
        let sig = signal(100);

        arr.set_signal(0, &sig);

        assert_eq!(arr.get(0), Some(100));

        sig.set(200);
        assert_eq!(arr.get(0), Some(200));
    }

    #[test]
    fn slot_array_getter() {
        let arr = slot_array::<i32>(None);
        let base = signal(5);
        let base_clone = base.clone();

        arr.set_getter(0, move || base_clone.get() * 3);

        assert_eq!(arr.get(0), Some(15));

        base.set(10);
        assert_eq!(arr.get(0), Some(30));
    }

    #[test]
    fn slot_array_write_through() {
        let arr = slot_array::<i32>(None);
        let sig = signal(100);

        arr.set_signal(0, &sig);
        arr.set(0, 200).unwrap();

        assert_eq!(sig.get(), 200);
    }

    #[test]
    fn slot_array_clear() {
        let arr = slot_array::<i32>(Some(0));

        arr.set_value(0, 42);
        assert_eq!(arr.get(0), Some(42));

        arr.clear(0);
        assert_eq!(arr.get(0), Some(0)); // Back to default
    }

    #[test]
    fn slot_array_has() {
        let arr = slot_array::<i32>(None);

        assert!(!arr.has(0));

        arr.set_value(0, 42);
        assert!(arr.has(0));
        assert!(!arr.has(1));
    }

    #[test]
    fn slot_array_get_raw_slot() {
        let arr = slot_array::<i32>(Some(0));

        arr.set_value(0, 42);

        let raw_slot = arr.slot(0);
        assert_eq!(raw_slot.get(), Some(42));

        // Modifying raw slot affects array
        raw_slot.set_value(100);
        assert_eq!(arr.get(0), Some(100));
    }

    // =========================================================================
    // TRACKED SLOT ARRAY TESTS
    // =========================================================================

    #[test]
    fn tracked_slot_array_tracks_set_value() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(Some(0), dirty.clone());

        assert!(dirty.borrow().is_empty());

        arr.set_value(5, 42);

        assert!(dirty.borrow().contains(&5));
        assert_eq!(dirty.borrow().len(), 1);
    }

    #[test]
    fn tracked_slot_array_tracks_multiple_indices() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(Some(0), dirty.clone());

        arr.set_value(0, 10);
        arr.set_value(3, 30);
        arr.set_value(7, 70);

        assert_eq!(dirty.borrow().len(), 3);
        assert!(dirty.borrow().contains(&0));
        assert!(dirty.borrow().contains(&3));
        assert!(dirty.borrow().contains(&7));
    }

    #[test]
    fn tracked_slot_array_tracks_signal() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(None, dirty.clone());
        let sig = signal(100);

        arr.set_signal(2, &sig);

        assert!(dirty.borrow().contains(&2));
    }

    #[test]
    fn tracked_slot_array_tracks_set_write_through() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(None, dirty.clone());
        let sig = signal(100);

        arr.set_signal(0, &sig);
        dirty.borrow_mut().clear(); // Clear from set_signal

        // Write through
        arr.set(0, 200).unwrap();

        assert!(dirty.borrow().contains(&0));
        assert_eq!(sig.get(), 200);
    }

    #[test]
    fn tracked_slot_array_tracks_clear() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(Some(0), dirty.clone());

        arr.set_value(0, 42);
        dirty.borrow_mut().clear();

        arr.clear(0);

        assert!(dirty.borrow().contains(&0));
    }

    #[test]
    fn tracked_slot_array_get_no_tracking() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(Some(0), dirty.clone());

        // Reading doesn't mark dirty
        let _ = arr.get(0);
        let _ = arr.peek(0);

        assert!(dirty.borrow().is_empty());
    }

    #[test]
    fn tracked_slot_array_with_derived_incremental_pattern() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(Some(0), dirty.clone());

        // Initial data
        arr.set_value(0, 10);
        arr.set_value(1, 20);
        arr.set_value(2, 30);

        // Simulate processing dirty indices
        let dirty_indices: Vec<usize> = dirty.borrow().iter().copied().collect();
        assert_eq!(dirty_indices.len(), 3);

        // Clear after processing
        dirty.borrow_mut().clear();
        assert!(dirty.borrow().is_empty());

        // Only modify one index
        arr.set_value(1, 25);

        // Only that index is dirty
        let dirty_indices: Vec<usize> = dirty.borrow().iter().copied().collect();
        assert_eq!(dirty_indices, vec![1]);
    }

    #[test]
    fn tracked_slot_array_duplicate_set_same_index() {
        let dirty = dirty_set();
        let arr = tracked_slot_array::<i32>(Some(0), dirty.clone());

        // Set same index multiple times
        arr.set_value(0, 10);
        arr.set_value(0, 20);
        arr.set_value(0, 30);

        // HashSet deduplicates
        assert_eq!(dirty.borrow().len(), 1);
        assert!(dirty.borrow().contains(&0));
    }
}
