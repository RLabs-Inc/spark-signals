//! spark-signals Benchmark Suite
//!
//! Comprehensive benchmarks covering all primitives and patterns.
//!
//! ## Target Performance (from TypeScript reference)
//! - Signal read: < 50ns (we achieve ~6ns)
//! - Signal write: < 100ns
//! - Derived cached read: < 50ns
//! - 1000-chain propagation: < 10ms
//!
//! ## Run Commands
//! ```bash
//! cargo bench                           # All benchmarks
//! cargo bench -- "signal/"              # Signal-only
//! cargo bench -- "derived/"             # Derived-only
//! cargo bench -- "stress/"              # Stress tests
//! cargo bench -- --test                 # Quick compile check
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use spark_signals::{
    batch, create_selector_eq, derived, dirty_set, effect, effect_scope, effect_sync,
    linked_signal, reactive_prop, signal, slot, slot_array, tracked_slot_array, untrack,
    PropValue, ReactiveMap, ReactiveSet, ReactiveVec,
};

// =============================================================================
// SIGNAL PRIMITIVES
// Target: read < 50ns, write < 100ns
// =============================================================================

fn signal_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("signal");

    // Creation
    g.bench_function("create", |b| b.iter(|| black_box(signal(0i32))));

    // Read (hot path - must be fast)
    let s = signal(42i32);
    g.bench_function("get", |b| b.iter(|| black_box(s.get())));

    // Read with closure (avoids clone for complex types)
    let vec_sig = signal(vec![1, 2, 3, 4, 5]);
    g.bench_function("with", |b| {
        b.iter(|| black_box(vec_sig.with(|v| v.iter().sum::<i32>())))
    });

    // Write (changing value)
    let write_sig = signal(0i32);
    let mut i = 0i32;
    g.bench_function("set", |b| {
        b.iter(|| {
            write_sig.set(black_box(i));
            i = i.wrapping_add(1);
        })
    });

    // Write (same value - should be fast due to equality check)
    let same_sig = signal(42i32);
    g.bench_function("set_same_value", |b| b.iter(|| same_sig.set(black_box(42))));

    // Update (read-modify-write)
    let update_sig = signal(0i32);
    g.bench_function("update", |b| {
        b.iter(|| update_sig.update(|v| *v = v.wrapping_add(1)))
    });

    // Untracked read (equivalent to peek)
    let peek_sig = signal(42i32);
    g.bench_function("untracked_get", |b| {
        b.iter(|| untrack(|| black_box(peek_sig.get())))
    });

    g.finish();
}

// =============================================================================
// DERIVED PRIMITIVES
// Target: cached read < 50ns
// =============================================================================

fn derived_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("derived");

    // Creation
    let source = signal(0i32);
    g.bench_function("create", |b| {
        let s = source.clone();
        b.iter(|| black_box(derived({ let s = s.clone(); move || s.get() * 2 })))
    });

    // Cached read (no recomputation)
    let cache_source = signal(42i32);
    let cached = derived({ let s = cache_source.clone(); move || s.get() * 2 });
    let _ = cached.get(); // Prime
    g.bench_function("get_cached", |b| b.iter(|| black_box(cached.get())));

    // Dirty read (requires recomputation)
    let dirty_source = signal(0i32);
    let dirty = derived({ let s = dirty_source.clone(); move || s.get() * 2 });
    let mut j = 0i32;
    g.bench_function("get_dirty", |b| {
        b.iter(|| {
            dirty_source.set(j);
            j = j.wrapping_add(1);
            black_box(dirty.get())
        })
    });

    // Diamond pattern: A -> B, A -> C, B+C -> D
    let a = signal(1i32);
    let b = derived({ let a = a.clone(); move || a.get() + 10 });
    let c_d = derived({ let a = a.clone(); move || a.get() * 10 });
    let d = derived({ let b = b.clone(); let c = c_d.clone(); move || b.get() + c.get() });
    let _ = d.get();
    let mut k = 1i32;
    g.bench_function("diamond", |b| {
        b.iter(|| {
            a.set(k);
            k = k.wrapping_add(1);
            black_box(d.get())
        })
    });

    g.finish();
}

