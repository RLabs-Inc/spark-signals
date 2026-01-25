# spark-signals Memory & Lifecycle Fixes

**Created:** January 24, 2026
**Status:** COMPLETED
**Total Issues:** 25
**Completed:** 25/25

---

## Quick Stats

| Severity | Count | Completed |
|----------|-------|-----------|
| 🔴 CRITICAL | 4 | 4/4 |
| 🟠 HIGH | 5 | 5/5 |
| ⚠️ MEDIUM | 11 | 11/11 |
| 🟢 LOW | 5 | 5/5 |

---

## Phase 1: Critical Drop Implementations (BLOCKING)

✅ **ALL COMPLETED**

### 1.1 EffectInner Missing Drop
- [x] **Status:** COMPLETED
- **Fix:** Implemented `Drop` to run `teardown`.

### 1.2 EffectScopeInner Missing Drop
- [x] **Status:** COMPLETED
- **Fix:** Implemented `Drop` to call `stop()`.

### 1.3 EffectScope Missing Drop
- [x] **Status:** COMPLETED
- **Fix:** Implemented `Drop` to call `stop()` on last strong reference.

### 1.4 Selector Weak ptr_eq Bug (Quadratic Growth)
- [x] **Status:** COMPLETED
- **Fix:** Changed `PartialEq` to use `Weak::ptr_eq` instead of upgrading.

---

## Phase 2: High Priority Drop Implementations

✅ **ALL COMPLETED**

### 2.1 Selector Missing Drop
- [x] **Status:** COMPLETED
- **Fix:** Implemented `Drop` to call `dispose` on last strong reference (Fixed cloning bug in Session 54).

### 2.2 LinkedSignal Missing Drop
- [x] **Status:** COMPLETED
- **Fix:** Implemented `Drop` to call `dispose` on last strong reference.

### 2.3 Effect Wrapper Missing Drop
- [x] **Status:** COMPLETED
- **Fix:** Implemented `Drop` to call `dispose` on last strong reference.

### 2.4 cleanup_dead_reactions() Never Called
- [x] **Status:** COMPLETED
- **Fix:** Added `cleanup_dead_reactions()` call in `mark_reactions` (iterative cleanup).

### 2.5 Scope.stop() Should Flush Pending Effects
- [x] **Status:** COMPLETED
- **Fix:** Added `flush_sync()` call at start of `stop()`.

---

## Phase 3: Medium Priority Fixes

✅ **ALL COMPLETED**

### 3.1 Hollow Test: flush_sync_runs_pending_effects
- [x] **Status:** COMPLETED

### 3.2 Hollow Test: max_flush_count_prevents_infinite_loop
- [x] **Status:** COMPLETED

### 3.3 Hollow Test: schedule_effect_in_batch_defers_execution
- [x] **Status:** COMPLETED

### 3.4 Derived Deduplication Gap
- [x] **Status:** COMPLETED (Verified: `update_derived_chain` handles version-based deduplication correctly)

### 3.5 Derived as Parent Issue
- [x] **Status:** COMPLETED
- **Fix:** Restored `active_effect` context correctly in `update_effect`. Derived never becomes `active_effect`.

### 3.6 ReactiveMap remove() Leaks Key Signals
- [x] **Status:** COMPLETED

### 3.7 ReactiveSet remove() Leaks Item Signals
- [x] **Status:** COMPLETED

### 3.8 Selector Subscribers HashMap Cleanup
- [x] **Status:** COMPLETED

### 3.9 Effect Cleanup - Clear Parent Reference
- [x] **Status:** COMPLETED

### 3.10 TrackedSlotArray Dirty Set Not Cleared
- [x] **Status:** COMPLETED (Documented as intentional behavior)

### 3.11 Port selector+derived Test from TypeScript
- [x] **Status:** COMPLETED

---

## Phase 4: Low Priority (Nice to Have)

✅ **ALL COMPLETED**

### 4.1 ReactiveVec Index Signal Accumulation
- [x] **Status:** COMPLETED
- **Fix:** Added `index_signals.remove()` in `pop`, `truncate`, and `clear`.

### 4.2 Pending Reactions Not Cleaned on Destroy
- [x] **Status:** COMPLETED
- **Fix:** Mitigated by `update_effect` checking `DESTROYED` flag. No manual cleanup needed.

### 4.3 Derived self_ref Pattern Documentation
- [x] **Status:** COMPLETED
- **Fix:** Updated `DerivedInner` comments.

### 4.4 Replace Unwraps with Proper Error Handling
- [x] **Status:** COMPLETED
- **Fix:** Replaced unwrap in `map.rs` with safer lookup. Verified derived unwraps are panic-safe invariants.

### 4.5 Effect Tree Unlinking Race Condition
- [x] **Status:** COMPLETED
- **Fix:** Refactored `destroy_effect_children` to collect children into a Vec before destroying, avoiding list mutation during iteration.

---

## Verification Checklist

- [x] 308 tests passing (256 unit + 6 lifecycle + 46 doc tests)
- [x] No memory growth in basic create/destroy cycles (verified via `lifecycle_drop` tests)
- [x] Effect cleanup callbacks run on drop
- [x] Scope.stop() runs all cleanups
- [x] Selector properly deduplicates subscribers

---

*Last updated: January 25, 2026*