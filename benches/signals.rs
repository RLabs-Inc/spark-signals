//! Benchmarks for spark-signals
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use spark_signals::{signal, derived, effect, batch};

// =============================================================================
// SIGNAL BENCHMARKS
// =============================================================================

fn bench_signal_create(c: &mut Criterion) {
    c.bench_function("signal_create", |b| {
        b.iter(|| {
            black_box(signal(0i32))
        })
    });
}

fn bench_signal_get(c: &mut Criterion) {
    let s = signal(42i32);
    c.bench_function("signal_get", |b| {
        b.iter(|| {
            black_box(s.get())
        })
    });
}

fn bench_signal_set(c: &mut Criterion) {
    let s = signal(0i32);
    c.bench_function("signal_set", |b| {
        b.iter(|| {
            s.set(black_box(42))
        })
    });
}

fn bench_signal_set_same_value(c: &mut Criterion) {
    let s = signal(42i32);
    c.bench_function("signal_set_same_value", |b| {
        b.iter(|| {
            s.set(black_box(42))
        })
    });
}

// =============================================================================
// DERIVED BENCHMARKS
// =============================================================================

fn bench_derived_create(c: &mut Criterion) {
    let s = signal(0i32);
    c.bench_function("derived_create", |b| {
        let s = s.clone();
        b.iter(|| {
            black_box(derived({
                let s = s.clone();
                move || s.get() * 2
            }))
        })
    });
}

fn bench_derived_get_cached(c: &mut Criterion) {
    let s = signal(42i32);
    let s_clone = s.clone();
    let d = derived(move || s_clone.get() * 2);

    // First get to cache the value
    let _ = d.get();

    c.bench_function("derived_get_cached", |b| {
        b.iter(|| {
            black_box(d.get())
        })
    });
}

fn bench_derived_get_dirty(c: &mut Criterion) {
    let s = signal(0i32);
    let s_clone = s.clone();
    let d = derived(move || s_clone.get() * 2);

    let mut i = 0i32;
    c.bench_function("derived_get_dirty", |b| {
        b.iter(|| {
            s.set(i);
            i += 1;
            black_box(d.get())
        })
    });
}

fn bench_derived_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("derived_chain");

    for depth in [1, 5, 10, 20] {
        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &depth| {
            let s = signal(1i32);

            // Build chain of deriveds
            let mut current = {
                let s = s.clone();
                derived(move || s.get() + 1)
            };

            for _ in 1..depth {
                let prev = current.clone();
                current = derived(move || prev.get() + 1);
            }

            b.iter(|| {
                s.set(black_box(1));
                black_box(current.get())
            })
        });
    }

    group.finish();
}

// =============================================================================
// EFFECT BENCHMARKS
// =============================================================================

fn bench_effect_create(c: &mut Criterion) {
    c.bench_function("effect_create", |b| {
        b.iter(|| {
            black_box(effect(|| {}))
        })
    });
}

fn bench_effect_trigger(c: &mut Criterion) {
    let s = signal(0i32);
    let s_clone = s.clone();
    let _e = effect(move || {
        black_box(s_clone.get());
    });

    let mut i = 0i32;
    c.bench_function("effect_trigger", |b| {
        b.iter(|| {
            s.set(i);
            i += 1;
        })
    });
}

fn bench_effect_multiple_deps(c: &mut Criterion) {
    let a = signal(0i32);
    let b = signal(0i32);
    let c_sig = signal(0i32);

    let a_c = a.clone();
    let b_c = b.clone();
    let c_c = c_sig.clone();
    let _e = effect(move || {
        black_box(a_c.get() + b_c.get() + c_c.get());
    });

    let mut i = 0i32;
    c.bench_function("effect_multiple_deps", |b| {
        b.iter(|| {
            a.set(i);
            i += 1;
        })
    });
}

// =============================================================================
// BATCH BENCHMARKS
// =============================================================================

fn bench_batch_updates(c: &mut Criterion) {
    let s = signal(0i32);
    let s_clone = s.clone();
    let _e = effect(move || {
        black_box(s_clone.get());
    });

    c.bench_function("batch_10_updates", |b| {
        b.iter(|| {
            batch(|| {
                for i in 0..10 {
                    s.set(black_box(i));
                }
            })
        })
    });
}

// =============================================================================
// STRESS TESTS
// =============================================================================

