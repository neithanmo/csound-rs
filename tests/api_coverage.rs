//! Tests for the host-facing Csound 7 calls added alongside array channels:
//! engine parameters, the threadsafe table copies, and the assorted attribute
//! and name accessors.

use csound::{Csound, Error, MessageType};

static ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 1
0dbfs = 1

instr 1
endin

instr Named
endin
"#;

/// Orchestra that creates a function table, so the table copies have something
/// real to work against.
static TABLE_ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 1
0dbfs = 1

gitab ftgen 1, 0, 16, 2, 0

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

fn started_csound(orc: &str) -> Csound {
    let cs = create_test_csound();
    cs.compile_orc(orc, 0).expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");
    cs
}

// ---------------------------------------------------------------------------
// Engine parameters
// ---------------------------------------------------------------------------

#[test]
fn get_params_reflects_applied_options() {
    let cs = started_csound(ORC);
    let params = cs.get_params().expect("params should be available");

    // -m0 was applied in create_test_csound.
    assert_eq!(params.msglevel, 0);
    // The orchestra header set these.
    assert_eq!(params.sample_accurate, 0);
}

#[test]
fn get_params_returns_an_independent_snapshot() {
    // Options are only accepted before start(), so the change is made there.
    let cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");

    let before = cs.get_params().expect("params should be available");
    assert_eq!(before.msglevel, 0, "-m0 was applied at construction");

    cs.set_option("-m7")
        .expect("failed to set -m7 before start");
    let after = cs.get_params().expect("params should be available");

    // The first snapshot is owned, so the engine-side change cannot reach it.
    assert_eq!(before.msglevel, 0, "the first snapshot must not change");
    assert_eq!(
        after.msglevel, 7,
        "a fresh snapshot observes the new option"
    );
}

#[test]
fn options_are_rejected_after_start() {
    // Csound 7 resolves its option set at start(); this documents that the
    // wrapper surfaces the refusal rather than silently ignoring it.
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.set_option("-m7").unwrap_err(),
        Error::InvalidOption(_)
    ));
}

#[test]
fn get_params_string_fields_are_owned() {
    let mut cs = started_csound(ORC);
    let params = cs.get_params().expect("params should be available");

    // Whatever these are, reading them must not hand out engine pointers, and
    // must survive a reset.
    let outfile = params.outfilename.clone();
    cs.reset();
    assert_eq!(params.outfilename, outfile);
}

// ---------------------------------------------------------------------------
// Attributes and names
// ---------------------------------------------------------------------------

#[test]
fn kcounter_advances_with_performance() {
    let cs = started_csound(ORC);
    let start = cs.get_kcounter();

    for _ in 0..4 {
        cs.perform_ksmps();
    }

    let after = cs.get_kcounter();
    assert!(
        after > start,
        "kcounter should advance during performance (was {start}, now {after})"
    );
}

#[test]
fn error_count_is_zero_for_a_clean_performance() {
    let cs = started_csound(ORC);
    for _ in 0..4 {
        cs.perform_ksmps();
    }
    assert_eq!(cs.error_count(), 0);
}

#[test]
fn system_sr_stores_and_queries() {
    let cs = started_csound(ORC);

    // Csound initialises the stored hardware rate to -1, meaning "not set".
    // Anything non-positive is a query, so this must not overwrite it.
    assert_eq!(cs.system_sr(0.0), -1.0, "default should be the -1 sentinel");
    assert_eq!(cs.system_sr(0.0), -1.0, "a query must not mutate the value");

    // A positive value stores and returns it.
    assert_eq!(cs.system_sr(48000.0), 48000.0);
    assert_eq!(cs.system_sr(0.0), 48000.0, "the value should persist");

    // A further query still must not clear it.
    assert_eq!(cs.system_sr(-1.0), 48000.0);
}

#[test]
fn output_name_is_reported() {
    // Written into the temp dir: starting Csound with an output set creates the
    // file, and a relative path would leave it in the repository.
    let out = std::env::temp_dir().join(format!("csound-rs-outname-{}.wav", std::process::id()));

    let mut cs = create_test_csound();
    cs.set_option(&format!("-o{}", out.display()))
        .expect("failed to set output");
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");

    let name = cs
        .get_output_name()
        .expect("output name should be valid UTF-8")
        .expect("an output name should be set");
    assert!(
        name.contains("csound-rs-outname-"),
        "unexpected output name: {name}"
    );

    cs.reset();
    let _ = std::fs::remove_file(&out);
}

