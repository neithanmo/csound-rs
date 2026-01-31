//! Minimal demonstration of the 'static lifetime restriction.
//!
//! This test will FAIL TO COMPILE, demonstrating that callbacks
//! cannot capture non-'static references.

use csound::Csound;

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

    cs.compile_orc(
        r#"sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1
instr 1
  print p1
endin"#,
        0,
    )
    .unwrap();
    cs.send_string_event("i1 0 0.1\ne", 0).ok();
    cs.start().unwrap();

    while !cs.perform_ksmps() {}

    // Counter is accessible after cs is dropped
    assert!(counter.load(Ordering::SeqCst) > 0);
}
