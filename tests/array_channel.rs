//! Integration tests for array channels (Csound 7).
//!
//! Covers the host-side API surface, the argument validation that keeps
//! `csoundSetArrayData` from reading past a short buffer, and round-trip
//! interop with orchestra `chnget`/`chnset` on array types.

use csound::{ArrayType, Csound, Error, MessageType, ScoreEventType};

const KSMPS: u32 = 32;

static ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 1
0dbfs = 1

instr 1
endin
"#;

/// Orchestra exercising both directions of array-channel transfer.
///
/// `instr 10` reads the host-written array and republishes element 0 on a
/// control channel. `instr 11` writes an array from the orchestra side.
static INTEROP_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 1
0dbfs = 1

instr 10
  kArr[] chnget "host_to_orc"
  chnset kArr[0], "readback0"
  chnset kArr[3], "readback3"
endin

instr 11
  kOut[] init 4
  kOut[0] = 42
  kOut[1] = 43
  kOut[2] = 44
  kOut[3] = 45
  chnset kOut, "orc_to_host"
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

fn started_csound(orc: &str) -> Csound {
    let cs = create_test_csound();
    cs.compile_orc(orc, 0).expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");
    cs
}

// ---------------------------------------------------------------------------
// Shape and metadata
// ---------------------------------------------------------------------------

#[test]
fn k_array_roundtrip() {
    let cs = started_csound(ORC);
    let chan = cs
        .init_array_channel("k_roundtrip", "k", &[4])
        .expect("init failed");

    chan.set_data(&[1.0, 2.0, 3.0, 4.0])
        .expect("set_data failed");
    assert_eq!(chan.read_all().unwrap(), vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn i_array_roundtrip() {
    let cs = started_csound(ORC);
    let chan = cs
        .init_array_channel("i_roundtrip", "i", &[3])
        .expect("init failed");

    let info = chan.info();
    assert_eq!(info.array_type, ArrayType::Init);
    assert_eq!(info.len, Some(3));

    chan.set_data(&[7.0, 8.0, 9.0]).expect("set_data failed");
    assert_eq!(chan.read_all().unwrap(), vec![7.0, 8.0, 9.0]);
}

#[test]
fn audio_array_element_is_ksmps_wide() {
    let cs = started_csound(ORC);
    assert_eq!(cs.get_ksmps(), KSMPS);

    let chan = cs
        .init_array_channel("a_array", "a", &[2])
        .expect("init failed");

    let info = chan.info();
    assert_eq!(info.array_type, ArrayType::Audio);
    assert_eq!(info.element_count, 2);
    // Each element is one ksmps-wide audio vector.
    assert_eq!(info.len, Some(2 * KSMPS as usize));

    let data: Vec<f64> = (0..2 * KSMPS as usize).map(|i| i as f64).collect();
    chan.set_data(&data).expect("set_data failed");
    assert_eq!(chan.read_all().unwrap(), data);
}

#[test]
fn multidimensional_shape() {
    let cs = started_csound(ORC);
    let chan = cs
        .init_array_channel("multi", "k", &[2, 3])
        .expect("init failed");

    let info = chan.info();
    assert_eq!(info.dimensions, 2);
    assert_eq!(info.sizes, vec![2, 3]);
    assert_eq!(info.element_count, 6);
    assert_eq!(info.len, Some(6));

    let data: Vec<f64> = (0..6).map(|i| i as f64 * 1.5).collect();
    chan.set_data(&data).expect("set_data failed");
    assert_eq!(chan.read_all().unwrap(), data);
}

#[test]
fn info_reports_type_name() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("typed", "k", &[1]).unwrap();
    assert_eq!(chan.array_type(), ArrayType::Control);
    assert_eq!(chan.array_type().as_str(), "k");
    assert!(chan.array_type().is_numeric());
}

// ---------------------------------------------------------------------------
// Length checking around csoundSetArrayData
// ---------------------------------------------------------------------------

#[test]
fn set_data_rejects_short_slice() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("short", "k", &[4]).unwrap();

    // A short slice would make Csound memcpy past the end of our buffer.
    let err = chan
        .set_data(&[1.0, 2.0])
        .expect_err("short slice must be rejected");

    match err {
        Error::InsufficientCapacity { expected, actual } => {
            assert_eq!(expected, 4);
            assert_eq!(actual, 2);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn set_data_rejects_long_slice() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("long", "k", &[2]).unwrap();

    let err = chan
        .set_data(&[1.0, 2.0, 3.0])
        .expect_err("long slice must be rejected");
    assert!(matches!(err, Error::InsufficientCapacity { .. }));
}

#[test]
fn set_data_accepts_exact_length_only() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("exact", "k", &[3]).unwrap();

    assert!(chan.set_data(&[1.0, 2.0]).is_err());
    assert!(chan.set_data(&[1.0, 2.0, 3.0, 4.0]).is_err());
    assert!(chan.set_data(&[1.0, 2.0, 3.0]).is_ok());
}

// ---------------------------------------------------------------------------
// Managed element types
// ---------------------------------------------------------------------------

#[test]
fn string_array_rejects_numeric_access() {
    let cs = started_csound(ORC);
    let chan = cs
        .init_array_channel("strings", "S", &[2])
        .expect("init failed");

    assert_eq!(chan.array_type(), ArrayType::Str);
    assert!(!chan.array_type().is_numeric());

    // len() is None: a STRINGDAT member is not a run of samples.
    assert_eq!(chan.info().len, None);

    assert!(matches!(
        chan.read_all().unwrap_err(),
        Error::InvalidArgument(_)
    ));
    assert!(matches!(
        chan.set_data(&[1.0, 2.0]).unwrap_err(),
        Error::InvalidArgument(_)
    ));
}

#[test]
fn string_array_slice_access_is_rejected_under_lock() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("strings2", "S", &[1]).unwrap();

    chan.with_lock(|mut lock| {
        assert!(matches!(
            lock.as_slice().unwrap_err(),
            Error::InvalidArgument(_)
        ));
        assert!(matches!(
            lock.as_mut_slice().unwrap_err(),
            Error::InvalidArgument(_)
        ));
    });
}