// =============================================================================
// EFFECT PRIMITIVES
// =============================================================================

fn effect_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("effect");

    // Creation (async scheduled)
    g.bench_function("create", |b| {
        b.iter(|| {
            let e = effect(|| {});
            black_box(e)
        })
    });

    // Creation (sync)
    g.bench_function("create_sync", |b| {
        b.iter(|| {
            let e = effect_sync(|| {});
            black_box(e)
        })
    });

    // Trigger (signal change causes effect to run)
    let trigger_sig = signal(0i32);
    let trigger_clone = trigger_sig.clone();
    let _trigger_effect = effect_sync(move || { black_box(trigger_clone.get()); });
    let mut m = 0i32;
    g.bench_function("trigger", |b| {
        b.iter(|| {
            trigger_sig.set(m);
            m = m.wrapping_add(1);
        })
    });

    // Through derived chain
    let chain_sig = signal(0i32);
    let chain_derived = derived({ let s = chain_sig.clone(); move || s.get() * 2 });
    let chain_derived_clone = chain_derived.clone();
    let _chain_effect = effect_sync(move || { black_box(chain_derived_clone.get()); });
    let mut n = 0i32;
    g.bench_function("through_derived", |b| {
        b.iter(|| {
            chain_sig.set(n);
            n = n.wrapping_add(1);
        })
    });

    // Multiple dependencies
    let ma = signal(0i32);
    let mb = signal(0i32);
    let mc = signal(0i32);
    let ma_c = ma.clone();
    let mb_c = mb.clone();
    let mc_c = mc.clone();
    let _multi_effect = effect_sync(move || { black_box(ma_c.get() + mb_c.get() + mc_c.get()); });
    let mut o = 0i32;
    g.bench_function("multi_deps", |b| {
        b.iter(|| {
            ma.set(o);
            o = o.wrapping_add(1);
        })
    });

    g.finish();
}

// =============================================================================
// BATCH & UNTRACK
// =============================================================================

fn batch_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("batch");

    // Batch N updates - only one effect run
    for count in [1, 10, 100] {
        let batch_sig = signal(0i32);
        let batch_clone = batch_sig.clone();
        let _batch_effect = effect_sync(move || { black_box(batch_clone.get()); });

        g.bench_with_input(BenchmarkId::new("updates", count), &count, |b, &count| {
            b.iter(|| {
                batch(|| {
                    for i in 0..count {
                        batch_sig.set(black_box(i));
                    }
                })
            })
        });
    }

    // Nested batches
    let na = signal(0i32);
    let nb = signal(0i32);
    let nc = signal(0i32);
    let na_c = na.clone();
    let nb_c = nb.clone();
    let nc_c = nc.clone();
    let _nested_effect = effect_sync(move || { black_box(na_c.get() + nb_c.get() + nc_c.get()); });
    let mut p = 0i32;
    g.bench_function("nested_3_levels", |b| {
        b.iter(|| {
            batch(|| {
                na.set(p);
                batch(|| {
                    nb.set(p);
                    batch(|| {
                        nc.set(p);
                    });
                });
            });
            p = p.wrapping_add(1);
        })
    });

    g.finish();

    // Untrack
    let mut g2 = c.benchmark_group("untrack");
    let ut_sig = signal(42i32);
    g2.bench_function("read", |b| {
        b.iter(|| {
            untrack(|| black_box(ut_sig.get()))
        })
    });
    g2.finish();
}

// =============================================================================
// SELECTOR (O(2) optimization)
// =============================================================================

