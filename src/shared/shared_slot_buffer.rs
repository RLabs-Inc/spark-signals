// ============================================================================
// spark-signals - SharedSlotBuffer
//
// Reactive typed arrays backed by shared memory. get() tracks dependencies,
// set() writes to shared memory + notifies reactive graph + notifies cross-side.
//
// This is Layer 1 of the Cross-Language Reactive Shared Memory architecture.
// ============================================================================

use std::marker::PhantomData;
use std::rc::Rc;

use crate::core::types::{AnySource, SourceInner};
use crate::reactivity::tracking::track_read;
use crate::shared::notify::Notifier;

// =============================================================================
// SHARED SLOT BUFFER
// =============================================================================

/// A reactive typed array backed by shared memory.
///
/// - `get(index)` performs a reactive read (tracks dependency via `track_read`)
/// - `set(index, value)` writes to shared memory + marks reactions dirty + notifies
/// - `peek(index)` reads without reactive tracking
///
/// The buffer owns no allocation — it operates on external memory via raw pointers.
///
/// # Type Parameters
///
/// - `T`: Element type (must be Copy + PartialEq for equality checking)
pub struct SharedSlotBuffer<T: Copy + PartialEq + 'static> {
    ptr: *mut T,
    len: usize,
    dirty: Option<*mut u8>,
    default_value: T,
    notifier: Box<dyn Notifier>,
    /// Coarse-grained reactive source (any index changed)
    source: Rc<SourceInner<u32>>, // value is a version counter
    _marker: PhantomData<T>,
}

