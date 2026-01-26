use spark_signals::{
    cloned, derived, effect, reactive_prop, signal, slot, tracked_slot_array, dirty_set,
    PropValue,
};
use std::rc::Rc;

#[test]
fn showcase_basics() {
    let count = signal(1);
    let multiplier = signal(10);

    // 1. Derived with cloned!
    // Captures 'count' and 'multiplier' without manual clones
    let result = derived(cloned!(count, multiplier => move || {
        count.get() * multiplier.get()
    }));

    assert_eq!(result.get(), 10);
}

#[test]
fn showcase_slot() {
    // Slot: A stable reference that can switch sources
    let s = slot::<i32>(None);
    let trigger = signal(0);
    
    // Using slot in an effect with cloned!
    // The slot itself needs to be captured to be read
    let _e = effect(cloned!(s, trigger => move || {
        // Read trigger to run often
        let _ = trigger.get();
        // Read slot (might be None, or value, or signal)
        let _val = s.get();
    }));
    
    // Also useful when setting a getter source for a slot
    let source_sig = signal(42);
    s.set_getter(Box::new(cloned!(source_sig => move || source_sig.get())));
    
    assert_eq!(s.get(), Some(42));
}

#[test]
fn showcase_reactive_props() {
    let user_name = signal("Rusty".to_string());
    let formatting = signal("Uppercase".to_string());

    // PropValue::Getter expects a Box<dyn Fn() -> T>
    // cloned! is perfect for creating the closure that goes into the box
    let prop = PropValue::Getter(Box::new(cloned!(user_name, formatting => move || {
        let name = user_name.get();
        match formatting.get().as_str() {
            "Uppercase" => name.to_uppercase(),
            _ => name.to_lowercase(),
        }
    })));

    // Convert to derived for uniform access
    let display_name = reactive_prop(prop);

    assert_eq!(display_name.get(), "RUSTY");
    
    formatting.set("lower".to_string());
    assert_eq!(display_name.get(), "rusty");
}

#[test]
fn showcase_tracked_slot_array() {
    // ECS Pattern: Parallel arrays for Position and Velocity
    // TrackedSlotArray needs a shared DirtySet to report changes
    let changes = dirty_set();
    // TrackedSlotArray is not implicitly shared, so we wrap in Rc for the system
    let positions = Rc::new(tracked_slot_array(Some((0.0, 0.0)), changes.clone()));
    
    // Initialize some entities
    for i in 0..5 {
        positions.set_value(i, (i as f32, i as f32));
    }

    // A "System" that reads positions
    // Needs to capture 'positions' and 'changes'
    let _render_system = effect(cloned!(positions, changes => move || {
        // Iterate only changed entities (ECS optimization)
        for &entity_id in changes.borrow().iter() {
            let _pos = positions.get(entity_id);
            // ... render logic ...
        }
    }));
    
    // Trigger a change
    changes.borrow_mut().clear(); // Clear previous frame changes
    positions.set_value(0, (100.0, 100.0));
    
    // Verify dirty set tracked it
    assert!(changes.borrow().contains(&0));
}
