use spark_signals::{cloned, derived, effect, signal, untrack};

#[test]
fn ergonomic_cloned_macro() {
    let a = signal(10);
    let b = signal(20);

    // Old way (painful)
    let _sum_old = derived({
        let a = a.clone();
        let b = b.clone();
        move || a.get() + b.get()
    });

    // New way (ergonomic)
    let sum = derived(cloned!(a, b => move || a.get() + b.get()));

    assert_eq!(sum.get(), 30);

    a.set(15);
    assert_eq!(sum.get(), 35);
}

#[test]
fn ergonomic_cloned_macro_in_effect() {
    let a = signal(0);
    let b = signal(0);
    
    // Capture multiple signals in effect
    let _e = effect(cloned!(a, b => move || {
        let _ = a.get();
        let _ = b.get();
    }));
    
    a.set(1);
    // Should pass (compilation is the main test here)
}

#[test]
fn ergonomic_cloned_macro_nested() {
    let a = signal(1);
    
    let d = derived(cloned!(a => move || {
        // Nesting works too
        untrack(cloned!(a => move || a.get() * 2))
    }));
    
    assert_eq!(d.get(), 2);
}