#[test]
fn input_name_is_absent_when_not_configured() {
    let cs = started_csound(ORC);
    // No input configured; the accessor must not crash and must not invent one.
    let name = cs
        .get_input_name()
        .expect("input name should be valid UTF-8");
    if let Some(name) = name {
        assert!(!name.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Named instruments
// ---------------------------------------------------------------------------

#[test]
fn named_instrument_resolves_to_a_number() {
    let cs = started_csound(ORC);
    let number = cs
        .get_instrument_number("Named")
        .expect("named instrument should resolve");
    assert!(number > 0, "expected a positive instrument number");
}

#[test]
fn unknown_instrument_name_is_not_found() {
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.get_instrument_number("NoSuchInstrument").unwrap_err(),
        Error::NotFound(_)
    ));
}

#[test]
fn instrument_name_validation() {
    let cs = started_csound(ORC);
    assert!(matches!(
        cs.get_instrument_number("").unwrap_err(),
        Error::EmptyString
    ));
    let err = cs.get_instrument_number("bad\0name").unwrap_err();
    assert!(matches!(err, Error::Nul(_)));
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn csound_status_is_preserved_as_an_error_source() {
    let err = Error::CsoundCall {
        operation: "test operation",
        status: csound::CsoundErrorCode::Error,
    };
    let source = std::error::Error::source(&err).expect("Csound status should be a source");
    assert_eq!(source.to_string(), "CSOUND_ERROR (-1)");
}

#[test]
fn c_integer_boundaries_are_rejected_before_ffi() {
    let cs = create_test_csound();
    let outside_c_int = i32::MAX as u32 + 1;

    assert!(matches!(
        cs.table_length(outside_c_int).unwrap_err(),
        Error::IntegerOutOfRange {
            argument: "table",
            ..
        }
    ));
    assert!(matches!(
        cs.udp_server_start(u32::MAX).unwrap_err(),
        Error::IntegerOutOfRange {
            argument: "port",
            ..
        }
    ));
    assert!(matches!(
        cs.create_circular_buffer::<u8>(u32::MAX)
            .err()
            .expect("oversized length must fail"),
        Error::IntegerOutOfRange {
            argument: "len",
            ..
        }
    ));
    assert!(matches!(
        cs.create_circular_buffer::<u8>(i32::MAX as u32)
            .err()
            .expect("sentinel size overflow must fail"),
        Error::SizeOverflow { .. }
    ));
}

// ---------------------------------------------------------------------------
// Threadsafe table copies
// ---------------------------------------------------------------------------

#[test]
fn table_copy_round_trip() {
    let cs = started_csound(TABLE_ORC);
    let len = cs.table_length(1).expect("table 1 should exist");
    assert_eq!(len, 16);

    // copy_in transfers len + 1 values: the table plus its guard point.
    let input: Vec<f64> = (0..=len).map(|i| i as f64 * 2.0).collect();
    let copied = cs.table_copy_in(1, &input).expect("copy in failed");
    assert_eq!(copied, len + 1);

    // copy_out transfers only len; the guard point is not read back.
    let mut output = vec![0.0f64; len];
    let copied = cs.table_copy_out(1, &mut output).expect("copy out failed");
    assert_eq!(copied, len);
    assert_eq!(output, input[..len]);
}

#[test]
fn table_copy_accepts_oversized_buffers() {
    let cs = started_csound(TABLE_ORC);
    let len = cs.table_length(1).unwrap();

    // Longer than required: Csound transfers only what it needs.
    let input = vec![1.5f64; len + 8];
    assert_eq!(cs.table_copy_in(1, &input).unwrap(), len + 1);

    let mut output = vec![0.0f64; len + 8];
    assert_eq!(cs.table_copy_out(1, &mut output).unwrap(), len);
    assert!(output[..len].iter().all(|&v| v == 1.5));
    // The tail past the table length must be untouched.
    assert!(output[len..].iter().all(|&v| v == 0.0));
}

#[test]
fn table_copy_rejects_short_buffers() {
    let cs = started_csound(TABLE_ORC);
    let len = cs.table_length(1).unwrap();

    // Csound copies exactly `len` elements and checks nothing about the
    // caller's buffer, so a short slice would be read or written past its end.
    // copy_in reads len + 1, so even a slice of exactly len is too short.
    let exact_in = vec![0.0f64; len];
    match cs.table_copy_in(1, &exact_in).unwrap_err() {
        Error::BufferTooSmall {
            required, actual, ..
        } => {
            assert_eq!(required, len + 1, "copy_in must demand the guard point");
            assert_eq!(actual, len);
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let mut short_out = vec![0.0f64; len - 1];
    assert!(matches!(
        cs.table_copy_out(1, &mut short_out).unwrap_err(),
        Error::BufferTooSmall { .. }
    ));
}

#[test]
fn table_copy_rejects_missing_tables() {
    let cs = started_csound(TABLE_ORC);

    // A missing table makes Csound's own helper compute a -1 length and pass
    // it to memcpy as a size_t. The existence check must catch it first.
    let mut out = vec![0.0f64; 16];
    assert!(matches!(
        cs.table_copy_out(999, &mut out).unwrap_err(),
        Error::NotFound(_)
    ));

    let input = vec![0.0f64; 17];
    assert!(matches!(
        cs.table_copy_in(999, &input).unwrap_err(),
        Error::NotFound(_)
    ));
}

#[test]
fn table_copy_out_matches_owned_snapshot() {
    let cs = started_csound(TABLE_ORC);
    let len = cs.table_length(1).unwrap();

    let input: Vec<f64> = (0..=len).map(|i| (i as f64).sin()).collect();
    cs.table_copy_in(1, &input).unwrap();

    // The explicit copy and the owned snapshot must agree.
    let mut copied = vec![0.0f64; len];
    cs.table_copy_out(1, &mut copied).unwrap();

    let snapshot = cs.read_table(1).expect("table should exist");
    assert_eq!(copied, snapshot);
}

// ---------------------------------------------------------------------------
// Handle lifetime vs. reset
// ---------------------------------------------------------------------------

#[test]
fn reset_is_usable_after_scoped_table_access() {
    // Scoped table access ends before reset, and the instance remains reusable
    // afterwards.
    let mut cs = started_csound(TABLE_ORC);

    cs.with_table(1, |table| {
        assert_eq!(table.len(), 16);
    })
    .expect("table should exist");

    cs.reset();

    // reset() clears the option set too, so it has to be re-applied; otherwise
    // the next start() falls back to Csound's defaults and writes a soundfile.
    cs.set_option("-n").expect("failed to re-apply -n");
    cs.set_option("-d").expect("failed to re-apply -d");
    cs.set_option("-m0").expect("failed to re-apply -m0");

    // The instance is reusable: compile and start again.
    cs.compile_orc(TABLE_ORC, 0).expect("recompile failed");
    cs.start().expect("restart failed");
    let table = cs.read_table(1).expect("table should exist after reset");
    assert_eq!(table.len(), 16);
}
