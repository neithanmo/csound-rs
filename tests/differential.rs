//! Differential tests: the bindings vs. the `csound` command-line frontend.
//!
//! The rest of the suite checks that the Rust API behaves as designed. These
//! tests check something different and harder to fake: that driving Csound
//! *through the FFI* produces the same audio as Csound driving itself.
//!
//! That matters because the failure mode this crate is most exposed to is a
//! binding that compiles and runs but is subtly wrong — a changed signature, a
//! wrong element count, a buffer read at the wrong rate. Csound 7 moved
//! `csoundGetChannelPtr` from `MYFLT**` to `void**`, and swapped
//! `csoundScoreEvent`/`csoundPerform` for `csoundEvent`/`csoundPerformKsmps`;
//! mistakes in that class type-check cleanly and then corrupt audio. Comparing
//! rendered samples against the reference frontend catches them.
//!
//! Output is written headerless (`-h`) as 64-bit doubles (`--format=double`),
//! so the bytes on disk are the sample stream with no format conversion, and
//! comparisons can be exact rather than approximate.
//!
//! # Requirements
//!
//! Needs a `csound` binary built from the *same* Csound as the linked library.
//! Set `CSOUND_BIN` to point at it. If no suitable binary is found the tests
//! skip; if one is found but its version disagrees with the linked library,
//! they fail, because a mismatch would invalidate the comparison.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use csound::{Csound, MessageType};

/// A CSD rendering a fixed 0.25s stereo signal.
///
/// Two different frequencies per channel so a channel swap or interleaving
/// mistake shows up, and an envelope so the comparison covers more than a
/// steady state.
fn csd_source_with_output_options(output_options: &str) -> String {
    format!(
        r#"<CsoundSynthesizer>
<CsOptions>
{output_options} -m0 -d
</CsOptions>
<CsInstruments>
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

instr 1
  kenv linen 0.5, 0.05, p3, 0.05
  aL oscili kenv, 440
  aR oscili kenv, 553.75
  outs aL, aR
endin
</CsInstruments>
<CsScore>
i 1 0 0.25
e
</CsScore>
</CsoundSynthesizer>
"#
    )
}

/// Builds the signal CSD with headerless double-precision file output.
fn csd_source(output: &Path) -> String {
    csd_source_with_output_options(&format!("-o {} -h --format=double", output.display()))
}

/// Builds the signal CSD without opening an output file.
///
/// Csound's `-n` is portable; using `/dev/null` here makes `csoundStart` fail
/// on Windows before the test can read the in-memory `spout` buffer.
fn csd_source_for_spout() -> String {
    csd_source_with_output_options("-n")
}

/// A CSD whose output depends on score events at several onsets, so a mistake
/// in event dispatch or score timing shows up as a divergence.
fn csd_multi_event(output: &Path) -> String {
    format!(
        r#"<CsoundSynthesizer>
<CsOptions>
-o {out} -h --format=double -m0 -d
</CsOptions>
<CsInstruments>
sr = 44100
ksmps = 64
nchnls = 1
0dbfs = 1

instr 1
  kenv linen 0.3, 0.01, p3, 0.01
  a1 oscili kenv, p4
  out a1
endin
</CsInstruments>
<CsScore>
i 1 0.00 0.10 220
i 1 0.05 0.10 330
i 1 0.12 0.08 440
i 1 0.20 0.05 880
e
</CsScore>
</CsoundSynthesizer>
"#,
        out = output.display()
    )
}

/// Locates a `csound` binary suitable for use as the reference renderer.
fn csound_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CSOUND_BIN") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }

    let mut candidates = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(&home).join("csound7-install/bin/csound"));
    }
    candidates.push(PathBuf::from("/usr/local/bin/csound"));
    candidates.push(PathBuf::from("/usr/bin/csound"));

    candidates.into_iter().find(|path| path.is_file())
}

/// Returns the reference binary, or `None` if the tests should be skipped.
///
/// Panics if a binary is found whose version disagrees with the linked
/// library: silently comparing against a different Csound would make these
/// tests meaningless.
fn reference_binary() -> Option<PathBuf> {
    let binary = csound_binary()?;

    let output = Command::new(&binary)
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", binary.display()));

    // --version writes to stderr on some builds.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let cli_version = parse_version(&text).unwrap_or_else(|| {
        panic!(
            "could not parse a version out of `{} --version`:\n{text}",
            binary.display()
        )
    });

    let linked = Csound::new().expect("failed to create Csound instance");
    let raw = linked.version();
    let expected = (raw / 1000, (raw % 1000) / 10);

    assert_eq!(
        cli_version,
        expected,
        "reference binary {} is Csound {}.{}, but the linked library is {}.{}. \
         Point CSOUND_BIN at a matching build; comparing across versions would \
         not prove anything.",
        binary.display(),
        cli_version.0,
        cli_version.1,
        expected.0,
        expected.1
    );

    Some(binary)
}

/// Extracts `(major, minor)` from a `--version` banner.
fn parse_version(text: &str) -> Option<(u32, u32)> {
    let idx = text.find("version")?;
    let rest = &text[idx + "version".len()..];
    let token = rest.split_whitespace().next()?;
    let mut parts = token.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Creates a unique scratch directory for one test.
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("csound-rs-diff-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create scratch dir");
    dir
}

