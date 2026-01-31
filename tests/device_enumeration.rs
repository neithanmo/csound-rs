//! Integration tests for device enumeration.
//!
//! These tests verify that audio and MIDI device enumeration works correctly
//! and can find devices on systems with actual hardware.

use csound::Csound;

/// Test audio device enumeration with different RT audio modules.
#[test]
fn test_audio_devices_with_different_modules() {
    let cs = Csound::new().expect("Failed to create Csound instance");

    // Try with no explicit module first (default)
    println!("\n=== Testing with default audio module ===");
    let (input_default, output_default) = cs.get_audio_devices().unwrap();
    println!(
        "Default module: {} input, {} output devices",
        input_default.len(),
        output_default.len()
    );

    if !input_default.is_empty() {
        println!("Input devices:");
        for dev in &input_default {
            println!("  - {:?}", dev);
        }
    }

    if !output_default.is_empty() {
        println!("Output devices:");
        for dev in &output_default {
            println!("  - {:?}", dev);
        }
    }

    // Try common audio modules
    let modules_to_try = vec![
        "portaudio",
        "alsa",
        "pulse",
        "jack",
        "auhal",     // macOS
        "coreaudio", // macOS
    ];

    for module in modules_to_try {
        println!("\n=== Testing with '{}' audio module ===", module);

        // Create a fresh Csound instance for each module
        let cs_test = Csound::new().expect("Failed to create Csound instance");

        // Try to set the module (may fail if not available)
        if cs_test.set_rt_audio_module(module).is_ok() {
            let (input_devs, output_devs) = cs_test.get_audio_devices().unwrap();
            println!(
                "{}: {} input, {} output devices",
                module,
                input_devs.len(),
                output_devs.len()
            );

            if !input_devs.is_empty() {
                println!("  Input devices:");
                for dev in &input_devs {
                    println!(
                        "    - Name: {:?}, ID: {:?}, Module: {:?}",
                        dev.device_name, dev.device_id, dev.rt_module
                    );
                }
            }

            if !output_devs.is_empty() {
                println!("  Output devices:");
                for dev in &output_devs {
                    println!(
                        "    - Name: {:?}, ID: {:?}, Module: {:?}",
                        dev.device_name, dev.device_id, dev.rt_module
                    );
                }
            }
        } else {
            println!("  Module '{}' not available or failed to set", module);
        }
    }
}

/// Test MIDI device enumeration with different MIDI modules.
#[test]
fn test_midi_devices_with_different_modules() {
    let cs = Csound::new().expect("Failed to create Csound instance");

    // Try with default module first
    println!("\n=== Testing with default MIDI module ===");
    let (input_default, output_default) = cs.get_midi_devices().unwrap();
    println!(
        "Default module: {} input, {} output MIDI devices",
        input_default.len(),
        output_default.len()
    );

    if !input_default.is_empty() {
        println!("Input MIDI devices:");
        for dev in &input_default {
            println!("  - {:?}", dev);
        }
    }

    if !output_default.is_empty() {
        println!("Output MIDI devices:");
        for dev in &output_default {
            println!("  - {:?}", dev);
        }
    }

    // Try common MIDI modules
    let modules_to_try = vec![
        "portmidi", "alsa", "winmme",   // Windows
        "coremidi", // macOS
    ];

    for module in modules_to_try {
        println!("\n=== Testing with '{}' MIDI module ===", module);

        let cs_test = Csound::new().expect("Failed to create Csound instance");
        cs_test.set_midi_module(module);

        let (input_devs, output_devs) = cs_test.get_midi_devices().unwrap();
        println!(
            "{}: {} input, {} output MIDI devices",
            module,
            input_devs.len(),
            output_devs.len()
        );

        if !input_devs.is_empty() {
            println!("  Input MIDI devices:");
            for dev in &input_devs {
                println!(
                    "    - Name: {:?}, ID: {:?}, Module: {:?}",
                    dev.device_name, dev.device_id, dev.midi_module
                );
            }
        }

        if !output_devs.is_empty() {
            println!("  Output MIDI devices:");
            for dev in &output_devs {
                println!(
                    "    - Name: {:?}, ID: {:?}, Module: {:?}",
                    dev.device_name, dev.device_id, dev.midi_module
                );
            }
        }
    }
}

/// Quick test to see if any devices are found at all.
#[test]
fn test_has_any_devices() {
    let cs = Csound::new().expect("Failed to create Csound instance");

    let (audio_in, audio_out) = cs.get_audio_devices().unwrap();
    let (midi_in, midi_out) = cs.get_midi_devices().unwrap();

    let total_devices = audio_in.len() + audio_out.len() + midi_in.len() + midi_out.len();

    println!("\nTotal devices found: {}", total_devices);
    println!("  Audio input:  {}", audio_in.len());
    println!("  Audio output: {}", audio_out.len());
    println!("  MIDI input:   {}", midi_in.len());
    println!("  MIDI output:  {}", midi_out.len());

    if total_devices == 0 {
        println!("\nWARNING: No devices found with default configuration!");
        println!("This could mean:");
        println!("  1. Csound needs a specific RT audio/MIDI module configured");
        println!("  2. The system has no audio/MIDI hardware");
        println!("  3. Permissions issues preventing device access");
        println!("\nTry running the other device enumeration tests to see which modules work.");
    }
}
