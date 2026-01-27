// ============================================================================
// spark-signals - Reactive Shared Arrays
//
// Arrays backed by shared memory (SharedArrayBuffer) that integrate with
// the reactive system. Designed for zero-copy FFI bridge between Rust and
// TypeScript.
//
// Key features:
// - Direct pointer access to shared memory (no copying)
// - Per-index dirty tracking for sparse updates
// - Cross-platform wait mechanism (futex on Linux, ulock on macOS)
// - Full integration with reactive tracking
// ============================================================================

pub mod notify;
pub mod shared_slot_buffer;

use std::marker::PhantomData;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

// =============================================================================
// CROSS-PLATFORM WAIT
// =============================================================================

/// Wait for the wake flag to become non-zero.
///
/// Uses platform-specific primitives:
/// - Linux: futex_wait
/// - macOS: __ulock_wait
/// - Windows: WaitOnAddress (not yet implemented)
///
/// Returns immediately if the flag is already non-zero.
pub fn wait_for_wake(wake_flag: &AtomicI32) {
    loop {
        // Check if flag is set
        let value = wake_flag.load(Ordering::SeqCst);
        if value != 0 {
            // Reset flag and return
            wake_flag.store(0, Ordering::SeqCst);
            return;
        }

        // Wait for notification
        platform_wait(wake_flag, 0);
    }
}

/// Wait with timeout (in microseconds). Returns true if woken, false if timeout.
pub fn wait_for_wake_timeout(wake_flag: &AtomicI32, timeout_us: u32) -> bool {
    let value = wake_flag.load(Ordering::SeqCst);
    if value != 0 {
        wake_flag.store(0, Ordering::SeqCst);
        return true;
    }

    platform_wait_timeout(wake_flag, 0, timeout_us);

    let value = wake_flag.load(Ordering::SeqCst);
    if value != 0 {
        wake_flag.store(0, Ordering::SeqCst);
        true
    } else {
        false
    }
}

#[cfg(target_os = "linux")]
fn platform_wait(flag: &AtomicI32, expected: i32) {
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            flag as *const AtomicI32,
            libc::FUTEX_WAIT,
            expected,
            std::ptr::null::<libc::timespec>(),
        );
    }
}

#[cfg(target_os = "linux")]
fn platform_wait_timeout(flag: &AtomicI32, expected: i32, timeout_us: u32) {
    let timeout = libc::timespec {
        tv_sec: (timeout_us / 1_000_000) as i64,
        tv_nsec: ((timeout_us % 1_000_000) * 1000) as i64,
    };
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            flag as *const AtomicI32,
            libc::FUTEX_WAIT,
            expected,
            &timeout as *const libc::timespec,
        );
    }
}

#[cfg(target_os = "macos")]
fn platform_wait(flag: &AtomicI32, expected: i32) {
    // macOS uses __ulock_wait
    // UL_COMPARE_AND_WAIT = 1
    unsafe extern "C" {
        fn __ulock_wait(operation: u32, addr: *const AtomicI32, value: u64, timeout: u32) -> i32;
    }
    unsafe {
        __ulock_wait(1, flag, expected as u64, 0);
    }
}

#[cfg(target_os = "macos")]
fn platform_wait_timeout(flag: &AtomicI32, expected: i32, timeout_us: u32) {
    unsafe extern "C" {
        fn __ulock_wait(operation: u32, addr: *const AtomicI32, value: u64, timeout: u32) -> i32;
    }
    unsafe {
        __ulock_wait(1, flag, expected as u64, timeout_us);
    }
}

#[cfg(target_os = "windows")]
fn platform_wait(flag: &AtomicI32, expected: i32) {
    // Windows uses WaitOnAddress
    extern "system" {
        fn WaitOnAddress(
            address: *const AtomicI32,
            compare_address: *const i32,
            address_size: usize,
            milliseconds: u32,
        ) -> i32;
    }
    unsafe {
        WaitOnAddress(flag, &expected, std::mem::size_of::<i32>(), u32::MAX);
    }
}

#[cfg(target_os = "windows")]
fn platform_wait_timeout(flag: &AtomicI32, expected: i32, timeout_us: u32) {
    extern "system" {
        fn WaitOnAddress(
            address: *const AtomicI32,
            compare_address: *const i32,
            address_size: usize,
            milliseconds: u32,
        ) -> i32;
    }
    let timeout_ms = timeout_us / 1000;
    unsafe {
        WaitOnAddress(flag, &expected, std::mem::size_of::<i32>(), timeout_ms);
    }
}