/// Renders `csd` with the reference command-line frontend.
fn render_with_cli(binary: &Path, csd: &Path) {
    let output = Command::new(binary)
        .arg(csd)
        .output()
        .expect("failed to run the csound frontend");

    assert!(
        output.status.success(),
        "csound frontend failed on {}:\n{}",
        csd.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Renders `csd` through the bindings, driving the performance loop directly.
fn render_with_bindings(csd: &Path) {
    let mut cs = Csound::new().expect("failed to create Csound instance");
    cs.message_string_callback(|_: MessageType, _: &str| {});

    cs.compile_csd(csd.to_str().unwrap(), 0, 0)
        .expect("compile_csd failed");
    cs.start().expect("start failed");

    // Bounded so a semantic regression cannot hang the suite: 0.25s at 44.1kHz
    // is well under this.
    let mut guard = 0;
    while !cs.perform_ksmps() {
        guard += 1;
        assert!(guard < 100_000, "performance did not terminate");
    }
    cs.reset();
}

/// Reads a headerless stream of native-endian `f64` samples.
fn read_doubles(path: &Path) -> Vec<f64> {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    assert!(
        !bytes.is_empty(),
        "{} is empty; nothing was rendered",
        path.display()
    );

    let (chunks, remainder) = bytes.as_chunks::<{ std::mem::size_of::<f64>() }>();
    assert!(
        remainder.is_empty(),
        "{} is not a whole number of f64 samples",
        path.display()
    );

    chunks
        .iter()
        .map(|chunk| f64::from_ne_bytes(*chunk))
        .collect()
}

/// Asserts two sample streams are identical, reporting the first divergence.
fn assert_samples_identical(reference: &[f64], actual: &[f64], context: &str) {
    assert_eq!(
        reference.len(),
        actual.len(),
        "{context}: sample count differs (frontend {}, bindings {})",
        reference.len(),
        actual.len()
    );

    for (i, (r, a)) in reference.iter().zip(actual.iter()).enumerate() {
        assert_eq!(
            r.to_bits(),
            a.to_bits(),
            "{context}: first divergence at sample {i}: frontend {r}, bindings {a}"
        );
    }
}

/// Runs one differential comparison end to end.
fn compare_rendering(tag: &str, build_csd: fn(&Path) -> String) {
    let Some(binary) = reference_binary() else {
        eprintln!("skipping differential test `{tag}`: no matching csound binary found");
        eprintln!("set CSOUND_BIN to a csound built from the same source as the linked library");
        return;
    };

    let dir = scratch_dir(tag);

    let cli_out = dir.join("cli.raw");
    let lib_out = dir.join("lib.raw");
    let cli_csd = dir.join("cli.csd");
    let lib_csd = dir.join("lib.csd");

    // Identical scores; only the output path differs.
    fs::write(&cli_csd, build_csd(&cli_out)).unwrap();
    fs::write(&lib_csd, build_csd(&lib_out)).unwrap();

    render_with_cli(&binary, &cli_csd);
    render_with_bindings(&lib_csd);

    let reference = read_doubles(&cli_out);
    let actual = read_doubles(&lib_out);
    assert_samples_identical(&reference, &actual, tag);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn bindings_match_frontend_for_a_simple_render() {
    compare_rendering("simple", csd_source);
}

#[test]
fn bindings_match_frontend_for_multiple_score_events() {
    compare_rendering("events", csd_multi_event);
}

/// Compares samples read live from `spout` against the frontend's file output.
///
/// This covers the direct-buffer path rather than Csound's own file writer:
/// `csoundGetSpout`, and the `ksmps * nchnls` framing the bindings derive for
/// it. A wrong frame size here would misalign every subsequent block.
#[test]
fn spout_stream_matches_frontend_output() {
    let tag = "spout";
    let Some(binary) = reference_binary() else {
        eprintln!("skipping differential test `{tag}`: no matching csound binary found");
        return;
    };

    let dir = scratch_dir(tag);
    let cli_out = dir.join("cli.raw");
    let cli_csd = dir.join("cli.csd");
    fs::write(&cli_csd, csd_source(&cli_out)).unwrap();
    render_with_cli(&binary, &cli_csd);
    let reference = read_doubles(&cli_out);

    // Render again through the bindings, this time with no output file, and
    // accumulate what Csound places in spout each control period.
    let lib_csd = dir.join("lib.csd");
    fs::write(&lib_csd, csd_source_for_spout()).unwrap();

    let mut cs = Csound::new().expect("failed to create Csound instance");
    cs.message_string_callback(|_: MessageType, _: &str| {});
    cs.compile_csd(lib_csd.to_str().unwrap(), 0, 0)
        .expect("compile_csd failed");
    cs.start().expect("start failed");

    let frame = (cs.get_ksmps() * cs.get_channels(0)) as usize;
    assert!(frame > 0, "spout frame size must be non-zero");

    let mut captured: Vec<f64> = Vec::with_capacity(reference.len());
    let mut guard = 0;
    while !cs.perform_ksmps() {
        let spout = cs.get_spout().expect("spout buffer not available");
        assert_eq!(
            spout.get_size(),
            frame,
            "spout length changed mid-performance"
        );
        captured.extend_from_slice(spout.as_slice());

        guard += 1;
        assert!(guard < 100_000, "performance did not terminate");
    }

    // The frontend writes whole control periods, so the streams should line up
    // exactly; compare over the common length and require the capture to cover
    // the full reference.
    assert!(
        captured.len() >= reference.len(),
        "captured {} samples from spout but the frontend wrote {}",
        captured.len(),
        reference.len()
    );
    assert_samples_identical(&reference, &captured[..reference.len()], tag);

    cs.reset();
    let _ = fs::remove_dir_all(&dir);
}