fn selector_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("selector");

    // Creation
    let sel_source = signal(1i32);
    g.bench_function("create", |b| {
        let s = sel_source.clone();
        b.iter(|| black_box(create_selector_eq({ let s = s.clone(); move || s.get() })))
    });

    // is_selected check
    let is_source = signal(1i32);
    let is_selector = create_selector_eq({ let s = is_source.clone(); move || s.get() });
    g.bench_function("is_selected", |b| {
        b.iter(|| black_box(is_selector.is_selected(&1)))
    });

    // O(2) vs O(n) comparison
    for count in [10, 100, 500] {
        // Selector O(2) approach
        g.bench_with_input(BenchmarkId::new("o2_change", count), &count, |b, &count| {
            let selected = signal(0i32);
            let selector = create_selector_eq({ let s = selected.clone(); move || s.get() });

            let _effects: Vec<_> = (0..count).map(|i| {
                let sel = selector.clone();
                effect_sync(move || { black_box(sel.is_selected(&i)); })
            }).collect();

            let mut i = 0i32;
            b.iter(|| {
                i = (i + 1) % count;
                selected.set(black_box(i));
            })
        });

        // Naive O(n) approach for comparison
        g.bench_with_input(BenchmarkId::new("naive_on", count), &count, |b, &count| {
            let selected = signal(0i32);

            let _effects: Vec<_> = (0..count).map(|i| {
                let s = selected.clone();
                effect_sync(move || { black_box(s.get() == i); })
            }).collect();

            let mut i = 0i32;
            b.iter(|| {
                i = (i + 1) % count;
                selected.set(black_box(i));
            })
        });
    }

    g.finish();
}

// =============================================================================
// LINKED SIGNAL
// =============================================================================

fn linked_signal_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("linked_signal");

    // Creation
    let ls_source = signal(1i32);
    g.bench_function("create", |b| {
        let s = ls_source.clone();
        b.iter(|| black_box(linked_signal({ let s = s.clone(); move || s.get() })))
    });

    // Get (from source)
    let lg_source = signal(42i32);
    let lg_linked = linked_signal({ let s = lg_source.clone(); move || s.get() });
    g.bench_function("get", |b| b.iter(|| black_box(lg_linked.get())));

    // Set (override)
    let lso_source = signal(1i32);
    let lso_linked = linked_signal({ let s = lso_source.clone(); move || s.get() });
    let mut q = 0i32;
    g.bench_function("set_override", |b| {
        b.iter(|| {
            lso_linked.set(q);
            q = q.wrapping_add(1);
        })
    });

    // Source change sync
    let lsc_source = signal(1i32);
    let lsc_linked = linked_signal({ let s = lsc_source.clone(); move || s.get() });
    let mut r = 0i32;
    g.bench_function("source_change", |b| {
        b.iter(|| {
            lsc_source.set(r);
            black_box(lsc_linked.get());
            r = r.wrapping_add(1);
        })
    });

    g.finish();
}

// =============================================================================
// SLOT & SLOT ARRAY (Entity Component backbone)
// =============================================================================

