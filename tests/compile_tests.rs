//! Integration tests for orchestra and score compilation.
//!
//! These tests verify that Csound correctly compiles valid orchestra/score code
//! and returns appropriate errors for malformed input.

use csound::{Csound, MessageType};

/// Creates a Csound instance configured for testing.
///
/// This sets up:
/// - No audio output (`-n` option)
/// - Suppressed displays (`-d` option)
/// - Minimal message level (`-m0` option)
/// - Empty message callback to suppress remaining messages
///
/// Note: Some cleanup messages (e.g., "overall amps") from Csound 7 are printed
/// directly during csoundDestroy/csoundReset and cannot be suppressed via the API.
fn create_test_csound() -> Csound {
    let cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");
    cs.set_option("-d").expect("Failed to set -d option");
    cs.set_option("-m0").expect("Failed to set -m0 option");
    // Suppress most Csound messages by setting an empty callback
    cs.message_string_callback(|_: MessageType, _: &str| {});
    cs
}

/// A valid orchestra that compiles successfully.
static VALID_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  aout oscil 0.5, 440
  outs aout, aout
endin
"#;

/// A valid score that works with VALID_ORC.
static VALID_SCO: &str = r#"
i1 0 1
e
"#;

/// An orchestra with syntax errors (missing 'endin').
static MALFORMED_ORC_MISSING_ENDIN: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  aout oscil 0.5, 440
  outs aout, aout
"#;

/// An orchestra with invalid opcode.
static MALFORMED_ORC_INVALID_OPCODE: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  aout nonexistent_opcode 0.5, 440
  outs aout, aout
endin
"#;

/// An orchestra with invalid header value.
static MALFORMED_ORC_INVALID_HEADER: &str = r#"
sr = not_a_number
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  aout oscil 0.5, 440
  outs aout, aout
endin
"#;

/// Two Csound instances in one process. The second `Csound::new` calls
/// `csoundInitialize` again and gets a positive "already initialized"
/// status; that must not be treated as `InitFailed`.
///
/// Stuffed VST3s are a stronger version of this (separate copies of the
/// crate, one process-wide Csound library). This test covers the same
/// return mapping inside a single crate.
#[test]
fn two_csound_instances_after_library_already_initialized() {
    let a = create_test_csound();
    let b = create_test_csound();
    drop((a, b));
}

/// Test that a valid orchestra compiles successfully.
#[test]
fn test_compile_valid_orchestra() {
    let cs = create_test_csound();

    let result = cs.compile_orc(VALID_ORC, 0);
    assert!(
        result.is_ok(),
        "Valid orchestra should compile successfully"
    );
}

/// Test that a valid orchestra and score can be compiled and started.
#[test]
fn test_compile_valid_orchestra_and_score() {
    let cs = create_test_csound();

    cs.compile_orc(VALID_ORC, 0)
        .expect("Failed to compile orchestra");

    cs.start().expect("Failed to start Csound");

    // Send score events
    let result = cs.send_string_event(VALID_SCO, 0);
    assert!(
        result.is_ok(),
        "Valid score should be accepted: {:?}",
        result
    );

    // Run a few k-cycles to verify performance works
    for _ in 0..10 {
        if cs.perform_ksmps() {
            break;
        }
    }
}

/// Test that an orchestra with missing 'endin' fails to compile.
#[test]
fn test_compile_orchestra_missing_endin_returns_error() {
    let cs = create_test_csound();

    let result = cs.compile_orc(MALFORMED_ORC_MISSING_ENDIN, 0);
    assert!(
        result.is_err(),
        "Orchestra with missing 'endin' should fail to compile"
    );
}

/// Test that an orchestra with an invalid opcode fails to compile.
#[test]
fn test_compile_orchestra_invalid_opcode_returns_error() {
    let cs = create_test_csound();

    let result = cs.compile_orc(MALFORMED_ORC_INVALID_OPCODE, 0);
    assert!(
        result.is_err(),
        "Orchestra with invalid opcode should fail to compile"
    );
}

/// Test that an orchestra with an invalid header value fails to compile.
#[test]
fn test_compile_orchestra_invalid_header_returns_error() {
    let cs = create_test_csound();

    let result = cs.compile_orc(MALFORMED_ORC_INVALID_HEADER, 0);
    assert!(
        result.is_err(),
        "Orchestra with invalid header should fail to compile"
    );
}

/// Test that compiling an empty orchestra returns an error.
#[test]
fn test_compile_empty_orchestra_returns_error() {
    let cs = create_test_csound();

    let result = cs.compile_orc("", 0);
    assert!(result.is_err(), "Empty orchestra should return an error");
}