// ---------------------------------------------------------------------------
// Argument validation
// ---------------------------------------------------------------------------

#[test]
fn empty_name_rejected() {
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.init_array_channel("", "k", &[1]).unwrap_err(),
        Error::EmptyString
    ));
    assert!(matches!(
        cs.get_array_channel("").unwrap_err(),
        Error::EmptyString
    ));
}

#[test]
fn empty_type_rejected() {
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.init_array_channel("t", "", &[1]).unwrap_err(),
        Error::EmptyString
    ));
}

#[test]
fn empty_sizes_rejected() {
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.init_array_channel("nodims", "k", &[]).unwrap_err(),
        Error::InvalidArgument(_)
    ));
}

#[test]
fn negative_size_rejected() {
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.init_array_channel("neg", "k", &[-1]).unwrap_err(),
        Error::InvalidArgument(_)
    ));
    assert!(matches!(
        cs.init_array_channel("neg2", "k", &[2, -3]).unwrap_err(),
        Error::InvalidArgument(_)
    ));
}

#[test]
fn interior_nul_in_name_rejected() {
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.init_array_channel("bad\0name", "k", &[1]).unwrap_err(),
        Error::Nul(_)
    ));
}

#[test]
fn unknown_element_type_rejected() {
    let cs = started_csound(ORC);
    // No such Csound type name; the engine cannot resolve it and returns NULL.
    let err = cs
        .init_array_channel("weird", "not_a_type", &[1])
        .expect_err("unknown type must fail");
    assert!(matches!(err, Error::NullPointer(_)));
}

