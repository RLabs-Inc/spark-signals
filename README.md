# Spark Signals ⚡️

> A standalone reactive signals library for Rust. Fine-grained reactivity, zero-overhead, and TypeScript-like ergonomics.

[![Crates.io](https://img.shields.io/crates/v/spark-signals.svg)](https://crates.io/crates/spark-signals)
[![Documentation](https://docs.rs/spark-signals/badge.svg)](https://docs.rs/spark-signals)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Spark Signals** is a port of the high-performance [alien-signals](https://github.com/stackblitz/alien-signals) library to Rust. It solves the "hard problems" of Rust reactivity—type erasure, circular dependencies, and borrow checking—while providing an API that feels like writing TypeScript.

## Features

-   **⚡️ Blazing Fast:** Benchmarked at ~4ns reads, ~22ns writes. Optimized for game engines and TUIs.
-   **🧠 TypeScript-like Ergonomics:** `derived!`, `effect!`, and `prop!` macros make Rust feel like a scripting language.
-   **🔄 Deep Reactivity:** `TrackedSlotArray` and `TrackedSlot` for fine-grained ECS and layout optimization.
-   **🛡️ Memory Safe:** Automatic dependency tracking and cleanup with zero unsafe code in the hot path.
-   **🔌 Framework Agnostic:** Use it for UI, games, state management, or backend logic.

## Installation

```toml
[dependencies]
spark-signals = "0.1.2"
```

## The "Pure Magic" Syntax

Spark Signals provides macros that handle `Rc` cloning and closure moving for you. Just list your dependencies and write code.

### Signals & Deriveds

```rust
use spark_signals::{signal, derived};

fn main() {
    let width = signal(10);
    let height = signal(20);

    // "Derived depends on width and height"
    // The macro handles cloning 'width' and 'height' for the closure
    let area = derived!(width, height => width.get() * height.get());

    println!("Area: {}", area.get()); // 200

    width.set(5);
    println!("Area: {}", area.get()); // 100
}
```

### Effects

Side effects that run automatically when dependencies change.

```rust
use spark_signals::{signal, effect};

let count = signal(0);

// "Effect reads count"
effect!(count => {
    println!("Count changed to: {}", count.get());
});

count.set(1); // Prints: Count changed to: 1
```

### Props (for Components)

Create getters that capture signals effortlessly.

```rust
use spark_signals::{signal, prop, reactive_prop};

let first = signal("Sherlock");
let last = signal("Holmes");

// Create a prop getter
let full_name_prop = prop!(first, last => format!("{} {}", first.get(), last.get()));

// Convert to derived for uniform access
let full_name = reactive_prop(full_name_prop);

println!("{}", full_name.get()); // Sherlock Holmes
```

## Advanced Primitives

### Slots & Binding

`Slot<T>` is a stable reference that can switch between static values, signals, or getters. Perfect for component inputs that might change source type at runtime.

```rust
use spark_signals::{slot, signal, PropValue};

let s = slot::<i32>(None);
let sig = signal(42);

// Bind to a signal
s.bind(PropValue::from_signal(&sig));
assert_eq!(s.get(), Some(42));

// Bind to a static value
s.bind(PropValue::Static(100));
assert_eq!(s.get(), Some(100));
```

### Tracked Slots (Optimization)

`TrackedSlot` automatically reports changes to a shared `DirtySet`. This is critical for optimizing layout engines (like Taffy) or ECS systems where you only want to process changed items.

```rust
use spark_signals::{tracked_slot, dirty_set};

let dirty = dirty_set();
// Slot ID 0 reports to 'dirty' set on change
let width = tracked_slot(Some(10), dirty.clone(), 0);

width.set_value(20);

assert!(dirty.borrow().contains(&0)); // We know ID 0 changed!
```

## Architecture

This library implements the **"Push-Pull"** reactivity model:
1.  **Push:** When a signal changes, it marks dependents as `DIRTY` or `MAYBE_DIRTY`.
2.  **Pull:** When a derived is read, it re-executes *only if* its dependencies are dirty.

It uses a **"Father State"** pattern (inspired by ECS) where data lives in parallel arrays or stable slots, minimizing object allocation and pointer chasing.

## License

MIT