/// Test that sending an empty score string returns an error.
#[test]
fn test_send_empty_score_returns_error() {
    let cs = create_test_csound();

    cs.compile_orc(VALID_ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let result = cs.send_string_event("", 0);
    assert!(result.is_err(), "Empty score string should return an error");
}

/// Test compiling a CSD from text (mode=1) with valid content.
#[test]
fn test_compile_csd_from_text_valid() {
    let cs = create_test_csound();

    let valid_csd = r#"
<CsoundSynthesizer>
<CsOptions>
-n
</CsOptions>
<CsInstruments>
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  aout oscil 0.5, 440
  outs aout, aout
endin
</CsInstruments>
<CsScore>
i1 0 0.1
e
</CsScore>
</CsoundSynthesizer>
"#;

    // mode=1 means compile from text string, not file
    let result = cs.compile_csd(valid_csd, 1, 0);
    assert!(
        result.is_ok(),
        "Valid CSD text should compile successfully: {:?}",
        result
    );
}

/// Test compiling a CSD from text with malformed content.
#[test]
fn test_compile_csd_from_text_malformed_returns_error() {
    let cs = create_test_csound();

    let malformed_csd = r#"
<CsoundSynthesizer>
<CsOptions>
-n
</CsOptions>
<CsInstruments>
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  aout nonexistent_opcode 0.5, 440
  outs aout, aout
endin
</CsInstruments>
<CsScore>
i1 0 0.1
e
</CsScore>
</CsoundSynthesizer>
"#;

    let result = cs.compile_csd(malformed_csd, 1, 0);
    assert!(result.is_err(), "Malformed CSD text should fail to compile");
}

/// Test that compiling a non-existent CSD file returns an error.
#[test]
fn test_compile_csd_nonexistent_file_returns_error() {
    let cs = create_test_csound();

    // mode=0 means compile from file
    let result = cs.compile_csd("/nonexistent/path/to/file.csd", 0, 0);
    assert!(
        result.is_err(),
        "Non-existent CSD file should fail to compile"
    );
}

/// Test that compiling an empty CSD path returns an error.
#[test]
fn test_compile_csd_empty_path_returns_error() {
    let cs = create_test_csound();

    let result = cs.compile_csd("", 0, 0);
    assert!(result.is_err(), "Empty CSD path should return an error");
}

/// Test that get_opcode_list_entry returns a non-empty list of opcodes.
#[test]
fn test_get_opcode_list_entry() {
    let cs = create_test_csound();

    // Compile something first to ensure externals are loaded
    cs.compile_orc(VALID_ORC, 0)
        .expect("Failed to compile orchestra");

    let opcodes = cs
        .get_opcode_list_entry()
        .expect("Failed to get opcode list");

    // Csound should have many built-in opcodes
    assert!(
        !opcodes.is_empty(),
        "Opcode list should not be empty after compilation"
    );

    // Verify we have some well-known opcodes
    let has_oscil = opcodes.iter().any(|op| op.opname == "oscil");
    let has_outs = opcodes.iter().any(|op| op.opname == "outs");

    assert!(has_oscil, "Opcode list should contain 'oscil'");
    assert!(has_outs, "Opcode list should contain 'outs'");

    // Verify structure: opname should never be empty, outypes/intypes can be None
    for opcode in &opcodes {
        assert!(!opcode.opname.is_empty(), "Opcode name should not be empty");
    }
}

/// Test that get_audio_devices returns valid device lists.
///
/// Note: This test attempts to set portaudio as the RT module.
/// If portaudio is not available, the test will pass with 0 devices.
#[test]
fn test_get_audio_devices() {
    let cs = create_test_csound();

    // Try to set portaudio module (most widely available)
    // If it fails, we'll just get 0 devices (acceptable in CI)
    let _ = cs.set_rt_audio_module("portaudio");

    let (input_devices, output_devices) = cs.get_audio_devices().unwrap();

    // Note: The number of devices can be 0 on systems without audio hardware,
    // in CI environments, or if the RT module isn't available

    // Verify input devices have is_output = 0
    for device in &input_devices {
        assert_eq!(
            device.is_output, 0,
            "Input audio device should have is_output = 0"
        );
        // Verify device has some identifying information
        assert!(
            !device.device_name.is_empty() || !device.device_id.is_empty(),
            "Device should have at least a name or ID"
        );
    }

    // Verify output devices have is_output = 1
    for device in &output_devices {
        assert_eq!(
            device.is_output, 1,
            "Output audio device should have is_output = 1"
        );
        // Verify device has some identifying information
        assert!(
            !device.device_name.is_empty() || !device.device_id.is_empty(),
            "Device should have at least a name or ID"
        );
    }

    println!(
        "Found {} input audio devices and {} output audio devices",
        input_devices.len(),
        output_devices.len()
    );
}

/// Test that get_midi_devices returns valid device lists.
///
/// Note: This test attempts to set portmidi as the MIDI module.
/// If portmidi is not available, the test will pass with 0 devices.
#[test]
fn test_get_midi_devices() {
    let cs = create_test_csound();

    // Try to set portmidi module (most widely available)
    // If it fails, we'll just get 0 devices (acceptable in CI)
    cs.set_midi_module("portmidi")
        .expect("module name should be a valid C string");

    let (input_devices, output_devices) = cs.get_midi_devices().unwrap();

    // Note: The number of devices can be 0 on systems without MIDI hardware,
    // in CI environments, or if the MIDI module isn't available

    // Verify input devices have is_output = 0
    for device in &input_devices {
        assert_eq!(
            device.is_output, 0,
            "Input MIDI device should have is_output = 0"
        );
        // Verify device has some identifying information
        assert!(
            !device.device_name.is_empty() || !device.device_id.is_empty(),
            "Device should have at least a name or ID"
        );
    }

    // Verify output devices have is_output = 1
    for device in &output_devices {
        assert_eq!(
            device.is_output, 1,
            "Output MIDI device should have is_output = 1"
        );
        // Verify device has some identifying information
        assert!(
            !device.device_name.is_empty() || !device.device_id.is_empty(),
            "Device should have at least a name or ID"
        );
    }

    println!(
        "Found {} input MIDI devices and {} output MIDI devices",
        input_devices.len(),
        output_devices.len()
    );
}