#[test]
fn zero_size_dimension_yields_an_empty_array() {
    let cs = started_csound(ORC);
    // A zero-size dimension is accepted and produces a valid but empty array
    // rather than an error, so the wrapper must report a length of 0 and not
    // attempt to build a slice over it.
    let chan = cs
        .init_array_channel("zero", "k", &[0])
        .expect("zero-size dimension is accepted by Csound");

    let info = chan.info();
    assert_eq!(info.dimensions, 1);
    assert_eq!(info.sizes, vec![0]);
    assert_eq!(info.element_count, 0);
    assert_eq!(info.len, Some(0));

    assert!(chan.read_all().unwrap().is_empty());
    chan.with_lock(|lock| assert!(lock.is_empty()));

    // An empty write is the only length that matches.
    assert!(chan.set_data(&[]).is_ok());
    assert!(matches!(
        chan.set_data(&[1.0]).unwrap_err(),
        Error::InsufficientCapacity { .. }
    ));
}

// ---------------------------------------------------------------------------
// Channel identity and lifecycle
// ---------------------------------------------------------------------------

#[test]
fn reinit_of_initialized_channel_is_a_noop() {
    let cs = started_csound(ORC);
    let first = cs.init_array_channel("reinit", "k", &[4]).unwrap();
    first.set_data(&[1.0, 2.0, 3.0, 4.0]).unwrap();

    // Documented Csound behaviour: an already-initialized channel is returned
    // unchanged, even when a different shape is requested.
    let second = cs.init_array_channel("reinit", "k", &[8]).unwrap();
    assert_eq!(second.info().sizes, vec![4]);
    assert_eq!(second.read_all().unwrap(), vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn get_array_channel_finds_initialized_channel() {
    let cs = started_csound(ORC);
    let created = cs.init_array_channel("findme", "k", &[3]).unwrap();
    created.set_data(&[5.0, 6.0, 7.0]).unwrap();

    let fetched = cs.get_array_channel("findme").expect("get failed");
    assert_eq!(fetched.name().unwrap(), "findme");
    assert_eq!(fetched.read_all().unwrap(), vec![5.0, 6.0, 7.0]);
}

#[test]
fn get_uninitialized_array_channel_is_rejected() {
    let cs = started_csound(ORC);
    // csoundGetChannelPtr creates the channel entry without allocating the
    // array. Reading the element type in that state dereferences a NULL
    // `arrayType` inside Csound, so the handle must be refused up front.
    let err = cs
        .get_array_channel("never_initialized")
        .expect_err("uninitialized array channel must be rejected");
    assert!(matches!(err, Error::BufferNotInitialized(_)));
}

#[test]
fn uninitialized_then_initialized_channel_becomes_usable() {
    let cs = started_csound(ORC);
    assert!(cs.get_array_channel("late_init").is_err());

    let chan = cs
        .init_array_channel("late_init", "k", &[2])
        .expect("init after a bare get should succeed");
    chan.set_data(&[3.0, 4.0]).unwrap();

    assert_eq!(
        cs.get_array_channel("late_init")
            .unwrap()
            .read_all()
            .unwrap(),
        vec![3.0, 4.0]
    );
}

#[test]
fn type_mismatch_with_existing_control_channel() {
    let cs = started_csound(ORC);
    cs.set_control_channel("scalar_chan", 1.0)
        .expect("failed to create control channel");

    let err = cs
        .get_array_channel("scalar_chan")
        .expect_err("expected a type mismatch");
    assert!(matches!(err, Error::ChannelTypeMismatch(_)));
}

// ---------------------------------------------------------------------------
// Guarded access
// ---------------------------------------------------------------------------

#[test]
fn write_through_mut_slice() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("mutslice", "k", &[4]).unwrap();

    chan.with_lock(|mut lock| {
        let slice = lock.as_mut_slice().unwrap();
        assert_eq!(slice.len(), 4);
        for (i, v) in slice.iter_mut().enumerate() {
            *v = (i as f64) * 10.0;
        }
    });

    assert_eq!(chan.read_all().unwrap(), vec![0.0, 10.0, 20.0, 30.0]);
}

#[test]
fn partial_read_into_smaller_buffer() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("partial", "k", &[4]).unwrap();
    chan.set_data(&[1.0, 2.0, 3.0, 4.0]).unwrap();

    let mut out = [0.0f64; 2];
    let n = chan.with_lock(|lock| lock.read(&mut out).unwrap());
    assert_eq!(n, 2);
    assert_eq!(out, [1.0, 2.0]);
}