// Fallback for other platforms (busy wait - not recommended for production)
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_wait(_flag: &AtomicI32, _expected: i32) {
    std::thread::sleep(std::time::Duration::from_micros(100));
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_wait_timeout(_flag: &AtomicI32, _expected: i32, timeout_us: u32) {
    std::thread::sleep(std::time::Duration::from_micros(timeout_us as u64));
}

// =============================================================================
// SHARED BUFFER CONTEXT
// =============================================================================

/// Context for a shared buffer - holds pointers to shared memory regions.
pub struct SharedBufferContext {
    /// Base pointer to the shared memory
    pub base_ptr: *mut u8,
    /// Total size of the shared buffer
    pub size: usize,
    /// Pointer to dirty flags (one byte per index)
    pub dirty_flags: *mut u8,
    /// Pointer to wake flag (AtomicI32)
    pub wake_flag: *const AtomicI32,
    /// Maximum number of elements
    pub max_elements: usize,
}

impl SharedBufferContext {
    /// Create a new context from raw pointers.
    ///
    /// # Safety
    ///
    /// - `base_ptr` must point to valid shared memory
    /// - All offsets must be within the buffer bounds
    /// - The memory must remain valid for the lifetime of this context
    pub unsafe fn new(
        base_ptr: *mut u8,
        size: usize,
        dirty_flags_offset: usize,
        wake_flag_offset: usize,
        max_elements: usize,
    ) -> Self {
        unsafe {
            Self {
                base_ptr,
                size,
                dirty_flags: base_ptr.add(dirty_flags_offset),
                wake_flag: base_ptr.add(wake_flag_offset) as *const AtomicI32,
                max_elements,
            }
        }
    }

    /// Get the wake flag reference for waiting.
    pub fn wake_flag(&self) -> &AtomicI32 {
        unsafe { &*self.wake_flag }
    }

    /// Check if an index is marked dirty.
    #[inline]
    pub fn is_dirty(&self, index: usize) -> bool {
        debug_assert!(index < self.max_elements);
        unsafe { *self.dirty_flags.add(index) != 0 }
    }

    /// Clear the dirty flag for an index.
    #[inline]
    pub fn clear_dirty(&self, index: usize) {
        debug_assert!(index < self.max_elements);
        unsafe {
            *self.dirty_flags.add(index) = 0;
        }
    }

    /// Get all dirty indices.
    pub fn dirty_indices(&self) -> Vec<usize> {
        (0..self.max_elements)
            .filter(|&i| self.is_dirty(i))
            .collect()
    }

    /// Clear all dirty flags.
    pub fn clear_all_dirty(&self) {
        unsafe {
            std::ptr::write_bytes(self.dirty_flags, 0, self.max_elements);
        }
    }
}

// Safety: The shared memory is synchronized via atomics
unsafe impl Send for SharedBufferContext {}
unsafe impl Sync for SharedBufferContext {}

// =============================================================================
// REACTIVE SHARED ARRAY
// =============================================================================

/// A reactive array backed by shared memory.
///
/// Reads from this array can trigger reactive subscriptions.
/// The array is read-only from Rust's perspective - writes come from the
/// TypeScript side.
///
/// # Type Parameters
///
/// - `T`: The element type (must be Copy for safe shared memory access)
pub struct ReactiveSharedArray<T: Copy> {
    ptr: *const T,
    len: usize,
    dirty: *const u8,
    /// Signal version for coarse-grained change detection
    version: AtomicU32,
    _marker: PhantomData<T>,
}

// Safety: The shared memory is synchronized via atomics
unsafe impl<T: Copy + Send> Send for ReactiveSharedArray<T> {}
unsafe impl<T: Copy + Sync> Sync for ReactiveSharedArray<T> {}

impl<T: Copy> ReactiveSharedArray<T> {
    /// Create a new reactive shared array.
    ///
    /// # Safety
    ///
    /// - `ptr` must point to valid shared memory with at least `len * size_of::<T>()` bytes
    /// - `dirty` must point to valid shared memory with at least `len` bytes
    /// - Both pointers must remain valid for the lifetime of this array
    pub unsafe fn new(ptr: *const T, len: usize, dirty: *const u8) -> Self {
        Self {
            ptr,
            len,
            dirty,
            version: AtomicU32::new(0),
            _marker: PhantomData,
        }
    }

    /// Create from a SharedBufferContext with byte offset.
    ///
    /// # Safety
    ///
    /// - The offset must be properly aligned for type T
    /// - The region must not overlap with other mutable regions
    pub unsafe fn from_context(ctx: &SharedBufferContext, byte_offset: usize, len: usize) -> Self {
        unsafe {
            let ptr = ctx.base_ptr.add(byte_offset) as *const T;
            Self::new(ptr, len, ctx.dirty_flags)
        }
    }

    /// Get a value at the given index.
    #[inline]
    pub fn get(&self, index: usize) -> T {
        debug_assert!(index < self.len, "index out of bounds");
        unsafe { *self.ptr.add(index) }
    }

    /// Check if an index is marked dirty.
    #[inline]
    pub fn is_dirty(&self, index: usize) -> bool {
        debug_assert!(index < self.len);
        unsafe { *self.dirty.add(index) != 0 }
    }

    /// Clear the dirty flag for an index.
    #[inline]
    pub fn clear_dirty(&self, index: usize) {
        debug_assert!(index < self.len);
        unsafe {
            let dirty_ptr = self.dirty as *mut u8;
            *dirty_ptr.add(index) = 0;
        }
    }

    /// Get all dirty indices.
    pub fn dirty_indices(&self) -> Vec<usize> {
        (0..self.len).filter(|&i| self.is_dirty(i)).collect()
    }

    /// Increment version (called when processing changes).
    pub fn bump_version(&self) {
        self.version.fetch_add(1, Ordering::SeqCst);
    }

    /// Get current version.
    pub fn version(&self) -> u32 {
        self.version.load(Ordering::SeqCst)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a slice of the underlying data.
    ///
    /// # Safety
    ///
    /// The returned slice is only valid while the TypeScript side is not writing.
    /// Use only within a synchronized section.
    pub unsafe fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Iterate over all elements.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        (0..self.len).map(move |i| self.get(i))
    }
}

// =============================================================================
// MUTABLE SHARED ARRAY (for output arrays that Rust writes to)
// =============================================================================

/// A mutable array backed by shared memory.
///
/// Used for output arrays where Rust writes computed results that
/// TypeScript reads.
pub struct MutableSharedArray<T: Copy> {
    ptr: *mut T,
    len: usize,
    _marker: PhantomData<T>,
}

// Safety: The shared memory is synchronized via atomics
unsafe impl<T: Copy + Send> Send for MutableSharedArray<T> {}
unsafe impl<T: Copy + Sync> Sync for MutableSharedArray<T> {}

impl<T: Copy> MutableSharedArray<T> {
    /// Create a new mutable shared array.
    ///
    /// # Safety
    ///
    /// - `ptr` must point to valid shared memory
    /// - The memory must remain valid for the lifetime of this array
    /// - No other code should write to this memory region
    pub unsafe fn new(ptr: *mut T, len: usize) -> Self {
        Self {
            ptr,
            len,
            _marker: PhantomData,
        }
    }

    /// Create from a SharedBufferContext with byte offset.
    pub unsafe fn from_context(ctx: &SharedBufferContext, byte_offset: usize, len: usize) -> Self {
        unsafe {
            let ptr = ctx.base_ptr.add(byte_offset) as *mut T;
            Self::new(ptr, len)
        }
    }

    /// Get a value at the given index.
    #[inline]
    pub fn get(&self, index: usize) -> T {
        debug_assert!(index < self.len, "index out of bounds");
        unsafe { *self.ptr.add(index) }
    }

    /// Set a value at the given index.
    #[inline]
    pub fn set(&self, index: usize, value: T) {
        debug_assert!(index < self.len, "index out of bounds");
        unsafe {
            *self.ptr.add(index) = value;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a mutable slice of the underlying data.
    ///
    /// # Safety
    ///
    /// The returned slice is only valid while no other code is accessing this memory.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

// =============================================================================
// TYPE ALIASES
// =============================================================================

/// Reactive f32 array backed by shared memory.
pub type ReactiveSharedF32Array = ReactiveSharedArray<f32>;

/// Reactive u8 array backed by shared memory.
pub type ReactiveSharedU8Array = ReactiveSharedArray<u8>;

/// Reactive i32 array backed by shared memory.
pub type ReactiveSharedI32Array = ReactiveSharedArray<i32>;

/// Reactive u32 array backed by shared memory.
pub type ReactiveSharedU32Array = ReactiveSharedArray<u32>;

/// Mutable f32 array for output data.
pub type MutableSharedF32Array = MutableSharedArray<f32>;

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reactive_shared_array_basic() {
        // Create a simple buffer
        let mut buffer = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let mut dirty = vec![0u8; 5];

        let array = unsafe {
            ReactiveSharedArray::new(buffer.as_ptr(), buffer.len(), dirty.as_ptr())
        };

        assert_eq!(array.len(), 5);
        assert_eq!(array.get(0), 1.0);
        assert_eq!(array.get(4), 5.0);

        // Test dirty tracking
        dirty[2] = 1;
        assert!(!array.is_dirty(0));
        assert!(array.is_dirty(2));

        let dirty_indices = array.dirty_indices();
        assert_eq!(dirty_indices, vec![2]);
    }

    #[test]
    fn test_mutable_shared_array() {
        let mut buffer = vec![0.0f32; 5];

        let array = unsafe { MutableSharedArray::new(buffer.as_mut_ptr(), buffer.len()) };

        array.set(0, 10.0);
        array.set(2, 20.0);

        assert_eq!(array.get(0), 10.0);
        assert_eq!(array.get(1), 0.0);
        assert_eq!(array.get(2), 20.0);
    }

    #[test]
    fn test_version_tracking() {
        let buffer = vec![1.0f32; 5];
        let dirty = vec![0u8; 5];

        let array = unsafe {
            ReactiveSharedArray::new(buffer.as_ptr(), buffer.len(), dirty.as_ptr())
        };

        assert_eq!(array.version(), 0);
        array.bump_version();
        assert_eq!(array.version(), 1);
        array.bump_version();
        assert_eq!(array.version(), 2);
    }
}
