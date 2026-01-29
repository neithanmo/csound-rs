//! Integration tests for callback panic handling.
//!
//! These tests verify that the panic handler correctly catches panics in user callbacks
//! and prevents re-entry into panicked callbacks while allowing Csound to continue.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use csound::{Csound, MessageType, PanickedCallbacks};

/// A simple orchestra that generates messages when compiled and run.
static ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  aout oscil 0.5, 440
  outs aout, aout
endin
"#;

/// A score that triggers instrument 1 and generates additional messages.
static SCO: &str = r#"
i1 0 1
e
"#;

/// Test that the panic handler correctly catches a panic in the message callback
/// and prevents re-entry into the panicked callback.
///
/// The test sets up a message callback that:
/// 1. Increments a counter on each call
/// 2. Panics when the counter reaches a threshold
///
/// After the panic:
/// - The callback should be marked as panicked
/// - Subsequent calls should be skipped (counter doesn't increase)
/// - Csound should continue operating normally
#[test]
fn test_message_callback_panic_handler() {
    const PANIC_THRESHOLD: u32 = 5;

    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);

    let cs = Csound::new().expect("Failed to create Csound instance");

    // Suppress audio output for testing
    cs.set_option("-n").unwrap(); // no sound

    // Set up a message callback that panics when counter reaches threshold
    cs.message_string_callback(move |_mtype: MessageType, _message: &str| {
        let current = counter_clone.fetch_add(1, Ordering::SeqCst);
        if current == PANIC_THRESHOLD - 1 {
            // Panic on the Nth call (0-indexed)
            panic!("Intentional panic at counter == {}", PANIC_THRESHOLD);
        }
    });

    // Compile the orchestra
    cs.compile_orc(ORC, 0).expect("Failed to compile orchestra");

    // Read the score to generate more messages
    cs.send_string_event(SCO, 0).ok();

    // Start csound - generates version messages, etc.
    cs.start().expect("Failed to start csound");

    // Run performance loop - this generates many messages
    // (section markers, timing info, etc.)
    while !cs.perform_ksmps() {
        // Performance continues even after callback panic
    }

    // After performance, verify:
    // 1. The counter reached exactly PANIC_THRESHOLD (panic happened on that call)
    // 2. The MESSAGE callback is marked as panicked
    // 3. Counter did NOT increase after the panic (callback was skipped)
    let final_count = counter.load(Ordering::SeqCst);

    // The count should be exactly PANIC_THRESHOLD because after the panic,
    // the callback is skipped entirely
    assert_eq!(
        final_count, PANIC_THRESHOLD,
        "Counter should be exactly {} (panic threshold), but was {}",
        PANIC_THRESHOLD, final_count
    );

    // Verify the panic state was recorded
    let panic_state = cs.panic_state();
    assert!(
        panic_state.has_panicked(PanickedCallbacks::MESSAGE),
        "MESSAGE callback should be marked as panicked"
    );

    // Verify other callbacks are NOT marked as panicked
    assert!(
        !panic_state.has_panicked(PanickedCallbacks::RT_PLAY),
        "RT_PLAY callback should not be marked as panicked"
    );
    assert!(
        !panic_state.has_panicked(PanickedCallbacks::FILE_OPEN),
        "FILE_OPEN callback should not be marked as panicked"
    );
}