#[test]
fn write_helper_copies_min_length() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("writemin", "k", &[4]).unwrap();
    chan.set_data(&[9.0, 9.0, 9.0, 9.0]).unwrap();

    let n = chan.with_lock(|mut lock| lock.write(&[1.0, 2.0]).unwrap());
    assert_eq!(n, 2);
    assert_eq!(chan.read_all().unwrap(), vec![1.0, 2.0, 9.0, 9.0]);
}

#[test]
fn sequential_locks_do_not_deadlock() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("seqlock", "k", &[2]).unwrap();

    for i in 0..16 {
        chan.with_lock(|mut lock| {
            lock.write(&[i as f64, i as f64]).unwrap();
        });
        let got = chan.read_all().unwrap();
        assert_eq!(got, vec![i as f64, i as f64]);
    }
}

#[test]
fn is_empty_reflects_length() {
    let cs = started_csound(ORC);
    let chan = cs.init_array_channel("nonempty", "k", &[2]).unwrap();
    chan.with_lock(|lock| {
        assert!(!lock.is_empty());
        assert_eq!(lock.len(), Some(2));
    });
}

// ---------------------------------------------------------------------------
// Orchestra interop
// ---------------------------------------------------------------------------

#[test]
fn host_writes_orchestra_reads() {
    let cs = started_csound(INTEROP_ORC);

    let chan = cs
        .init_array_channel("host_to_orc", "k", &[4])
        .expect("init failed");
    chan.set_data(&[11.0, 12.0, 13.0, 14.0]).unwrap();

    cs.send_score_event(ScoreEventType::Instrument, &[10.0, 0.0, 1.0]);
    for _ in 0..8 {
        if cs.perform_ksmps() {
            break;
        }
    }

    assert_eq!(cs.get_control_channel("readback0").unwrap(), 11.0);
    assert_eq!(cs.get_control_channel("readback3").unwrap(), 14.0);
}

#[test]
fn orchestra_writes_host_reads() {
    let cs = started_csound(INTEROP_ORC);

    cs.send_score_event(ScoreEventType::Instrument, &[11.0, 0.0, 1.0]);
    for _ in 0..8 {
        if cs.perform_ksmps() {
            break;
        }
    }

    let chan = cs
        .get_array_channel("orc_to_host")
        .expect("channel should exist after the orchestra wrote it");
    assert_eq!(chan.array_type(), ArrayType::Control);
    assert_eq!(chan.read_all().unwrap(), vec![42.0, 43.0, 44.0, 45.0]);
}

#[test]
fn host_update_is_visible_to_orchestra_across_k_periods() {
    let cs = started_csound(INTEROP_ORC);

    let chan = cs.init_array_channel("host_to_orc", "k", &[4]).unwrap();
    chan.set_data(&[1.0, 0.0, 0.0, 2.0]).unwrap();

    cs.send_score_event(ScoreEventType::Instrument, &[10.0, 0.0, 10.0]);
    for _ in 0..4 {
        cs.perform_ksmps();
    }
    assert_eq!(cs.get_control_channel("readback0").unwrap(), 1.0);

    // Update from the host mid-performance; the orchestra should observe it.
    chan.set_data(&[99.0, 0.0, 0.0, 98.0]).unwrap();
    for _ in 0..4 {
        cs.perform_ksmps();
    }
    assert_eq!(cs.get_control_channel("readback0").unwrap(), 99.0);
    assert_eq!(cs.get_control_channel("readback3").unwrap(), 98.0);
}
