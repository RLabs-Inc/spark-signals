use spark_signals::{derived, effect, signal, cloned};

#[test]
fn macro_derived_syntax() {
    let a = signal(10);
    let b = signal(20);

    // Pure magic syntax
    let sum = derived!(a, b => a.get() + b.get());

    assert_eq!(sum.get(), 30);
    
    a.set(15);
    assert_eq!(sum.get(), 35);
}

#[test]
fn macro_effect_syntax() {
    let a = signal(0);
    let b = signal(0);
    
    // Pure magic syntax
    let _e = effect!(a, b => {
        let _ = a.get();
        let _ = b.get();
    });
    
    a.set(1);
}

#[test]
fn macro_nested_usage() {
    let a = signal(1);
    
    // Nesting derived! inside derived!
    let d = derived!(a => {
        let inner = derived!(a => a.get() * 2);
        inner.get()
    });
    
    assert_eq!(d.get(), 2);
}