fn slot_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("slot");

    // Slot creation
    g.bench_function("create", |b| b.iter(|| black_box(slot::<i32>(None))));

    // Slot get/set
    let gs_slot = slot(Some(42i32));
    let mut s_val = 0i32;
    g.bench_function("get_set", |b| {
        b.iter(|| {
            gs_slot.set_value(s_val);
            s_val = s_val.wrapping_add(1);
            black_box(gs_slot.get())
        })
    });

    g.finish();

    // Slot Array
    let mut g2 = c.benchmark_group("slot_array");

    g2.bench_function("create", |b| b.iter(|| black_box(slot_array::<i32>(Some(0)))));

    // Slot array access
    let sa = slot_array::<i32>(Some(0));
    for i in 0..1000 { sa.set_value(i, i as i32); }
    let mut idx = 0usize;
    g2.bench_function("access_1000", |b| {
        b.iter(|| {
            sa.set_value(idx, (idx * 2) as i32);
            black_box(sa.get(idx));
            idx = (idx + 1) % 1000;
        })
    });

    g2.finish();

    // Tracked Slot Array
    let mut g3 = c.benchmark_group("tracked_slot_array");

    let tsa_dirty = dirty_set();
    g3.bench_function("create", |b| {
        let d = tsa_dirty.clone();
        b.iter(|| black_box(tracked_slot_array::<i32>(Some(0), d.clone())))
    });

    // Access with dirty tracking
    let tsa_dirty2 = dirty_set();
    let tsa = tracked_slot_array::<i32>(Some(0), tsa_dirty2.clone());
    for i in 0..1000 { tsa.set_value(i, i as i32); }
    let mut tsa_idx = 0usize;
    g3.bench_function("access_1000", |b| {
        b.iter(|| {
            tsa.set_value(tsa_idx, (tsa_idx * 2) as i32);
            black_box(tsa.get(tsa_idx));
            tsa_idx = (tsa_idx + 1) % 1000;
        })
    });

    // Dirty iteration (ECS pattern)
    let di_dirty = dirty_set();
    let di_arr = tracked_slot_array::<i32>(Some(0), di_dirty.clone());
    for i in 0..1000 { di_arr.set_value(i, i as i32); }
    di_dirty.borrow_mut().clear();
    for i in (0..1000).step_by(10) { di_arr.set_value(i, (i * 2) as i32); }

    g3.bench_function("dirty_iter_100", |b| {
        b.iter(|| {
            let mut sum = 0i32;
            for &idx in di_dirty.borrow().iter() {
                sum += di_arr.peek(idx).unwrap_or(0);
            }
            black_box(sum)
        })
    });

    g3.finish();
}

// =============================================================================
// COLLECTIONS
// =============================================================================

fn collection_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("collections");

    // ReactiveVec
    g.bench_function("vec_create", |b| b.iter(|| black_box(ReactiveVec::<i32>::new())));

    g.bench_function("vec_push_100", |b| {
        b.iter(|| {
            let mut v = ReactiveVec::new();
            for i in 0..100 { v.push(i); }
            black_box(v)
        })
    });

    let rv = {
        let mut v = ReactiveVec::new();
        for i in 0..100 { v.push(i); }
        v
    };
    g.bench_function("vec_get", |b| b.iter(|| black_box(rv.get(50))));

    // ReactiveMap
    g.bench_function("map_create", |b| b.iter(|| black_box(ReactiveMap::<i32, i32>::new())));

    g.bench_function("map_insert_100", |b| {
        b.iter(|| {
            let mut m = ReactiveMap::new();
            for i in 0..100 { m.insert(i, i * 2); }
            black_box(m)
        })
    });

    let rm = {
        let mut m = ReactiveMap::new();
        for i in 0..100 { m.insert(i, i * 2); }
        m
    };
    g.bench_function("map_get", |b| b.iter(|| black_box(rm.get(&50))));

    // ReactiveSet
    g.bench_function("set_insert_100", |b| {
        b.iter(|| {
            let mut s = ReactiveSet::new();
            for i in 0..100 { s.insert(i); }
            black_box(s)
        })
    });

    g.finish();
}

// =============================================================================
// SCOPE
// =============================================================================

fn scope_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("scope");

    g.bench_function("create", |b| {
        b.iter(|| black_box(effect_scope(false)))
    });

    let scope_sig = signal(0i32);
    g.bench_function("with_10_effects", |b| {
        b.iter(|| {
            let scope = effect_scope(false);
            scope.run(|| {
                for _ in 0..10 {
                    let s = scope_sig.clone();
                    let _e = effect(move || { black_box(s.get()); });
                }
            });
            scope.stop();
        })
    });

    g.finish();
}

// =============================================================================
// PROP VALUE
// =============================================================================

