//! Integration tests for channels, tables, and message buffer APIs.
//!
//! These tests verify the high-level Rust API for:
//! - Control channels (get/set roundtrip)
//! - Audio channels (read/write with size validation)
//! - String channels (get/set roundtrip)
//! - Function tables (create, length, read/write cycles)
//! - Message buffer (create, drain messages)

use csound::{Csound, MessageType, Myflt};

/// Creates a Csound instance configured for testing.
fn create_test_csound() -> Csound {
    let cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");
    cs.set_option("-d").expect("Failed to set -d option");
    cs.set_option("-m0").expect("Failed to set -m0 option");
    cs.message_string_callback(|_: MessageType, _: &str| {});
    cs
}

// ============================================================================
// CONTROL CHANNEL TESTS
// ============================================================================

/// Orchestra that declares a control channel for input.
static CONTROL_CHANNEL_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

chn_k "freq", 1  ; input channel

instr 1
  kfreq chnget "freq"
  aout oscil 0.5, kfreq
  outs aout, aout
endin
"#;

#[test]
fn test_control_channel_roundtrip() {
    let mut cs = create_test_csound();

    cs.compile_orc(CONTROL_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    // Set a control channel value
    cs.set_control_channel("freq", 440.0)
        .expect("Failed to set control channel");

    // Read it back
    let value = cs
        .get_control_channel("freq")
        .expect("Failed to get control channel");

    assert!(
        (value - 440.0).abs() < Myflt::EPSILON,
        "Control channel roundtrip failed: expected 440.0, got {}",
        value
    );
}

#[test]
fn test_control_channel_multiple_values() {
    let mut cs = create_test_csound();

    cs.compile_orc(CONTROL_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let test_values = [0.0, 100.0, 440.0, 880.0, -1.0, 1e6];

    for expected in test_values {
        cs.set_control_channel("freq", expected)
            .expect("Failed to set control channel");
        let actual = cs
            .get_control_channel("freq")
            .expect("Failed to get control channel");
        assert!(
            (actual - expected).abs() < Myflt::EPSILON,
            "Control channel mismatch: expected {}, got {}",
            expected,
            actual
        );
    }
}

#[test]
fn test_get_control_channel_before_start() {
    let cs = create_test_csound();

    // Try to get channel before compiling/starting - should fail or return 0
    // Different Csound versions may behave differently here
    let result = cs.get_control_channel("nonexistent_channel");
    // Just verify it doesn't panic - behavior varies by Csound version
    let _ = result;
}

// ============================================================================
// STRING CHANNEL TESTS
// ============================================================================

/// Orchestra that declares a string channel.
static STRING_CHANNEL_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

chn_S "message", 1  ; input string channel

instr 1
  Smsg chnget "message"
  prints Smsg
endin
"#;

#[test]
fn test_string_channel_roundtrip() {
    let mut cs = create_test_csound();

    cs.compile_orc(STRING_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let test_string = "Hello, Csound!";
    cs.set_string_channel("message", test_string)
        .expect("Failed to set string channel");

    let result = cs
        .get_string_channel("message")
        .expect("Failed to get string channel");

    assert_eq!(
        result, test_string,
        "String channel roundtrip failed: expected '{}', got '{}'",
        test_string, result
    );
}

#[test]
fn test_string_channel_empty_string() {
    let mut cs = create_test_csound();

    cs.compile_orc(STRING_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    // First set a non-empty value
    cs.set_string_channel("message", "initial")
        .expect("Failed to set string channel");

    // Then set empty string
    cs.set_string_channel("message", "")
        .expect("Failed to set empty string channel");

    let result = cs
        .get_string_channel("message")
        .expect("Failed to get string channel");

    assert!(
        result.is_empty(),
        "String channel should be empty, got '{}'",
        result
    );
}

#[test]
fn test_string_channel_unicode() {
    let mut cs = create_test_csound();

    cs.compile_orc(STRING_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let test_string = "こんにちは世界 🎵";
    cs.set_string_channel("message", test_string)
        .expect("Failed to set unicode string channel");

    let result = cs
        .get_string_channel("message")
        .expect("Failed to get string channel");

    assert_eq!(
        result, test_string,
        "Unicode string channel roundtrip failed"
    );
}

// ============================================================================
// AUDIO CHANNEL TESTS
// ============================================================================

/// Orchestra that declares audio channels.
static AUDIO_CHANNEL_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

chn_a "audio_in", 1   ; input audio channel
chn_a "audio_out", 2  ; output audio channel

instr 1
  ain chnget "audio_in"
  chnset ain * 2, "audio_out"
endin
"#;

#[test]
fn test_audio_channel_read_write() {
    let mut cs = create_test_csound();

    cs.compile_orc(AUDIO_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let ksmps = cs.get_ksmps() as usize;

    // Create input samples (sine wave fragment)
    let input: Vec<Myflt> = (0..ksmps)
        .map(|i| ((i as f64 * 0.1).sin() * 0.5) as Myflt)
        .collect();

    // Write to audio input channel
    cs.write_audio_channel("audio_in", &input)
        .expect("Failed to write audio channel");

    // Read from audio input channel to verify write
    let mut output = vec![0.0 as Myflt; ksmps];
    cs.read_audio_channel("audio_in", &mut output)
        .expect("Failed to read audio channel");

    // Verify the data matches
    for (i, (inp, out)) in input.iter().zip(output.iter()).enumerate() {
        assert!(
            (inp - out).abs() < (1e-6 as Myflt),
            "Audio channel mismatch at sample {}: expected {}, got {}",
            i,
            inp,
            out
        );
    }
}

#[test]
fn test_audio_channel_read_insufficient_buffer_returns_error() {
    let cs = create_test_csound();

    cs.compile_orc(AUDIO_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let ksmps = cs.get_ksmps() as usize;

    // Try to read into a buffer that's too small
    let mut small_buffer = vec![0.0 as Myflt; ksmps - 1];
    let result = cs.read_audio_channel("audio_in", &mut small_buffer);

    assert!(
        result.is_err(),
        "Reading audio channel with insufficient buffer should return error"
    );
}

#[test]
fn test_audio_channel_write_undersized_returns_error() {
    let mut cs = create_test_csound();

    cs.compile_orc(AUDIO_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let ksmps = cs.get_ksmps() as usize;

    // Try to write fewer samples than required
    let undersized = vec![0.0 as Myflt; ksmps - 1];
    let result = cs.write_audio_channel("audio_in", &undersized);

    assert!(
        result.is_err(),
        "Writing undersized audio channel should return error"
    );
}

// ============================================================================
// TABLE TESTS
// ============================================================================

/// Orchestra that creates a function table via ftgen.
static TABLE_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

; Create a sine table with 1024 points using GEN10
gi_sine ftgen 1, 0, 1024, 10, 1

; Create a smaller table for testing
gi_small ftgen 2, 0, 64, 7, 0, 32, 1, 32, 0

instr 1
  aout oscil 0.5, 440, gi_sine
  outs aout, aout
endin
"#;

#[test]
fn test_table_length() {
    let cs = create_test_csound();

    cs.compile_orc(TABLE_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    // Table 1 should have 1024 points
    let len = cs.table_length(1).expect("Failed to get table length");
    assert_eq!(len, 1024, "Table 1 should have 1024 points");

    // Table 2 should have 64 points
    let len = cs.table_length(2).expect("Failed to get table length");
    assert_eq!(len, 64, "Table 2 should have 64 points");
}

#[test]
fn test_table_length_nonexistent_returns_error() {
    let cs = create_test_csound();

    cs.compile_orc(TABLE_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let result = cs.table_length(999);
    assert!(
        result.is_err(),
        "Getting length of nonexistent table should return error"
    );
}

#[test]
fn test_get_table_read_data() {
    let cs = create_test_csound();

    cs.compile_orc(TABLE_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let table = cs.get_table(1).expect("Failed to get table 1");

    // Verify table size
    assert_eq!(table.get_size(), 1024, "Table should have 1024 points");

    // Sine table (GEN10 with single harmonic) should have specific properties
    // At index 0, sine should be 0
    // At index 256 (1/4 period), should be ~1.0
    // At index 512 (1/2 period), should be ~0
    // At index 768 (3/4 period), should be ~-1.0

    let data = table.as_slice();
    assert!(
        data[0].abs() < 0.01,
        "Sine table at 0 should be near 0, got {}",
        data[0]
    );
    assert!(
        (data[256] - 1.0).abs() < 0.01,
        "Sine table at 1/4 period should be near 1, got {}",
        data[256]
    );
    assert!(
        data[512].abs() < 0.01,
        "Sine table at 1/2 period should be near 0, got {}",
        data[512]
    );
    assert!(
        (data[768] + 1.0).abs() < 0.01,
        "Sine table at 3/4 period should be near -1, got {}",
        data[768]
    );
}

#[test]
fn test_table_write_and_read_cycle() {
    let cs = create_test_csound();

    cs.compile_orc(TABLE_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let table = cs.get_table(2).expect("Failed to get table 2");
    let size = table.get_size();

    // Create test data
    let test_data: Vec<Myflt> = (0..size).map(|i| ((i as f64) * 0.1) as Myflt).collect();

    // Write to table using copy_from_slice
    let copied = table.copy_from_slice(&test_data);
    assert_eq!(copied, size, "Should copy all {} elements", size);

    // Read back and verify
    let mut read_back = vec![0.0 as Myflt; size];
    let read_count = table.copy_to_slice(&mut read_back);
    assert_eq!(read_count, size, "Should read all {} elements", size);

    for (i, (expected, actual)) in test_data.iter().zip(read_back.iter()).enumerate() {
        assert!(
            (expected - actual).abs() < Myflt::EPSILON,
            "Table data mismatch at index {}: expected {}, got {}",
            i,
            expected,
            actual
        );
    }
}

#[test]
fn test_table_direct_slice_access() {
    let cs = create_test_csound();

    cs.compile_orc(TABLE_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let mut table = cs.get_table(2).expect("Failed to get table 2");

    // Write directly via mutable slice
    {
        let slice = table.as_mut_slice();
        for (i, val) in slice.iter_mut().enumerate() {
            *val = ((i as f64).powi(2)) as Myflt;
        }
    }

    // Read back via immutable slice
    let slice = table.as_slice();
    for (i, val) in slice.iter().enumerate() {
        let expected = ((i as f64).powi(2)) as Myflt;
        assert!(
            (val - expected).abs() < Myflt::EPSILON,
            "Direct slice access mismatch at {}: expected {}, got {}",
            i,
            expected,
            val
        );
    }
}

#[test]
fn test_get_table_nonexistent_returns_none() {
    let cs = create_test_csound();

    cs.compile_orc(TABLE_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let result = cs.get_table(999);
    assert!(
        result.is_none(),
        "Getting nonexistent table should return None"
    );
}

#[test]
fn test_get_table_args() {
    let cs = create_test_csound();

    cs.compile_orc(TABLE_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    // Table 1: ftgen 1, 0, 1024, 10, 1
    // Args should be [10.0, 1.0] (GEN number followed by parameters)
    let args = cs.get_table_args(1).expect("Failed to get table args");
    assert!(
        args.len() >= 2,
        "Should have at least GEN and one parameter"
    );
    assert!(
        (args[0] - 10.0).abs() < Myflt::EPSILON,
        "GEN should be 10, got {}",
        args[0]
    );
    assert!(
        (args[1] - 1.0).abs() < Myflt::EPSILON,
        "First harmonic should be 1, got {}",
        args[1]
    );
}

// ============================================================================
// MESSAGE BUFFER TESTS
// ============================================================================

#[test]
fn test_message_buffer_create_and_drain() {
    let mut cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");
    cs.set_option("-d").expect("Failed to set -d option");

    // Create message buffer BEFORE compiling (to capture compile messages)
    cs.create_message_buffer(0);

    // Compile something that generates messages
    cs.compile_orc(CONTROL_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    // Drain messages using count to avoid calling get_first_message on empty buffer
    let mut messages = Vec::new();
    while cs.get_message_count().unwrap_or(0) > 0 {
        if let Some(msg) = cs.get_first_message() {
            messages.push(msg);
        }
        cs.pop_first_message();
    }

    // After draining, count should be 0
    assert_eq!(
        cs.get_message_count().unwrap_or(0),
        0,
        "Message buffer should be empty after draining"
    );
}

#[test]
fn test_message_buffer_attributes() {
    let mut cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");

    cs.create_message_buffer(0);

    // Compile to generate messages
    let _ = cs.compile_orc(CONTROL_CHANNEL_ORC, 0);
    let _ = cs.start();

    // Check that we can get message attributes
    if cs.get_message_count().unwrap_or(0) > 0 {
        let _attr = cs.get_first_message_attr();
        // Just verify it returns without panicking - any MessageType variant is valid
    }
}

// ============================================================================
// LIST CHANNELS TEST
// ============================================================================

#[test]
fn test_list_channels() {
    let mut cs = create_test_csound();

    cs.compile_orc(CONTROL_CHANNEL_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    // Set a channel value to ensure it's registered
    cs.set_control_channel("freq", 440.0)
        .expect("Failed to set control channel");

    let channels = cs.list_channels().expect("Failed to list channels");

    // We declared "freq" channel in the orchestra
    let freq_channel = channels.iter().find(|c| c.name == "freq");
    assert!(
        freq_channel.is_some(),
        "Should find 'freq' channel in list, found: {:?}",
        channels.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
}

// ============================================================================
// CHANNEL HINTS TESTS
// ============================================================================

/// Orchestra with channel hints.
static CHANNEL_HINTS_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

chn_k "volume", 1, 2, 0.5, 0.0, 1.0  ; input, linear, default 0.5, range 0-1

instr 1
  kvol chnget "volume"
  aout oscil kvol, 440
  outs aout, aout
endin
"#;

#[test]
fn test_get_channel_hints() {
    let cs = create_test_csound();

    cs.compile_orc(CHANNEL_HINTS_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let hints = cs
        .get_channel_hints("volume")
        .expect("Failed to get channel hints");

    assert!(
        (hints.dflt - 0.5).abs() < Myflt::EPSILON,
        "Default should be 0.5, got {}",
        hints.dflt
    );
    assert!(
        hints.min.abs() < Myflt::EPSILON,
        "Min should be 0.0, got {}",
        hints.min
    );
    assert!(
        (hints.max - 1.0).abs() < Myflt::EPSILON,
        "Max should be 1.0, got {}",
        hints.max
    );
}
