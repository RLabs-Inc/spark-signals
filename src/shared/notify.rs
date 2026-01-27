// ============================================================================
// spark-signals - Notifier
//
// Pluggable cross-side notification mechanism.
// Counterpart to the TypeScript Notifier interface.
// ============================================================================

use std::sync::atomic::{AtomicI32, Ordering};

// =============================================================================
// NOTIFIER TRAIT
// =============================================================================

/// A notification mechanism for cross-side communication.
///
/// When a SharedSlotBuffer is written to, the notifier is called to inform
/// the other side (e.g., TypeScript) that changes are pending.
pub trait Notifier: 'static {
    /// Notify the other side that changes are pending.
    fn notify(&self);
}

// =============================================================================
// ATOMICS NOTIFIER
// =============================================================================

/// Notifier that sets a wake flag using atomic store.
///
/// The TypeScript side uses `Atomics.wait` on this flag.
/// We set it to 1 and call platform_wake to unblock the waiter.
pub struct AtomicsNotifier {
    wake_flag: *const AtomicI32,
}

impl AtomicsNotifier {
    /// Create a new AtomicsNotifier.
    ///
    /// # Safety
    ///
    /// `wake_flag` must point to valid shared memory for the lifetime of this notifier.
    pub unsafe fn new(wake_flag: *const AtomicI32) -> Self {
        Self { wake_flag }
    }
}

impl Notifier for AtomicsNotifier {
    fn notify(&self) {
        let flag = unsafe { &*self.wake_flag };
        flag.store(1, Ordering::SeqCst);
        platform_wake(flag);
    }
}

// Safety: The AtomicI32 is in shared memory and synchronized via atomics
unsafe impl Send for AtomicsNotifier {}
unsafe impl Sync for AtomicsNotifier {}

// =============================================================================
// NOOP NOTIFIER
// =============================================================================

/// A no-op notifier for testing or local-only usage.
pub struct NoopNotifier;

impl Notifier for NoopNotifier {
    fn notify(&self) {
        // intentionally empty
    }
}

// =============================================================================
// PLATFORM WAKE
// =============================================================================

/// Wake a thread waiting on the given atomic flag.
///
/// Counterpart to `platform_wait()` in shared/mod.rs.
///
/// Uses platform-specific primitives:
/// - Linux: futex_wake
/// - macOS: __ulock_wake
/// - Windows: WakeByAddressSingle
#[cfg(target_os = "linux")]
pub fn platform_wake(flag: &AtomicI32) {
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            flag as *const AtomicI32,
            libc::FUTEX_WAKE,
            1i32, // wake one waiter
        );
    }
}

#[cfg(target_os = "macos")]
pub fn platform_wake(flag: &AtomicI32) {
    // macOS uses __ulock_wake
    // UL_COMPARE_AND_WAIT = 1
    unsafe extern "C" {
        fn __ulock_wake(operation: u32, addr: *const AtomicI32, wake_value: u64) -> i32;
    }
    unsafe {
        __ulock_wake(1, flag, 0);
    }
}

#[cfg(target_os = "windows")]
pub fn platform_wake(flag: &AtomicI32) {
    extern "system" {
        fn WakeByAddressSingle(address: *const AtomicI32);
    }
    unsafe {
        WakeByAddressSingle(flag);
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn platform_wake(_flag: &AtomicI32) {
    // Fallback: no-op. The waiter will poll.
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_notifier_does_nothing() {
        let notifier = NoopNotifier;
        notifier.notify(); // should not panic
    }

    #[test]
    fn atomics_notifier_sets_flag() {
        let flag = AtomicI32::new(0);
        let notifier = unsafe { AtomicsNotifier::new(&flag) };

        assert_eq!(flag.load(Ordering::SeqCst), 0);
        notifier.notify();
        assert_eq!(flag.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn platform_wake_does_not_panic() {
        let flag = AtomicI32::new(0);
        platform_wake(&flag); // should not panic even with no waiters
    }
}