impl<T: Copy + PartialEq + 'static> SharedSlotBuffer<T> {
    /// Create a new SharedSlotBuffer over external memory.
    ///
    /// # Safety
    ///
    /// - `ptr` must point to valid memory with at least `len * size_of::<T>()` bytes
    /// - The memory must remain valid for the lifetime of this buffer
    /// - If `dirty` is Some, it must point to valid memory with at least `len` bytes
    pub unsafe fn new(
        ptr: *mut T,
        len: usize,
        default_value: T,
        notifier: impl Notifier,
    ) -> Self {
        Self {
            ptr,
            len,
            dirty: None,
            default_value,
            notifier: Box::new(notifier),
            source: Rc::new(SourceInner::new(0u32)),
            _marker: PhantomData,
        }
    }

    /// Create with dirty flags.
    ///
    /// # Safety
    ///
    /// Same as `new()`, plus `dirty` must point to valid memory with `len` bytes.
    pub unsafe fn with_dirty(
        ptr: *mut T,
        len: usize,
        dirty: *mut u8,
        default_value: T,
        notifier: impl Notifier,
    ) -> Self {
        Self {
            ptr,
            len,
            dirty: Some(dirty),
            default_value,
            notifier: Box::new(notifier),
            source: Rc::new(SourceInner::new(0u32)),
            _marker: PhantomData,
        }
    }

    /// Reactive read — tracks dependency via the reactive graph.
    #[inline]
    pub fn get(&self, index: usize) -> T {
        debug_assert!(index < self.len, "SharedSlotBuffer: index out of bounds");
        track_read(self.source.clone() as Rc<dyn AnySource>);
        unsafe { *self.ptr.add(index) }
    }

    /// Non-reactive read.
    #[inline]
    pub fn peek(&self, index: usize) -> T {
        debug_assert!(index < self.len, "SharedSlotBuffer: index out of bounds");
        unsafe { *self.ptr.add(index) }
    }

    /// Write + mark reactions dirty + set dirty flag + notify cross-side.
    #[inline]
    pub fn set(&self, index: usize, value: T) {
        debug_assert!(index < self.len, "SharedSlotBuffer: index out of bounds");

        let current = unsafe { *self.ptr.add(index) };
        if current == value {
            return; // equality check
        }

        // Write to shared memory
        unsafe { *self.ptr.add(index) = value; }

        // Set dirty flag
        if let Some(dirty) = self.dirty {
            unsafe { *dirty.add(index) = 1; }
        }

        // Update reactive source version
        let new_version = self.source.get() + 1;
        self.source.set(new_version);

        // Notify cross-side
        self.notifier.notify();
    }

    /// Batch write — single notification at end.
    pub fn set_batch(&self, updates: &[(usize, T)]) {
        let mut changed = false;

        for &(index, value) in updates {
            debug_assert!(index < self.len, "SharedSlotBuffer: index out of bounds");

            let current = unsafe { *self.ptr.add(index) };
            if current != value {
                unsafe { *self.ptr.add(index) = value; }
                if let Some(dirty) = self.dirty {
                    unsafe { *dirty.add(index) = 1; }
                }
                changed = true;
            }
        }

        if changed {
            let new_version = self.source.get() + 1;
            self.source.set(new_version);
            self.notifier.notify();
        }
    }

    /// Notify the Rust reactive graph that the other side changed data.
    /// Call this after waking from a cross-side notification.
    pub fn notify_changed(&self) {
        let new_version = self.source.get() + 1;
        self.source.set(new_version);
    }

    /// Get the coarse-grained reactive source (for building deriveds that depend on this buffer).
    pub fn source(&self) -> Rc<SourceInner<u32>> {
        self.source.clone()
    }

    /// Reset index to default value.
    pub fn clear(&self, index: usize) {
        self.set(index, self.default_value);
    }

    /// Get buffer length (capacity).
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::notify::NoopNotifier;

    #[test]
    fn basic_get_set() {
        let mut data = vec![0.0f32; 8];
        let buf = unsafe {
            SharedSlotBuffer::new(data.as_mut_ptr(), data.len(), 0.0, NoopNotifier)
        };

        assert_eq!(buf.peek(0), 0.0);
        buf.set(0, 42.0);
        assert_eq!(buf.peek(0), 42.0);
        assert_eq!(buf.len(), 8);
    }

    #[test]
    fn equality_check_skips_write() {
        let mut data = vec![10.0f32; 4];
        let buf = unsafe {
            SharedSlotBuffer::new(data.as_mut_ptr(), data.len(), 0.0, NoopNotifier)
        };

        // Set same value — should be a no-op
        buf.set(0, 10.0);
        // No way to directly observe the skip, but it shouldn't panic or change anything
        assert_eq!(buf.peek(0), 10.0);

        // Set different value
        buf.set(0, 20.0);
        assert_eq!(buf.peek(0), 20.0);
    }

    #[test]
    fn dirty_flags() {
        let mut data = vec![0i32; 4];
        let mut dirty = vec![0u8; 4];
        let buf = unsafe {
            SharedSlotBuffer::with_dirty(
                data.as_mut_ptr(),
                data.len(),
                dirty.as_mut_ptr(),
                0,
                NoopNotifier,
            )
        };

        assert_eq!(dirty[0], 0);
        buf.set(0, 42);
        assert_eq!(dirty[0], 1);
        assert_eq!(dirty[1], 0);

        buf.set(2, 99);
        assert_eq!(dirty[2], 1);
    }

    #[test]
    fn batch_set() {
        let mut data = vec![0u32; 8];
        let buf = unsafe {
            SharedSlotBuffer::new(data.as_mut_ptr(), data.len(), 0, NoopNotifier)
        };

        buf.set_batch(&[(0, 10), (3, 30), (7, 70)]);
        assert_eq!(buf.peek(0), 10);
        assert_eq!(buf.peek(1), 0);
        assert_eq!(buf.peek(3), 30);
        assert_eq!(buf.peek(7), 70);
    }

    #[test]
    fn clear_resets_to_default() {
        let mut data = vec![0.0f32; 4];
        let buf = unsafe {
            SharedSlotBuffer::new(data.as_mut_ptr(), data.len(), -1.0, NoopNotifier)
        };

        buf.set(0, 42.0);
        assert_eq!(buf.peek(0), 42.0);

        buf.clear(0);
        assert_eq!(buf.peek(0), -1.0);
    }
}