fn prop_value_operations(c: &mut Criterion) {
    let mut g = c.benchmark_group("prop_value");

    g.bench_function("static", |b| {
        b.iter(|| {
            let prop = PropValue::Static(42i32);
            let d = reactive_prop(prop);
            black_box(d.get())
        })
    });

    let pv_source = signal(42i32);
    g.bench_function("getter", |b| {
        let s = pv_source.clone();
        b.iter(|| {
            let s = s.clone();
            let prop = PropValue::Getter(Box::new(move || s.get()));
            let d = reactive_prop(prop);
            black_box(d.get())
        })
    });

    g.bench_function("signal", |b| {
        let s = pv_source.clone();
        b.iter(|| {
            let prop = PropValue::from_signal(&s);
            let d = reactive_prop(prop);
            black_box(d.get())
        })
    });

    g.finish();
}

// =============================================================================
// CHAIN DEPTH STRESS
// Target: 1000-chain < 10ms
// =============================================================================

fn chain_stress(c: &mut Criterion) {
    let mut g = c.benchmark_group("stress/chain");

    for depth in [10, 50, 100, 500, 1000] {
        g.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &depth| {
            let root = signal(1i32);

            let mut current = { let r = root.clone(); derived(move || r.get() + 1) };
            for _ in 1..depth {
                let prev = current.clone();
                current = derived(move || prev.get() + 1);
            }

            let _ = current.get(); // Prime
            let mut i = 1i32;
            b.iter(|| {
                root.set(i);
                i = i.wrapping_add(1);
                black_box(current.get())
            })
        });
    }

    g.finish();
}

// =============================================================================
// WIDE FAN-OUT STRESS
// =============================================================================

fn fanout_stress(c: &mut Criterion) {
    let mut g = c.benchmark_group("stress/fanout");

    for count in [10, 100, 500, 1000] {
        g.bench_with_input(BenchmarkId::new("effects", count), &count, |b, &count| {
            let source = signal(0i32);

            let _effects: Vec<_> = (0..count).map(|_| {
                let s = source.clone();
                effect_sync(move || { black_box(s.get()); })
            }).collect();

            let mut i = 0i32;
            b.iter(|| {
                source.set(i);
                i = i.wrapping_add(1);
            })
        });

        g.bench_with_input(BenchmarkId::new("deriveds", count), &count, |b, &count| {
            let source = signal(0i32);

            let deriveds: Vec<_> = (0..count).map(|i| {
                let s = source.clone();
                derived(move || s.get() + i)
            }).collect();

            let deriveds_c = deriveds.clone();
            let _e = effect_sync(move || {
                let sum: i32 = deriveds_c.iter().map(|d| d.get()).sum();
                black_box(sum);
            });

            let mut i = 0i32;
            b.iter(|| {
                source.set(i);
                i = i.wrapping_add(1);
            })
        });
    }

    g.finish();
}

// =============================================================================
// LIFECYCLE STRESS (rapid create/drop)
// =============================================================================

fn lifecycle_stress(c: &mut Criterion) {
    let mut g = c.benchmark_group("stress/lifecycle");

    g.bench_function("signal_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let s = signal(i);
                black_box(s.get());
            }
        })
    });

    let lc_source = signal(0i32);
    g.bench_function("effect_100", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let s = lc_source.clone();
                let e = effect_sync(move || { black_box(s.get()); });
                drop(e);
            }
        })
    });

    g.bench_function("scope_10_effects", |b| {
        b.iter(|| {
            let scope = effect_scope(false);
            scope.run(|| {
                for _ in 0..10 {
                    let s = lc_source.clone();
                    let _e = effect(move || { black_box(s.get()); });
                }
            });
            scope.stop();
        })
    });

    g.finish();
}

// =============================================================================
// DIAMOND STRESS (many parallel diamonds)
// =============================================================================

