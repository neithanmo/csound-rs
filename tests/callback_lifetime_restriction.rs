//! Test demonstrating the 'static lifetime restriction on callbacks.
//!
//! This test shows scenarios where users would reasonably want to capture
//! non-'static references in callbacks, but can't due to the current API design.

use csound::{Csound, MessageType};

/// Example 1: Cannot capture a mutable reference to application state
///
/// This is a common pattern - wanting to update application state from callbacks.
/// With 'static requirement, users are forced to use Arc<Mutex<_>> even when
/// the state clearly outlives the Csound instance.
#[test]
#[ignore] // This test won't compile with current 'static restriction
fn test_cannot_capture_mut_ref() {
    struct AppState {
        message_count: u32,
        error_messages: Vec<String>,
    }

    let mut app_state = AppState {
        message_count: 0,
        error_messages: Vec::new(),
    };

    let cs = Csound::new().expect("Failed to create Csound");

    // This SHOULD work - app_state clearly outlives cs
    // But won't compile because closure needs 'static
    cs.message_string_callback(|mtype: MessageType, msg: &str| {
        app_state.message_count += 1; // ERROR: borrowed data escapes outside of function
        if mtype == MessageType::Error {
            app_state.error_messages.push(msg.to_string());
        }
    });

    cs.compile_orc("sr = 44100\nksmps = 32\nnchnls = 2\n0dbfs = 1", 0)
        .unwrap();
    cs.start().unwrap();

    // app_state is used after cs, proving it outlives cs
    println!("Message count: {}", app_state.message_count);
}

/// Example 2: Cannot capture a reference to configuration
///
/// Another common pattern - reading configuration that's owned elsewhere
/// but clearly outlives the Csound instance.
#[test]
#[ignore] // Won't compile with current 'static restriction
fn test_cannot_capture_config_ref() {
    struct AudioConfig {
        sample_rate: u32,
        #[allow(dead_code)]
        channels: u32,
        #[allow(dead_code)]
        buffer_size: u32,
    }

    let config = AudioConfig {
        sample_rate: 44100,
        channels: 2,
        buffer_size: 256,
    };

    let cs = Csound::new().expect("Failed to create Csound");

    // This SHOULD work - config clearly outlives cs
    cs.message_string_callback(|_mtype: MessageType, msg: &str| {
        // Want to log with config info
        println!(
            "[SR={}] {}",
            config.sample_rate, // ERROR: borrowed data escapes
            msg
        );
    });

    drop(cs);
    // config is still valid here, proving it outlives cs
    println!("Config valid: {}", config.sample_rate);
}

/// Example 3: The workaround - forced to use Arc<Mutex<_>>
///
/// This shows what users MUST do currently, even when it's unnecessary.
#[test]
fn test_workaround_with_arc_mutex() {
    use std::sync::{Arc, Mutex};

    struct AppState {
        message_count: u32,
    }

    // Forced to wrap in Arc<Mutex<_>> even though single-threaded
    let app_state = Arc::new(Mutex::new(AppState { message_count: 0 }));
    let state_clone = Arc::clone(&app_state);

    let cs = Csound::new().expect("Failed to create Csound");

    // This works because Arc<Mutex<_>> is 'static
    cs.message_string_callback(move |_mtype: MessageType, _msg: &str| {
        let mut state = state_clone.lock().unwrap();
        state.message_count += 1;
    });

    cs.set_option("-n").unwrap();
    cs.compile_orc("sr = 44100\nksmps = 32\nnchnls = 2\n0dbfs = 1", 0)
        .unwrap();
    cs.start().unwrap();

    while !cs.perform_ksmps() {
        // Performance loop
    }

    let final_count = app_state.lock().unwrap().message_count;
    assert!(final_count > 0, "Callback should have been invoked");
}

/// Example 4: What the API SHOULD allow
///
/// This pseudo-code shows what users should be able to write if callbacks
/// were properly lifetime-parameterized.
#[test]
#[ignore] // This is the desired API, but won't compile currently
fn test_desired_api() {
    struct AppState {
        message_count: u32,
    }

    let mut app_state = AppState { message_count: 0 };

    // Ideal: Csound<'state> where 'state >= lifetime of app_state borrow
    let cs = Csound::new().expect("Failed to create Csound");

    // The borrow checker should verify that app_state outlives cs
    cs.message_string_callback(|_mtype: MessageType, _msg: &str| {
        app_state.message_count += 1; // Should work!
    });

    cs.set_option("-n").unwrap();
    cs.compile_orc("sr = 44100\nksmps = 32\nnchnls = 2\n0dbfs = 1", 0)
        .unwrap();
    cs.start().unwrap();

    while !cs.perform_ksmps() {}

    // app_state is still accessible here
    assert!(app_state.message_count > 0);
}
