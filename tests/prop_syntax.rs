use spark_signals::{prop, signal, reactive_prop, PropValue};

#[test]
fn macro_prop_syntax() {
    let first = signal("Sherlock".to_string());
    let last = signal("Holmes".to_string());

    // Magic syntax for props
    // prop!(deps => expression) creates a PropValue::Getter
    let full_name_prop = prop!(first, last => format!("{} {}", first.get(), last.get()));

    // Convert to derived for usage
    let full_name = reactive_prop(full_name_prop);

    assert_eq!(full_name.get(), "Sherlock Holmes");

    first.set("Mycroft".to_string());
    assert_eq!(full_name.get(), "Mycroft Holmes");
}

#[test]
fn macro_prop_no_deps() {
    // Just a computed prop without external deps (rare but valid)
    let static_calc = prop!({
        10 + 20
    });
    
    let val = reactive_prop(static_calc);
    assert_eq!(val.get(), 30);
}