fn diamond_stress(c: &mut Criterion) {
    let mut g = c.benchmark_group("stress/diamond");

    for count in [5, 10, 20] {
        g.bench_with_input(BenchmarkId::new("count", count), &count, |b, &count| {
            let root = signal(1i32);

            let finals: Vec<_> = (0..count).map(|i| {
                let r = root.clone();
                let left = derived({ let r = r.clone(); move || r.get() + i });
                let right = derived({ let r = r.clone(); move || r.get() * (i + 1) });
                let l = left.clone();
                let ri = right.clone();
                derived(move || l.get() + ri.get())
            }).collect();

            let finals_c = finals.clone();
            let _e = effect_sync(move || {
                let sum: i32 = finals_c.iter().map(|d| d.get()).sum();
                black_box(sum);
            });

            let mut i = 1i32;
            b.iter(|| {
                root.set(i);
                i = i.wrapping_add(1);
            })
        });
    }

    g.finish();
}

// =============================================================================
// ECS PATTERN (game-loop simulation)
// =============================================================================

fn ecs_stress(c: &mut Criterion) {
    let mut g = c.benchmark_group("stress/ecs");

    // Position + Velocity update (1000 entities)
    let pv_dirty = dirty_set();
    let positions = tracked_slot_array::<(f32, f32)>(Some((0.0, 0.0)), pv_dirty.clone());
    let velocities = slot_array::<(f32, f32)>(Some((0.0, 0.0)));

    for i in 0..1000 {
        positions.set_value(i, (i as f32, i as f32 * 2.0));
        velocities.set_value(i, (1.0, 0.5));
    }

    g.bench_function("pos_vel_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let pos = positions.peek(i).unwrap();
                let vel = velocities.peek(i).unwrap();
                positions.set_value(i, (pos.0 + vel.0, pos.1 + vel.1));
            }
        })
    });

    // Batched game loop
    g.bench_function("batched_100_entities", |b| {
        let gl_pos = slot_array::<(f32, f32)>(Some((0.0, 0.0)));
        let gl_vel = slot_array::<(f32, f32)>(Some((0.0, 0.0)));
        for i in 0..100 {
            gl_pos.set_value(i, (i as f32, i as f32));
            gl_vel.set_value(i, (1.0, 0.5));
        }

        b.iter(|| {
            batch(|| {
                for i in 0..100 {
                    let pos = gl_pos.peek(i).unwrap();
                    let vel = gl_vel.peek(i).unwrap();
                    gl_pos.set_value(i, (pos.0 + vel.0, pos.1 + vel.1));
                }
            });
        })
    });

    g.finish();
}

// =============================================================================
// MEGA BATCH (many signals at once)
// =============================================================================

fn batch_stress(c: &mut Criterion) {
    let mut g = c.benchmark_group("stress/batch");

    for count in [10, 100, 1000] {
        g.bench_with_input(BenchmarkId::new("signals", count), &count, |b, &count| {
            let signals: Vec<_> = (0..count).map(|i| signal(i)).collect();

            let signals_c: Vec<_> = signals.iter().cloned().collect();
            let _e = effect_sync(move || {
                let sum: i32 = signals_c.iter().map(|s| s.get()).sum();
                black_box(sum);
            });

            let mut i = 0i32;
            b.iter(|| {
                batch(|| {
                    for s in &signals { s.set(i); }
                });
                i = i.wrapping_add(1);
            })
        });
    }

    g.finish();
}

// =============================================================================
// CRITERION GROUPS
// =============================================================================

criterion_group!(
    primitives,
    signal_operations,
    derived_operations,
    effect_operations,
    batch_operations,
    selector_operations,
    linked_signal_operations,
    slot_operations,
    prop_value_operations,
);

criterion_group!(
    collections_scope,
    collection_operations,
    scope_operations,
);

criterion_group!(
    stress,
    chain_stress,
    fanout_stress,
    lifecycle_stress,
    diamond_stress,
    ecs_stress,
    batch_stress,
);

criterion_main!(primitives, collections_scope, stress);
