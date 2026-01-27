# Changelog

All notable changes to spark-signals will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-01-27

### Added

- **`SharedSlotBuffer<T>`** - Reactive typed arrays backed by shared memory (Layer 1)
  - `get(index)` performs reactive read via `track_read()`
  - `set(index, value)` writes to shared memory + marks reactions dirty + notifies cross-side
  - `peek(index)` for non-reactive reads
  - Coarse-grained reactive source via internal `SourceInner<u32>` (version counter)
  - `notify_changed()` to trigger reactive graph propagation from external notifications
  - Optional dirty flags via `with_dirty(ptr)` builder method
  - Operates on external memory via raw pointers — zero allocation

- **`RepeaterInner`** - New reactive graph primitive (Layer 2)
  - NOT an effect, NOT a derived — a purpose-built forwarding node
  - Runs INLINE during `mark_reactions` — zero scheduling overhead
  - Implements `AnyReaction` with `REPEATER` flag
  - `forward()` — called by `mark_reactions` when REPEATER flag is detected
  - `repeat()` factory — connects a reactive source to a SharedSlotBuffer position
  - New `REPEATER = 1 << 19` flag in reactive constants

- **`Notifier` trait** - Pluggable cross-side notification (Layer 3)
  - `AtomicsNotifier` — atomic store + platform wake (`futex_wake` on Linux, `__ulock_wake` on macOS)
  - `NoopNotifier` — silent, for testing
  - `platform_wake()` — cross-platform wake implementation

### Changed

- `mark_reactions` in `tracking.rs` now has a REPEATER branch that calls `forward()` inline and marks the reaction CLEAN

## [0.2.0] - 2026-01-27

### Added

- **`ReactiveSharedArray`** / **`MutableSharedArray`** - SharedBuffer-backed reactive arrays for zero-copy FFI
  - Pointer-based read/write to external shared memory
  - Dirty flags and version tracking
  - Per-index source tracking via `get_index_source()`

## [0.1.2] - 2026-01-26

### Added

- `TrackedSlot` and `tracked_slot()` for automatic dirty tracking
- `bind()` method on Slot and arrays
- `prop!` macro for ergonomic reactive properties
- `derived!` and `effect!` macros for ultra-clean syntax
- `cloned!` macro for ergonomic closures

## [0.1.1] - 2026-01-25

### Changed

- Complete Rust rewrite of the reactive signals library

## [0.1.0] - 2026-01-24

### Added

- Initial release with core reactivity: signal, derived, effect
- Reactive collections: ReactiveVec, ReactiveMap, ReactiveSet, TrackedSlotArray
- `#[derive(Reactive)]` proc macro for deep reactivity
- Props system with PropValue
- Benchmarked at 50M signals/sec creation, 4.1ns reads on M1