fn bench_many_signals(c: &mut Criterion) {
    let mut group = c.benchmark_group("many_signals");

    for count in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("create", count), &count, |b, &count| {
            b.iter(|| {
                let signals: Vec<_> = (0..count).map(|i| signal(i)).collect();
                black_box(signals)
            })
        });
    }

    group.finish();
}

fn bench_many_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("many_effects");

    for count in [10, 100, 500] {
        group.bench_with_input(BenchmarkId::new("trigger", count), &count, |b, &count| {
            let s = signal(0i32);

            let effects: Vec<_> = (0..count).map(|_| {
                let s = s.clone();
                effect(move || { black_box(s.get()); })
            }).collect();

            let mut i = 0i32;
            b.iter(|| {
                s.set(i);
                i += 1;
            });

            drop(effects);
        });
    }

    group.finish();
}

// =============================================================================
// COMPARABLE TO TYPESCRIPT BENCHMARKS
// =============================================================================

/// Matches TypeScript: "Single write + flushSync()"
/// Signal write + effect runs immediately (our impl is sync by default)
fn bench_ts_comparable_single_write_effect(c: &mut Criterion) {
    let count = signal(0i32);
    let count_clone = count.clone();

    let _e = effect(move || {
        black_box(count_clone.get());
    });

    let mut i = 0i32;
    c.bench_function("ts_compare/single_write+effect", |b| {
        b.iter(|| {
            count.set(i);
            i += 1;
        })
    });
}

/// Matches TypeScript: "Batched (10 writes) + flushSync()"
fn bench_ts_comparable_batched_writes(c: &mut Criterion) {
    let count = signal(0i32);
    let count_clone = count.clone();

    let _e = effect(move || {
        black_box(count_clone.get());
    });

    let mut base = 0i32;
    c.bench_function("ts_compare/batched_10_writes", |b| {
        b.iter(|| {
            batch(|| {
                for i in 0..10 {
                    count.set(base + i);
                }
            });
            base += 10;
        })
    });
}

/// Matches TypeScript: "3 signals write + flushSync()"
fn bench_ts_comparable_multi_signal(c: &mut Criterion) {
    let sig_a = signal(0i32);
    let sig_b = signal(0i32);
    let sig_c = signal(0i32);

    let a_c = sig_a.clone();
    let b_c = sig_b.clone();
    let c_c = sig_c.clone();
    let _e = effect(move || {
        black_box(a_c.get() + b_c.get() + c_c.get());
    });

    let mut i = 0i32;
    c.bench_function("ts_compare/3_signals_write", |bencher| {
        bencher.iter(|| {
            batch(|| {
                sig_a.set(i);
                sig_b.set(i);
                sig_c.set(i);
            });
            i += 1;
        })
    });
}

/// Matches TypeScript: "Signal -> Derived -> Effect + flushSync()"
fn bench_ts_comparable_derived_chain(c: &mut Criterion) {
    let count = signal(0i32);
    let count_clone = count.clone();
    let doubled = derived(move || count_clone.get() * 2);

    let doubled_clone = doubled.clone();
    let _e = effect(move || {
        black_box(doubled_clone.get());
    });

    let mut i = 0i32;
    c.bench_function("ts_compare/signal_derived_effect", |b| {
        b.iter(|| {
            count.set(i);
            i += 1;
        })
    });
}

// =============================================================================
// CRITERION SETUP
// =============================================================================

criterion_group!(
    signal_benches,
    bench_signal_create,
    bench_signal_get,
    bench_signal_set,
    bench_signal_set_same_value,
);

criterion_group!(
    ts_compare_benches,
    bench_ts_comparable_single_write_effect,
    bench_ts_comparable_batched_writes,
    bench_ts_comparable_multi_signal,
    bench_ts_comparable_derived_chain,
);

criterion_group!(
    derived_benches,
    bench_derived_create,
    bench_derived_get_cached,
    bench_derived_get_dirty,
    bench_derived_chain,
);

criterion_group!(
    effect_benches,
    bench_effect_create,
    bench_effect_trigger,
    bench_effect_multiple_deps,
    bench_batch_updates,
);

criterion_group!(
    stress_benches,
    bench_many_signals,
    bench_many_effects,
);

criterion_main!(signal_benches, derived_benches, effect_benches, stress_benches, ts_compare_benches);
