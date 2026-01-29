//! Minimal demonstration of the 'static lifetime restriction.
//!
//! This test will FAIL TO COMPILE, demonstrating that callbacks
//! cannot capture non-'static references.

use csound::Csound;

#[test]
fn test_cannot_capture_local_mut_ref() {
    let mut counter = 0u32;

    let cs = Csound::new().expect("Failed to create Csound");
    cs.set_option("-n").unwrap();

    // This should fail to compile but apparently doesn't!
    cs.message_string_callback(|_mtype, _msg| {
        counter += 1; // This actually works!
    });

    cs.compile_orc("sr = 44100\nksmps = 32\nnchnls = 2\n0dbfs = 1", 0)
        .unwrap();
    cs.start().unwrap();

    while !cs.perform_ksmps() {}

    drop(cs);

    // Counter should have been incremented by the callback
    println!("Counter after csound: {}", counter);
    assert!(counter > 0, "Counter was incremented: {}", counter);
}

// This version compiles because we DON'T capture any references
#[test]
fn test_static_callback_works() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Arc makes it 'static - this is the only way currently
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);

    let cs = Csound::new().expect("Failed to create Csound");
    cs.set_option("-n").unwrap();

    // This works because Arc is 'static
    cs.message_string_callback(move |_mtype, _msg| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    cs.compile_orc("sr = 44100\nksmps = 32\nnchnls = 2\n0dbfs = 1", 0)
        .unwrap();
    cs.start().unwrap();

    while !cs.perform_ksmps() {}

    // Counter is accessible after cs is dropped
    assert!(counter.load(Ordering::SeqCst) > 0);
}
