//! Read-only access to the engine's resolved option set.
//!
//! Csound 6 exposed `csoundSetParameters()` alongside a getter. Csound 7
//! removed the setter: options are now set through command-line arguments,
//! `<CsOptions>`, or [`Csound::set_option`], and `csoundGetParams()` reports
//! the resolved result. This module mirrors that read-only view.
//!
//! The engine returns a borrowed `OPARMS` (a `CSOUND_PARAMS` alias) that stays
//! owned by Csound and can change as options are applied, so
//! [`Csound::get_params`] copies it into an owned snapshot and converts the
//! file-name fields into owned `String`s rather than handing out raw pointers.

use std::ffi::CStr;

use libc::c_char;

use crate::Csound;

/// A snapshot of the engine's resolved options.
///
/// Field names mirror the C `CSOUND_PARAMS` members; the original spelling is
/// noted on each so it can be matched against Csound's own documentation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CsoundParams {
    /// `odebug`
    pub odebug: i32,
    /// `sfread`
    pub sfread: i32,
    /// `sfwrite`
    pub sfwrite: i32,
    /// `filetyp`
    pub filetyp: i32,
    /// `inbufsamps`
    pub inbufsamps: i32,
    /// `outbufsamps`
    pub outbufsamps: i32,
    /// `informat`
    pub informat: i32,
    /// `outformat`
    pub outformat: i32,
    /// `sndfileSampleSize`
    pub sndfile_sample_size: i32,
    /// `displays`
    pub displays: i32,
    /// `graphsoff`
    pub graphsoff: i32,
    /// `postscript`
    pub postscript: i32,
    /// `msglevel`
    pub msglevel: i32,
    /// `Beatmode`
    pub beatmode: i32,
    /// `oMaxLag`
    pub o_max_lag: i32,
    /// `Linein`
    pub linein: i32,
    /// `RTevents`
    pub r_tevents: i32,
    /// `Midiin`
    pub midiin: i32,
    /// `FMidiin`
    pub f_midiin: i32,
    /// `RMidiin`
    pub r_midiin: i32,
    /// `ringbell`
    pub ringbell: i32,
    /// `termifend`
    pub termifend: i32,
    /// `rewrt_hdr`
    pub rewrt_hdr: i32,
    /// `heartbeat`
    pub heartbeat: i32,
    /// `gen01defer`
    pub gen01defer: i32,
    /// `cmdTempo`
    pub cmd_tempo: f64,
    /// `sr_override`
    pub sr_override: f64,
    /// `kr_override`
    pub kr_override: f64,
    /// `nchnls_override`
    pub nchnls_override: i32,
    /// `nchnls_i_override`
    pub nchnls_i_override: i32,
    /// `midiKey`
    pub midi_key: i32,
    /// `midiKeyCps`
    pub midi_key_cps: i32,
    /// `midiKeyOct`
    pub midi_key_oct: i32,
    /// `midiKeyPch`
    pub midi_key_pch: i32,
    /// `midiVelocity`
    pub midi_velocity: i32,
    /// `midiVelocityAmp`
    pub midi_velocity_amp: i32,
    /// `noDefaultPaths`
    pub no_default_paths: i32,
    /// `numThreads`
    pub num_threads: i32,
    /// `syntaxCheckOnly`
    pub syntax_check_only: i32,
    /// `runUnitTests`
    pub run_unit_tests: i32,
    /// `useCsdLineCounts`
    pub use_csd_line_counts: i32,
    /// `sampleAccurate`
    pub sample_accurate: i32,
    /// `realtime`
    pub realtime: i32,
    /// `e0dbfs_override`
    pub e0dbfs_override: f64,
    /// `daemon`
    pub daemon: i32,
    /// `quality`
    pub quality: f64,
    /// `ksmps_override`
    pub ksmps_override: i32,
    /// `fft_lib`
    pub fft_lib: i32,
    /// `echo`
    pub echo: i32,
    /// `limiter`
    pub limiter: f64,
    /// `sr_default`
    pub sr_default: f64,
    /// `kr_default`
    pub kr_default: f64,
    /// `mp3_mode`
    pub mp3_mode: i32,
    /// `redef`
    pub redef: i32,
    /// `error_deprecated`
    pub error_deprecated: i32,
    /// `recursion_depth`
    pub recursion_depth: i32,
    /// `infilename`
    pub infilename: Option<String>,
    /// `outfilename`
    pub outfilename: Option<String>,
    /// `Linename`
    pub linename: Option<String>,
    /// `Midiname`
    pub midiname: Option<String>,
    /// `FMidiname`
    pub f_midiname: Option<String>,
    /// `Midioutname`
    pub midioutname: Option<String>,
    /// `FMidioutname`
    pub f_midioutname: Option<String>,
}

