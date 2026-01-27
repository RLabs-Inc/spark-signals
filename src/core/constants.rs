// ============================================================================
// spark-signals - Constants
// Flag constants for signal states, ported from TypeScript implementation
// ============================================================================

// =============================================================================
// SIGNAL TYPE FLAGS
// =============================================================================

/// Source signal (basic reactive value)
pub const SOURCE: u32 = 1 << 0;

/// Signal is a derived value (computed)
pub const DERIVED: u32 = 1 << 1;

/// Signal is an effect
pub const EFFECT: u32 = 1 << 2;

/// Effect is a render effect - runs before DOM updates
pub const RENDER_EFFECT: u32 = 1 << 3;

/// Effect is a root effect (created via effect.root())
pub const ROOT_EFFECT: u32 = 1 << 4;

/// Effect is a branch effect (if/each blocks)
pub const BRANCH_EFFECT: u32 = 1 << 5;

/// Effect is a user effect
pub const USER_EFFECT: u32 = 1 << 6;

/// Effect is a block effect
pub const BLOCK_EFFECT: u32 = 1 << 7;

// =============================================================================
// DERIVED-SPECIFIC FLAGS
// =============================================================================

/// Derived has no owner (created outside effect)
pub const UNOWNED: u32 = 1 << 8;

/// Derived is disconnected (no reactions, can be GC'd)
pub const DISCONNECTED: u32 = 1 << 9;

// =============================================================================
// SIGNAL STATE FLAGS
// =============================================================================

/// Signal/reaction is clean (up-to-date)
pub const CLEAN: u32 = 1 << 10;

/// Signal/reaction is dirty (definitely needs update)
pub const DIRTY: u32 = 1 << 11;

/// Signal/reaction might be dirty (needs to check dependencies)
pub const MAYBE_DIRTY: u32 = 1 << 12;

/// Reaction is currently being updated
pub const REACTION_IS_UPDATING: u32 = 1 << 13;

/// Effect has been destroyed
pub const DESTROYED: u32 = 1 << 14;

/// Effect is inert (paused)
pub const INERT: u32 = 1 << 15;

/// Effect has run at least once
pub const EFFECT_RAN: u32 = 1 << 16;

/// Effect is preserved (not destroyed with parent)
pub const EFFECT_PRESERVED: u32 = 1 << 17;

// =============================================================================
// INSPECT FLAGS (for debugging)
// =============================================================================

/// Effect is an inspect effect (for debugging)
pub const INSPECT_EFFECT: u32 = 1 << 18;

/// Reaction is a repeater (inline write-through forwarding node)
pub const REPEATER: u32 = 1 << 19;

// =============================================================================
// STATUS MASK (for clearing status bits)
// =============================================================================

/// Mask to clear all status bits (CLEAN, DIRTY, MAYBE_DIRTY)
pub const STATUS_MASK: u32 = !(DIRTY | MAYBE_DIRTY | CLEAN);

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_distinct() {
        // Ensure no flags overlap
        let all_flags = [
            SOURCE,
            DERIVED,
            EFFECT,
            RENDER_EFFECT,
            ROOT_EFFECT,
            BRANCH_EFFECT,
            USER_EFFECT,
            BLOCK_EFFECT,
            UNOWNED,
            DISCONNECTED,
            CLEAN,
            DIRTY,
            MAYBE_DIRTY,
            REACTION_IS_UPDATING,
            DESTROYED,
            INERT,
            EFFECT_RAN,
            EFFECT_PRESERVED,
            INSPECT_EFFECT,
            REPEATER,
        ];

        for (i, &a) in all_flags.iter().enumerate() {
            for (j, &b) in all_flags.iter().enumerate() {
                if i != j {
                    assert_eq!(
                        a & b,
                        0,
                        "Flags at index {} and {} overlap: {:b} & {:b}",
                        i,
                        j,
                        a,
                        b
                    );
                }
            }
        }
    }

    #[test]
    fn status_mask_clears_status_bits() {
        let flags = DERIVED | DIRTY | EFFECT_RAN;
        let cleared = flags & STATUS_MASK;

        // Should clear DIRTY but keep DERIVED and EFFECT_RAN
        assert_eq!(cleared & DIRTY, 0);
        assert_ne!(cleared & DERIVED, 0);
        assert_ne!(cleared & EFFECT_RAN, 0);
    }

    #[test]
    fn can_combine_flags() {
        let derived_dirty = DERIVED | DIRTY;
        assert_ne!(derived_dirty & DERIVED, 0);
        assert_ne!(derived_dirty & DIRTY, 0);
        assert_eq!(derived_dirty & EFFECT, 0);
    }

    #[test]
    fn can_check_and_modify_flags() {
        let mut flags = SOURCE | CLEAN;

        // Check flags
        assert_ne!(flags & SOURCE, 0);
        assert_ne!(flags & CLEAN, 0);
        assert_eq!(flags & DIRTY, 0);

        // Clear CLEAN, set DIRTY
        flags = (flags & STATUS_MASK) | DIRTY;

        assert_ne!(flags & SOURCE, 0);
        assert_eq!(flags & CLEAN, 0);
        assert_ne!(flags & DIRTY, 0);
    }
}
