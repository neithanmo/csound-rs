//! Integration tests for PVS channels.
//!
//! Focuses on basic init + read/write behavior and parameter mismatch.

use csound::{Csound, Error, MessageType, PvsChannelParams, PvsFormat, PvsWindowType};

static ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 1
0dbfs = 1

instr 1
endin
"#;

fn create_test_csound() -> Csound {
    let cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");
    cs.set_option("-d").expect("Failed to set -d option");
    cs.set_option("-m0").expect("Failed to set -m0 option");
    cs.message_string_callback(|_: MessageType, _: &str| {});
    cs
}

#[test]
fn test_pvs_channel_init_roundtrip() {
    let cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let params = PvsChannelParams::new(1024, 256, 1024, PvsWindowType::Hann, PvsFormat::AmpFreq);

    let channel = cs
        .init_pvs_channel("pvs_roundtrip", params)
        .expect("Failed to initialize PVS channel");

    let info = channel.info();
    assert_eq!(info.fft_size, 1024);
    assert_eq!(info.overlap, 256);
    assert_eq!(info.window_size, 1024);
    assert_eq!(info.format, PvsFormat::AmpFreq);
    assert_eq!(info.frame_len, 1026);

    let input: Vec<f32> = (0..info.frame_len).map(|i| i as f32 * 0.5).collect();

    channel.with_lock(|mut lock| {
        let written = lock.write(&input);
        assert_eq!(written, input.len());
    });

    let output = channel.with_lock(|lock| lock.read_frame());
    assert_eq!(output.frame_len(), input.len());
    for (got, expected) in output.frame.iter().zip(input.iter()) {
        assert!((got - expected).abs() < 1e-6);
    }
}

#[test]
fn test_pvs_channel_init_param_mismatch() {
    let cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");

    let params = PvsChannelParams::new(1024, 256, 1024, PvsWindowType::Hann, PvsFormat::AmpFreq);
    cs.init_pvs_channel("pvs_mismatch", params)
        .expect("Failed to initialize PVS channel");

    let mismatch = PvsChannelParams::new(512, 128, 512, PvsWindowType::Hann, PvsFormat::AmpFreq);

    let err = cs
        .init_pvs_channel("pvs_mismatch", mismatch)
        .expect_err("Expected init to fail due to parameter mismatch");

    assert!(matches!(err, Error::InvalidArgument(_)));
}