/// Copies a possibly-null C string into an owned `String`.
///
/// Returns `None` for a null pointer or non-UTF-8 contents; file names come
/// from the host environment and are not guaranteed to be UTF-8.
fn cstr_opt(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

impl Csound {
    /// Returns a snapshot of the engine's resolved options.
    ///
    /// Csound 7 has no setter counterpart: use [`Csound::set_option`],
    /// `<CsOptions>`, or command-line arguments to change these.
    ///
    /// # Returns
    /// `None` if the engine reported no parameter block.
    ///
    /// # Example
    /// ```no_run
    /// # use csound::Csound;
    /// let cs = Csound::new().unwrap();
    /// cs.set_option("-m0").unwrap();
    /// let params = cs.get_params().unwrap();
    /// assert_eq!(params.msglevel, 0);
    /// ```
    pub fn get_params(&self) -> Option<CsoundParams> {
        // SAFETY: the pointer is owned by Csound and only read here, before
        // any further API call could invalidate it.
        let raw = unsafe { csound_sys::csoundGetParams(self.csound_ptr()) };
        if raw.is_null() {
            return None;
        }
        let raw = unsafe { &*raw };

        Some(CsoundParams {
            odebug: raw.odebug,
            sfread: raw.sfread,
            sfwrite: raw.sfwrite,
            filetyp: raw.filetyp,
            inbufsamps: raw.inbufsamps,
            outbufsamps: raw.outbufsamps,
            informat: raw.informat,
            outformat: raw.outformat,
            sndfile_sample_size: raw.sndfileSampleSize,
            displays: raw.displays,
            graphsoff: raw.graphsoff,
            postscript: raw.postscript,
            msglevel: raw.msglevel,
            beatmode: raw.Beatmode,
            o_max_lag: raw.oMaxLag,
            linein: raw.Linein,
            r_tevents: raw.RTevents,
            midiin: raw.Midiin,
            f_midiin: raw.FMidiin,
            r_midiin: raw.RMidiin,
            ringbell: raw.ringbell,
            termifend: raw.termifend,
            rewrt_hdr: raw.rewrt_hdr,
            heartbeat: raw.heartbeat,
            gen01defer: raw.gen01defer,
            cmd_tempo: raw.cmdTempo,
            sr_override: raw.sr_override,
            kr_override: raw.kr_override,
            nchnls_override: raw.nchnls_override,
            nchnls_i_override: raw.nchnls_i_override,
            midi_key: raw.midiKey,
            midi_key_cps: raw.midiKeyCps,
            midi_key_oct: raw.midiKeyOct,
            midi_key_pch: raw.midiKeyPch,
            midi_velocity: raw.midiVelocity,
            midi_velocity_amp: raw.midiVelocityAmp,
            no_default_paths: raw.noDefaultPaths,
            num_threads: raw.numThreads,
            syntax_check_only: raw.syntaxCheckOnly,
            run_unit_tests: raw.runUnitTests,
            use_csd_line_counts: raw.useCsdLineCounts,
            sample_accurate: raw.sampleAccurate,
            realtime: raw.realtime,
            e0dbfs_override: raw.e0dbfs_override,
            daemon: raw.daemon,
            quality: raw.quality,
            ksmps_override: raw.ksmps_override,
            fft_lib: raw.fft_lib,
            echo: raw.echo,
            limiter: raw.limiter,
            sr_default: raw.sr_default,
            kr_default: raw.kr_default,
            mp3_mode: raw.mp3_mode,
            redef: raw.redef,
            error_deprecated: raw.error_deprecated,
            recursion_depth: raw.recursion_depth,
            infilename: cstr_opt(raw.infilename),
            outfilename: cstr_opt(raw.outfilename),
            linename: cstr_opt(raw.Linename),
            midiname: cstr_opt(raw.Midiname),
            f_midiname: cstr_opt(raw.FMidiname),
            midioutname: cstr_opt(raw.Midioutname),
            f_midioutname: cstr_opt(raw.FMidioutname),
        })
    }
}
