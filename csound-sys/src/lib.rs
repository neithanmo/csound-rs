#![allow(bad_style)]
#![allow(dead_code)]
#![allow(improper_ctypes)]

//extern crate va_list;
extern crate libc;
use std::ptr;

use libc::FILE;
use libc::{c_char, c_int, c_long, c_uchar, c_uint, c_float, c_double, c_void};
//use va_list::VaList;

pub type CSOUND_STATUS = c_int;
pub const CSOUND_SIGNAL: CSOUND_STATUS = -5;
pub const CSOUND_MEMORY: CSOUND_STATUS = -4;
pub const CSOUND_PERFORMANCE: CSOUND_STATUS = -3;
pub const CSOUND_INITIALIZATION: CSOUND_STATUS = -2;
pub const CSOUND_ERROR: CSOUND_STATUS = -1;
pub const CSOUND_SUCCESS: CSOUND_STATUS = 0;

pub type controlChannelType = c_uint;
pub const CSOUND_CONTROL_CHANNEL: controlChannelType = 1;
pub const CSOUND_AUDIO_CHANNEL: controlChannelType = 2;
pub const CSOUND_STRING_CHANNEL: controlChannelType = 3;
pub const CSOUND_PVS_CHANNEL: controlChannelType = 4;
pub const CSOUND_VAR_CHANNEL: controlChannelType = 5;
pub const CSOUND_CHANNEL_TYPE_MASK: controlChannelType = 15;
pub const CSOUND_INPUT_CHANNEL: controlChannelType = 16;
pub const CSOUND_OUTPUT_CHANNEL: controlChannelType = 32;

pub type controlChannelBehavior = c_uint;
pub const CSOUND_CONTROL_CHANNEL_NO_HINTS: controlChannelBehavior = 0;
pub const CSOUND_CONTROL_CHANNEL_INT: controlChannelBehavior = 1;
pub const CSOUND_CONTROL_CHANNEL_LIN: controlChannelBehavior = 2;
pub const CSOUND_CONTROL_CHANNEL_EXP: controlChannelBehavior = 3;

pub type csound_file_open_callback = extern "C" fn(*mut CSOUND, *const c_char, c_int, c_int, c_int);

pub type csound_open_callback = Option<extern "C" fn(*mut CSOUND, *const csRtAudioParams) -> c_int>;

pub type csound_rt_play_callback = Option<extern "C" fn(*mut CSOUND, *const c_double, c_int)>;
pub type csound_rt_rec_callback = Option<extern "C" fn(*mut CSOUND, *mut c_double, c_int) -> c_int>;
pub type csound_rt_close_callback = Option<extern "C" fn(*mut CSOUND)>;
pub type cscore_callback_type = Option<extern "C" fn(*mut CSOUND)>;

pub type csound_dev_list_callback =
    Option<extern "C" fn(*mut CSOUND, *mut CS_AUDIODEVICE, c_int) -> c_int>;

pub type csound_midi_dev_list_callback =
    Option<extern "C" fn(*mut CSOUND, *mut CS_MIDIDEVICE, c_int) -> c_int>;

pub type csound_ext_midi_open_callback =
    Option<extern "C" fn(*mut CSOUND, *mut *mut c_void, *const c_char) -> c_int>;
pub type csound_ext_midi_close_callback =
    Option<extern "C" fn(arg1: *mut CSOUND, userData: *mut c_void) -> c_int>;

pub type csound_ext_midi_read_data_callback =
    Option<extern "C" fn(*mut CSOUND, *mut c_void, *mut c_uchar, c_int) -> c_int>;

pub type csound_ext_midi_write_data_callback =
    Option<extern "C" fn(*mut CSOUND, *mut c_void, *const c_uchar, c_int) -> c_int>;

pub type csound_ext_midi_error_callback = Option<extern "C" fn(c_int) -> *const c_char>;

pub type csound_message_callback = extern "C" fn(*mut CSOUND, c_int, *const c_char);

pub type csound_channel_callback =
    extern "C" fn(*mut CSOUND, *const c_char, *mut c_void, *const c_void);
pub const CSOUND_EXITJMP_SUCCESS: u32 = 256;
pub const CSOUNDINIT_NO_SIGNAL_HANDLER: u32 = 1;
pub const CSOUNDINIT_NO_ATEXIT: u32 = 2;
pub const CSOUND_CALLBACK_KBD_EVENT: u32 = 1;
pub const CSOUND_CALLBACK_KBD_TEXT: u32 = 2;
pub const CSOUNDCFG_INTEGER: u32 = 1;
pub const CSOUNDCFG_BOOLEAN: u32 = 2;
pub const CSOUNDCFG_FLOAT: u32 = 3;
pub const CSOUNDCFG_DOUBLE: u32 = 4;
pub const CSOUNDCFG_MYFLT: u32 = 5;
pub const CSOUNDCFG_STRING: u32 = 6;
pub const CSOUNDCFG_POWOFTWO: u32 = 1;
pub const CSOUNDCFG_SUCCESS: u32 = 0;
pub const CSOUNDCFG_INVALID_NAME: i32 = -1;
pub const CSOUNDCFG_INVALID_TYPE: i32 = -2;
pub const CSOUNDCFG_INVALID_FLAG: i32 = -3;
pub const CSOUNDCFG_NULL_POINTER: i32 = -4;
pub const CSOUNDCFG_TOO_HIGH: i32 = -5;
pub const CSOUNDCFG_TOO_LOW: i32 = -6;
pub const CSOUNDCFG_NOT_POWOFTWO: i32 = -7;
pub const CSOUNDCFG_INVALID_BOOLEAN: i32 = -8;
pub const CSOUNDCFG_MEMORY: i32 = -9;
pub const CSOUNDCFG_STRING_LENGTH: i32 = -10;
pub const CSOUNDCFG_LASTERROR: i32 = -10;
pub const CSOUNDMSG_DEFAULT: u32 = 0;
pub const CSOUNDMSG_ERROR: u32 = 4096;
pub const CSOUNDMSG_ORCH: u32 = 8192;
pub const CSOUNDMSG_REALTIME: u32 = 12288;
pub const CSOUNDMSG_WARNING: u32 = 16384;
pub const CSOUNDMSG_STDOUT: u32 = 20480;
pub const CSOUNDMSG_FG_BLACK: u32 = 256;
pub const CSOUNDMSG_FG_RED: u32 = 257;
pub const CSOUNDMSG_FG_GREEN: u32 = 258;
pub const CSOUNDMSG_FG_YELLOW: u32 = 259;
pub const CSOUNDMSG_FG_BLUE: u32 = 260;
pub const CSOUNDMSG_FG_MAGENTA: u32 = 261;
pub const CSOUNDMSG_FG_CYAN: u32 = 262;
pub const CSOUNDMSG_FG_WHITE: u32 = 263;
pub const CSOUNDMSG_FG_BOLD: u32 = 8;
pub const CSOUNDMSG_FG_UNDERLINE: u32 = 128;
pub const CSOUNDMSG_BG_BLACK: u32 = 512;
pub const CSOUNDMSG_BG_RED: u32 = 528;
pub const CSOUNDMSG_BG_GREEN: u32 = 544;
pub const CSOUNDMSG_BG_ORANGE: u32 = 560;
pub const CSOUNDMSG_BG_BLUE: u32 = 576;
pub const CSOUNDMSG_BG_MAGENTA: u32 = 592;
pub const CSOUNDMSG_BG_CYAN: u32 = 608;
pub const CSOUNDMSG_BG_GREY: u32 = 624;
pub const CSOUNDMSG_TYPE_MASK: u32 = 28672;
pub const CSOUNDMSG_FG_COLOR_MASK: u32 = 263;
pub const CSOUNDMSG_FG_ATTR_MASK: u32 = 136;
pub const CSOUNDMSG_BG_COLOR_MASK: u32 = 624;

pub type csLenguage_t = u32;
pub const CSLANGUAGE_DEFAULT: csLenguage_t = 0;
pub const CSLANGUAGE_AFRIKAANS: csLenguage_t = 1;
pub const CSLANGUAGE_ALBANIAN: csLenguage_t = 2;
pub const CSLANGUAGE_ARABIC: csLenguage_t = 3;
pub const CSLANGUAGE_ARMENIAN: csLenguage_t = 4;
pub const CSLANGUAGE_ASSAMESE: csLenguage_t = 5;
pub const CSLANGUAGE_AZERI: csLenguage_t = 6;
pub const CSLANGUAGE_BASQUE: csLenguage_t = 7;
pub const CSLANGUAGE_BELARUSIAN: csLenguage_t = 8;
pub const CSLANGUAGE_BENGALI: csLenguage_t = 9;
pub const CSLANGUAGE_BULGARIAN: csLenguage_t = 10;
pub const CSLANGUAGE_CATALAN: csLenguage_t = 11;
pub const CSLANGUAGE_CHINESE: csLenguage_t = 12;
pub const CSLANGUAGE_CROATIAN: csLenguage_t = 13;
pub const CSLANGUAGE_CZECH: csLenguage_t = 14;
pub const CSLANGUAGE_DANISH: csLenguage_t = 15;
pub const CSLANGUAGE_DUTCH: csLenguage_t = 16;
pub const CSLANGUAGE_ENGLISH_UK: csLenguage_t = 17;
pub const CSLANGUAGE_ENGLISH_US: csLenguage_t = 18;
pub const CSLANGUAGE_ESTONIAN: csLenguage_t = 19;
pub const CSLANGUAGE_FAEROESE: csLenguage_t = 20;
pub const CSLANGUAGE_FARSI: csLenguage_t = 21;
pub const CSLANGUAGE_FINNISH: csLenguage_t = 22;
pub const CSLANGUAGE_FRENCH: csLenguage_t = 23;
pub const CSLANGUAGE_GEORGIAN: csLenguage_t = 24;
pub const CSLANGUAGE_GERMAN: csLenguage_t = 25;
pub const CSLANGUAGE_GREEK: csLenguage_t = 26;
pub const CSLANGUAGE_GUJARATI: csLenguage_t = 27;
pub const CSLANGUAGE_HEBREW: csLenguage_t = 28;
pub const CSLANGUAGE_HINDI: csLenguage_t = 29;
pub const CSLANGUAGE_HUNGARIAN: csLenguage_t = 30;
pub const CSLANGUAGE_ICELANDIC: csLenguage_t = 31;
pub const CSLANGUAGE_INDONESIAN: csLenguage_t = 32;
pub const CSLANGUAGE_ITALIAN: csLenguage_t = 33;
pub const CSLANGUAGE_JAPANESE: csLenguage_t = 34;
pub const CSLANGUAGE_KANNADA: csLenguage_t = 35;
pub const CSLANGUAGE_KASHMIRI: csLenguage_t = 36;
pub const CSLANGUAGE_KAZAK: csLenguage_t = 37;
pub const CSLANGUAGE_KONKANI: csLenguage_t = 38;
pub const CSLANGUAGE_KOREAN: csLenguage_t = 39;
pub const CSLANGUAGE_LATVIAN: csLenguage_t = 40;
pub const CSLANGUAGE_LITHUANIAN: csLenguage_t = 41;
pub const CSLANGUAGE_MACEDONIAN: csLenguage_t = 42;
pub const CSLANGUAGE_MALAY: csLenguage_t = 43;
pub const CSLANGUAGE_MALAYALAM: csLenguage_t = 44;
pub const CSLANGUAGE_MANIPURI: csLenguage_t = 45;
pub const CSLANGUAGE_MARATHI: csLenguage_t = 46;
pub const CSLANGUAGE_NEPALI: csLenguage_t = 47;
pub const CSLANGUAGE_NORWEGIAN: csLenguage_t = 48;
pub const CSLANGUAGE_ORIYA: csLenguage_t = 49;
pub const CSLANGUAGE_POLISH: csLenguage_t = 50;
pub const CSLANGUAGE_PORTUGUESE: csLenguage_t = 51;
pub const CSLANGUAGE_PUNJABI: csLenguage_t = 52;
pub const CSLANGUAGE_ROMANIAN: csLenguage_t = 53;
pub const CSLANGUAGE_RUSSIAN: csLenguage_t = 54;
pub const CSLANGUAGE_SANSKRIT: csLenguage_t = 55;
pub const CSLANGUAGE_SERBIAN: csLenguage_t = 56;
pub const CSLANGUAGE_SINDHI: csLenguage_t = 57;
pub const CSLANGUAGE_SLOVAK: csLenguage_t = 58;
pub const CSLANGUAGE_SLOVENIAN: csLenguage_t = 59;
pub const CSLANGUAGE_SPANISH: csLenguage_t = 60;
pub const CSLANGUAGE_SWAHILI: csLenguage_t = 61;
pub const CSLANGUAGE_SWEDISH: csLenguage_t = 62;
pub const CSLANGUAGE_TAMIL: csLenguage_t = 63;
pub const CSLANGUAGE_TATAR: csLenguage_t = 64;
pub const CSLANGUAGE_TELUGU: csLenguage_t = 65;
pub const CSLANGUAGE_THAI: csLenguage_t = 66;
pub const CSLANGUAGE_TURKISH: csLenguage_t = 67;
pub const CSLANGUAGE_UKRAINIAN: csLenguage_t = 68;
pub const CSLANGUAGE_URDU: csLenguage_t = 69;
pub const CSLANGUAGE_UZBEK: csLenguage_t = 70;
pub const CSLANGUAGE_VIETNAMESE: csLenguage_t = 71;
pub const CSLANGUAGE_COLUMBIAN: csLenguage_t = 72;

/**
 * The following constants are used with csound->FileOpen2() and
 * csound->ldmemfile2() to specify the format of a file that is being
 * opened.  This information is passed by Csound to a host's FileOpen
 * callback and does not influence the opening operation in any other
 * way. Conversion from Csound's TYP_XXX macros for audio formats to
 * CSOUND_FILETYPES values can be done with csound->type2csfiletype().
 */
pub type CSOUND_FILETYPES_t = u32;

pub const CSFTYPE_UNIFIED_CSD: CSOUND_FILETYPES_t = 1; /* Unified Csound document */
pub const CSFTYPE_ORCHESTRA: CSOUND_FILETYPES_t = 2; /* the primary orc file (may be temporary) */
pub const CSFTYPE_SCORE: CSOUND_FILETYPES_t = 3; /* the primary sco file (may be temporary)*/
/*or any additional score opened by Cscore */
pub const CSFTYPE_ORC_INCLUDE: CSOUND_FILETYPES_t = 4; /* a file #included by the orchestra */
pub const CSFTYPE_SCO_INCLUDE: CSOUND_FILETYPES_t = 5; /* a file #included by the score */
pub const CSFTYPE_SCORE_OUT: CSOUND_FILETYPES_t = 6; /* used for score.srt, score.xtr, cscore.out */
pub const CSFTYPE_SCOT: CSOUND_FILETYPES_t = 7; /* Scot score input format */
pub const CSFTYPE_OPTIONS: CSOUND_FILETYPES_t = 8; /* for .csoundrc and -@ flag */
pub const CSFTYPE_EXTRACT_PARMS: CSOUND_FILETYPES_t = 9; /* extraction file specified by -x */

/* audio file types that Csound can write (10-19) or read */
pub const CSFTYPE_RAW_AUDIO: CSOUND_FILETYPES_t = 9;
pub const CSFTYPE_IRCAM: CSOUND_FILETYPES_t = 10;
pub const CSFTYPE_AIFF: CSOUND_FILETYPES_t = 11;
pub const CSFTYPE_AIFC: CSOUND_FILETYPES_t = 12;
pub const CSFTYPE_WAVE: CSOUND_FILETYPES_t = 13;
pub const CSFTYPE_AU: CSOUND_FILETYPES_t = 14;
pub const CSFTYPE_SD2: CSOUND_FILETYPES_t = 15;
pub const CSFTYPE_W64: CSOUND_FILETYPES_t = 16;
pub const CSFTYPE_WAVEX: CSOUND_FILETYPES_t = 17;
pub const CSFTYPE_FLAC: CSOUND_FILETYPES_t = 18;
pub const CSFTYPE_CAF: CSOUND_FILETYPES_t = 19;
pub const CSFTYPE_WVE: CSOUND_FILETYPES_t = 20;
pub const CSFTYPE_OGG: CSOUND_FILETYPES_t = 21;
pub const CSFTYPE_MPC2K: CSOUND_FILETYPES_t = 22;
pub const CSFTYPE_RF64: CSOUND_FILETYPES_t = 23;
pub const CSFTYPE_AVR: CSOUND_FILETYPES_t = 24;
pub const CSFTYPE_HTK: CSOUND_FILETYPES_t = 25;
pub const CSFTYPE_MAT4: CSOUND_FILETYPES_t = 26;
pub const CSFTYPE_MAT5: CSOUND_FILETYPES_t = 27;
pub const CSFTYPE_NIST: CSOUND_FILETYPES_t = 28;
pub const CSFTYPE_PAF: CSOUND_FILETYPES_t = 29;
pub const CSFTYPE_PVF: CSOUND_FILETYPES_t = 30;
pub const CSFTYPE_SDS: CSOUND_FILETYPES_t = 31;
pub const CSFTYPE_SVX: CSOUND_FILETYPES_t = 32;
pub const CSFTYPE_VOC: CSOUND_FILETYPES_t = 33;
pub const CSFTYPE_XI: CSOUND_FILETYPES_t = 34;
pub const CSFTYPE_UNKNOWN_AUDIO: CSOUND_FILETYPES_t = 35; /* used when opening audio file for reading
                                                          or temp file written with <CsSampleB> */

/* miscellaneous music formats */
pub const CSFTYPE_SOUNDFONT: CSOUND_FILETYPES_t = 36;
pub const CSFTYPE_STD_MIDI: CSOUND_FILETYPES_t = 37; /* Standard MIDI file */
pub const CSFTYPE_MIDI_SYSEX: CSOUND_FILETYPES_t = 38; /* Raw MIDI codes, eg. SysEx dump */

/* analysis formats */
pub const CSFTYPE_HETRO: CSOUND_FILETYPES_t = 39;
pub const CSFTYPE_HETROT: CSOUND_FILETYPES_t = 40;
pub const CSFTYPE_PVC: CSOUND_FILETYPES_t = 41; /* original PVOC format */
pub const CSFTYPE_PVCEX: CSOUND_FILETYPES_t = 42; /* PVOC-EX format */
pub const CSFTYPE_CVANAL: CSOUND_FILETYPES_t = 43;
pub const CSFTYPE_LPC: CSOUND_FILETYPES_t = 44;
pub const CSFTYPE_ATS: CSOUND_FILETYPES_t = 45;
pub const CSFTYPE_LORIS: CSOUND_FILETYPES_t = 46;
pub const CSFTYPE_SDIF: CSOUND_FILETYPES_t = 47;
pub const CSFTYPE_HRTF: CSOUND_FILETYPES_t = 48;

/* Types for plugins and the files they read/write */
pub const CSFTYPE_UNUSED: CSOUND_FILETYPES_t = 49;
pub const CSFTYPE_LADSPA_PLUGIN: CSOUND_FILETYPES_t = 50;
pub const CSFTYPE_SNAPSHOT: CSOUND_FILETYPES_t = 51;

/* Special formats for Csound ftables or scanned synthesis
matrices with header info */
pub const CSFTYPE_FTABLES_TEXT: CSOUND_FILETYPES_t = 52; /* for ftsave and ftload  */
pub const CSFTYPE_FTABLES_BINARY: CSOUND_FILETYPES_t = 53; /* for ftsave and ftload  */
pub const CSFTYPE_XSCANU_MATRIX: CSOUND_FILETYPES_t = 54; /* for xscanu opcode  */

/* These are for raw lists of numbers without header info */
pub const CSFTYPE_FLOATS_TEXT: CSOUND_FILETYPES_t = 55; /* used by GEN23, GEN28, dumpk, readk */
pub const CSFTYPE_FLOATS_BINARY: CSOUND_FILETYPES_t = 56; /* used by dumpk, readk, etc. */
pub const CSFTYPE_INTEGER_TEXT: CSOUND_FILETYPES_t = 57; /* used by dumpk, readk, etc. */
pub const CSFTYPE_INTEGER_BINARY: CSOUND_FILETYPES_t = 58; /* used by dumpk, readk, etc. */

/* image file formats */
pub const CSFTYPE_IMAGE_PNG: CSOUND_FILETYPES_t = 59;

/* For files that don't match any of the above */
pub const CSFTYPE_POSTSCRIPT: CSOUND_FILETYPES_t = 60; /* EPS format used by graphs */
pub const CSFTYPE_SCRIPT_TEXT: CSOUND_FILETYPES_t = 61; /* executable script files (eg. Python) */
pub const CSFTYPE_OTHER_TEXT: CSOUND_FILETYPES_t = 62;
pub const CSFTYPE_OTHER_BINARY: CSOUND_FILETYPES_t = 63;

/* This should only be used internally by the original FileOpen()
API call or for temp files written with <CsFileB> */
pub const CSFTYPE_UNKNOWN: CSOUND_FILETYPES_t = 0;

//pub type CSOUND = CSOUND_;
pub enum CSOUND {}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct windat_ {
    _unused: [u8; 0],
}
pub type WINDAT = windat_;

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Debug, Copy, Clone)]
pub struct CSOUND_PARAMS {
    pub debug_mode: c_int,
    pub buffer_frames: c_int,
    pub hardware_buffer_frames: c_int,
    pub displays: c_int,
    pub ascii_graphs: c_int,
    pub postscript_graphs: c_int,
    pub message_level: c_int,
    pub tempo: c_int,
    pub ring_bell: c_int,
    pub use_cscore: c_int,
    pub terminate_on_midi: c_int,
    pub heartbeat: c_int,
    pub defer_gen01_load: c_int,
    pub midi_key: c_int,
    pub midi_key_cps: c_int,
    pub midi_key_oct: c_int,
    pub midi_key_pch: c_int,
    pub midi_velocity: c_int,
    pub midi_velocity_amp: c_int,
    pub no_default_paths: c_int,
    pub number_of_threads: c_int,
    pub syntax_check_only: c_int,
    pub csd_line_counts: c_int,
    pub compute_weights: c_int,
    pub realtime_mode: c_int,
    pub sample_accurate: c_int,
    pub sample_rate_override: c_double,
    pub control_rate_override: c_double,
    pub nchnls_override: c_int,
    pub nchnls_i_override: c_int,
    pub e0dbfs_override: c_double,
    pub daemon: c_int,
    pub ksmps_override: c_int,
    pub FFT_library: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ORCTOKEN {
    pub type_: c_int,
    pub lexeme: *mut c_char,
    pub value: c_int,
    pub fvalue: c_double,
    pub optype: *mut c_char,
    pub next: *mut ORCTOKEN,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Tree {
    pub type_: c_int,
    pub value: *mut ORCTOKEN,
    pub rate: c_int,
    pub len: c_int,
    pub line: c_int,
    pub locn: u64,
    pub left: *mut Tree,
    pub right: *mut Tree,
    pub next: *mut Tree,
    pub markup: *mut c_void,
}

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Copy, Clone)]
pub struct CS_AUDIODEVICE {
    pub device_name: [c_char; 64usize],
    pub device_id: [c_char; 64usize],
    pub rt_module: [c_char; 64usize],
    pub max_nchnls: c_int,
    pub isOutput: c_int,
}

impl Default for CS_AUDIODEVICE {
    fn default() -> CS_AUDIODEVICE {
        CS_AUDIODEVICE {
            device_name: [0i8; 64usize],
            device_id: [0i8; 64usize],
            rt_module: [0i8; 64usize],
            max_nchnls: 0,
            isOutput: 0,
        }
    }
}

pub type PVSDATEXT = pvsdat_ext;

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Debug, Copy, Clone)]
pub struct csRtAudioParams {
    pub devName: *mut c_char,
    pub devNum: c_int,
    pub bufSamp_SW: c_uint,
    pub bufSamp_HW: c_int,
    pub nChannels: c_int,
    pub sampleFormat: c_int,
    pub sampleRate: c_float,
}

impl Default for csRtAudioParams {
    fn default() -> csRtAudioParams {
        csRtAudioParams {
            devName: ptr::null_mut(),
            devNum: 0,
            bufSamp_SW: 0,
            bufSamp_HW: 0,
            nChannels: 0,
            sampleFormat: 0,
            sampleRate: 0.0,
        }
    }
}

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Copy, Clone)]
pub struct CS_MIDIDEVICE {
    pub device_name: [c_char; 64usize],
    pub interface_name: [c_char; 64usize],
    pub device_id: [c_char; 64usize],
    pub midi_module: [c_char; 64usize],
    pub isOutput: c_int,
}

impl Default for CS_MIDIDEVICE {
    fn default() -> CS_MIDIDEVICE {
        CS_MIDIDEVICE {
            device_name: [0i8; 64usize],
            interface_name: [0i8; 64usize],
            device_id: [0i8; 64usize],
            midi_module: [0i8; 64usize],
            isOutput: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct controlChannelHints_s {
    pub behav: controlChannelBehavior,
    pub dflt: c_double,
    pub min: c_double,
    pub max: c_double,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub attributes: *mut c_char,
}

impl Default for controlChannelHints_s {
    fn default() -> controlChannelHints_s {
        controlChannelHints_s {
            behav: CSOUND_CONTROL_CHANNEL_NO_HINTS,
            dflt: 0 as c_double,
            min: 0 as c_double,
            max: 0 as c_double,
            x: 0 as c_int,
            y: 0 as c_int,
            width: 0 as c_int,
            height: 0 as c_int,
            attributes: ::std::ptr::null_mut(),
        }
    }
}
pub type controlChannelHints_t = controlChannelHints_s;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct controlChannelInfo_s {
    pub name: *mut c_char,
    pub type_: c_int,
    pub hints: controlChannelHints_t,
}
pub type controlChannelInfo_t = controlChannelInfo_s;

impl Default for controlChannelInfo_s {
    fn default() -> controlChannelInfo_s {
        controlChannelInfo_s {
            name: ::std::ptr::null_mut(),
            type_: 0 as c_int,
            hints: controlChannelHints_t::default(),
        }
    }
}

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Debug, Copy, Clone, Default)]
pub struct RTCLOCK_S {
    pub starttime_real: c_long,
    pub starttime_CPU: c_long,
}

pub type RTCLOCK = RTCLOCK_S;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct opcodeListEntry {
    pub opname: *mut c_char,
    pub outypes: *mut c_char,
    pub intypes: *mut c_char,
    pub flags: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CsoundRandMTState_ {
    pub mti: c_int,
    pub mt: [u32; 624usize],
}

pub type CsoundRandMTState = CsoundRandMTState_;

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct pvsdat_ext {
    pub N: c_int,
    pub sliding: c_int,
    pub NB: c_int,
    pub overlap: c_int,
    pub winsize: c_int,
    pub wintype: c_int,
    pub format: c_int,
    pub framecount: c_uint,
    pub frame: *mut c_float,
}
impl Default for pvsdat_ext {
    fn default() -> pvsdat_ext {
        pvsdat_ext {
            N: 0,
            sliding: 0,
            NB: 0,
            overlap: 0,
            winsize: 0,
            wintype: 0,
            format: 0,
            framecount: 0,
            frame: ptr::null_mut(),
        }
    }
}

extern "C" {
    pub fn csoundLocalizeString(s: *const c_char) -> *mut c_char;

    /* Csound instantiation functions ******************************************************** */
    pub fn csoundInitialize(flags: c_int) -> c_int;

    pub fn csoundCreate(hostData: *mut c_void) -> *mut CSOUND;

    pub fn csoundDestroy(arg1: *mut CSOUND);

    pub fn csoundGetVersion() -> c_int;

    pub fn csoundGetAPIVersion() -> c_int;

    /* Csound performance functions ********************************************************* */

    pub fn csoundParseOrc(csound: *mut CSOUND, str: *const c_char) -> *mut Tree;

    pub fn csoundCompileTree(csound: *mut CSOUND, root: *mut Tree) -> c_int;

    pub fn csoundDeleteTree(csound: *mut CSOUND, tree: *mut Tree);

    pub fn csoundCompileOrc(csound: *mut CSOUND, str: *const c_char) -> c_int;

    pub fn csoundCompileOrcAsync(csound: *mut CSOUND, str: *const c_char) -> c_int;

    pub fn csoundEvalCode(csound: *mut CSOUND, str: *const c_char) -> c_double;

    pub fn csoundCompileArgs(arg1: *mut CSOUND, argc: c_int, argv: *const *const c_char) -> c_int;

    pub fn csoundStart(csound: *mut CSOUND) -> c_int;

    pub fn csoundCompile(arg1: *mut CSOUND, argc: c_int, argv: *const *const c_char) -> c_int;

    pub fn csoundCompileCsd(csound: *mut CSOUND, str: *const c_char) -> c_int;

    pub fn csoundCompileCsdText(csound: *mut CSOUND, csd_text: *const c_char) -> c_int;

    pub fn csoundPerform(arg1: *mut CSOUND) -> c_int;

    pub fn csoundPerformKsmps(arg1: *mut CSOUND) -> c_int;

    pub fn csoundPerformBuffer(arg1: *mut CSOUND) -> c_int;

    pub fn csoundStop(arg1: *mut CSOUND);

    pub fn csoundCleanup(arg1: *mut CSOUND) -> c_int;

    pub fn csoundReset(arg1: *mut CSOUND);

    /* UDP functions ************************************************************************/
    pub fn csoundUDPServerStart(csound: *mut CSOUND, port: c_int) -> c_int;
    pub fn csoundUDPServerStatus(csound: *mut CSOUND) -> c_int;
    pub fn csoundUDPServerClose(csound: *mut CSOUND) -> c_int;
    pub fn csoundUDPConsole(
        csound: *mut CSOUND,
        addr: *const c_char,
        port: c_int,
        mirror: c_int,
    ) -> c_int;
    pub fn csoundStopUDPConsole(csound: *mut CSOUND);
    /* Csound atributtes functions ***********************************************************/

    pub fn csoundGetA4(arg1: *mut CSOUND) -> c_double;

    pub fn csoundGetSr(arg1: *mut CSOUND) -> c_double;

    pub fn csoundGetKr(arg1: *mut CSOUND) -> c_double;

    pub fn csoundGetKsmps(arg1: *mut CSOUND) -> u32;

    pub fn csoundGetNchnls(arg1: *mut CSOUND) -> u32;

    pub fn csoundGetNchnlsInput(csound: *mut CSOUND) -> u32;

    pub fn csoundGet0dBFS(arg1: *mut CSOUND) -> c_double;

    pub fn csoundGetCurrentTimeSamples(csound: *mut CSOUND) -> i64;

    pub fn csoundGetSizeOfMYFLT() -> c_int;

    pub fn csoundGetHostData(arg1: *mut CSOUND) -> *mut c_void;

    pub fn csoundSetHostData(arg1: *mut CSOUND, hostData: *mut c_void);

    pub fn csoundSetOption(csound: *mut CSOUND, option: *const c_char) -> c_int;

    pub fn csoundSetParams(csound: *mut CSOUND, p: *mut CSOUND_PARAMS);

    pub fn csoundGetParams(csound: *mut CSOUND, p: *mut CSOUND_PARAMS);

    pub fn csoundGetDebug(arg1: *mut CSOUND) -> c_int;

    pub fn csoundSetDebug(arg1: *mut CSOUND, debug: c_int);

    /* Csound input/output functions **********************************************************/

    pub fn csoundGetOutputName(arg1: *mut CSOUND) -> *const c_char;
    pub fn csoundGetInputName(arg1: *mut CSOUND) -> *const c_char;

    pub fn csoundSetOutput(
        csound: *mut CSOUND,
        name: *const c_char,
        type_: *const c_char,
        format: *const c_char,
    );

    pub fn csoundGetOutputFormat(csound: *mut CSOUND, type_: *mut c_char, format: *mut c_char);

    pub fn csoundSetInput(csound: *mut CSOUND, name: *const c_char);

    pub fn csoundSetMIDIInput(csound: *mut CSOUND, name: *const c_char);

    pub fn csoundSetMIDIFileInput(csound: *mut CSOUND, name: *const c_char);

    pub fn csoundSetMIDIOutput(csound: *mut CSOUND, name: *const c_char);

    pub fn csoundSetMIDIFileOutput(csound: *mut CSOUND, name: *const c_char);

    pub fn csoundSetFileOpenCallback(
        p: *mut CSOUND,
        open_callback: Option<csound_file_open_callback>,
    );

    /* Csound realtime audio I/O functions ***************************************************/

    pub fn csoundSetRTAudioModule(csound: *mut CSOUND, module: *const c_char);

    pub fn csoundGetModule(
        csound: *mut CSOUND,
        number: c_int,
        name: *mut *mut c_char,
        type_: *mut *mut c_char,
    ) -> c_int;

    pub fn csoundGetInputBufferSize(arg1: *mut CSOUND) -> c_long;

    pub fn csoundGetOutputBufferSize(arg1: *mut CSOUND) -> c_long;

    pub fn csoundGetInputBuffer(arg1: *mut CSOUND) -> *mut c_void;

    pub fn csoundGetOutputBuffer(arg1: *mut CSOUND) -> *const c_void;

    pub fn csoundGetSpin(arg1: *mut CSOUND) -> *mut c_void;

    pub fn csoundClearSpin(arg1: *mut CSOUND);

    pub fn csoundAddSpinSample(csound: *mut CSOUND, frame: c_int, channel: c_int, sample: c_double);

    pub fn csoundSetSpinSample(csound: *mut CSOUND, frame: c_int, channel: c_int, sample: c_double);

    pub fn csoundGetSpout(csound: *mut CSOUND) -> *mut c_void;

    pub fn csoundGetSpoutSample(csound: *mut CSOUND, frame: c_int, channel: c_int) -> c_double;

    pub fn csoundGetRtRecordUserData(arg1: *mut CSOUND) -> *mut *mut c_void;

    pub fn csoundGetRtPlayUserData(arg1: *mut CSOUND) -> *mut *mut c_void;

    pub fn csoundSetHostImplementedAudioIO(arg1: *mut CSOUND, state: c_int, bufSize: c_int);

    pub fn csoundGetAudioDevList(
        csound: *mut CSOUND,
        list: *mut CS_AUDIODEVICE,
        isOutput: c_int,
    ) -> c_int;

    pub fn csoundSetPlayopenCallback(arg1: *mut CSOUND, func: csound_open_callback);

    pub fn csoundSetRecopenCallback(arg1: *mut CSOUND, func: csound_open_callback);

    pub fn csoundSetRtplayCallback(arg1: *mut CSOUND, func: csound_rt_play_callback);

    pub fn csoundSetRtrecordCallback(arg1: *mut CSOUND, func: csound_rt_rec_callback);

    pub fn csoundSetRtcloseCallback(arg1: *mut CSOUND, func: csound_rt_close_callback);

    pub fn csoundSetAudioDeviceListCallback(csound: *mut CSOUND, func: csound_dev_list_callback);

    /* Csound realtime midi I/O **************************************************************/

    pub fn csoundSetMIDIModule(csound: *mut CSOUND, module: *const c_char);

    pub fn csoundSetHostImplementedMIDIIO(csound: *mut CSOUND, state: c_int);

    pub fn csoundGetMIDIDevList(
        csound: *mut CSOUND,
        list: *mut CS_MIDIDEVICE,
        isOutput: c_int,
    ) -> c_int;

    pub fn csoundSetExternalMidiInOpenCallback(
        arg1: *mut CSOUND,
        func: csound_ext_midi_open_callback,
    );

    pub fn csoundSetExternalMidiOutOpenCallback(
        arg1: *mut CSOUND,
        func: csound_ext_midi_open_callback,
    );

    pub fn csoundSetExternalMidiReadCallback(
        arg1: *mut CSOUND,
        func: csound_ext_midi_read_data_callback,
    );

    pub fn csoundSetExternalMidiWriteCallback(
        arg1: *mut CSOUND,
        func: csound_ext_midi_write_data_callback,
    );

    pub fn csoundSetExternalMidiInCloseCallback(
        arg1: *mut CSOUND,
        func: csound_ext_midi_close_callback,
    );

    pub fn csoundSetExternalMidiOutCloseCallback(
        arg1: *mut CSOUND,
        func: csound_ext_midi_close_callback,
    );

    pub fn csoundSetExternalMidiErrorStringCallback(
        arg1: *mut CSOUND,
        func: csound_ext_midi_error_callback,
    );

    pub fn csoundSetMIDIDeviceListCallback(
        csound: *mut CSOUND,
        func: csound_midi_dev_list_callback,
    );

    /* Csound score handling functions ********************************************************/

    pub fn csoundReadScore(csound: *mut CSOUND, str: *const c_char) -> c_int;

    pub fn csoundReadScoreAsync(csound: *mut CSOUND, str: *const c_char);

    pub fn csoundGetScoreTime(arg1: *mut CSOUND) -> c_double;

    pub fn csoundIsScorePending(arg1: *mut CSOUND) -> c_int;

    pub fn csoundSetScorePending(arg1: *mut CSOUND, pending: c_int);

    pub fn csoundGetScoreOffsetSeconds(arg1: *mut CSOUND) -> c_double;

    pub fn csoundSetScoreOffsetSeconds(arg1: *mut CSOUND, time: c_double);

    pub fn csoundRewindScore(arg1: *mut CSOUND);

    pub fn csoundScoreSort(arg1: *mut CSOUND, input: *const FILE, out: *mut FILE) -> c_int;

    pub fn csoundScoreExtract(
        arg1: *mut CSOUND,
        input: *const FILE,
        out: *mut FILE,
        extract: *const FILE,
    ) -> c_int;

    /* Csound messages and text functions *****************************************************/

    pub fn csoundMessage(arg1: *mut CSOUND, format: *const c_char, ...);

    pub fn csoundSetMessageStringCallback(arg1: *mut CSOUND, callback: csound_message_callback);

    pub fn csoundGetMessageLevel(arg1: *mut CSOUND) -> c_int;

    pub fn csoundSetMessageLevel(arg1: *mut CSOUND, messageLevel: c_int);

    pub fn csoundCreateMessageBuffer(csound: *mut CSOUND, toStdOut: c_int);

    pub fn csoundGetFirstMessage(csound: *mut CSOUND) -> *const c_char;

    pub fn csoundGetFirstMessageAttr(csound: *mut CSOUND) -> c_int;

    pub fn csoundPopFirstMessage(csound: *mut CSOUND);

    pub fn csoundGetMessageCnt(csound: *mut CSOUND) -> c_int;

    pub fn csoundDestroyMessageBuffer(csound: *mut CSOUND);

    pub fn csoundGetChannelPtr(
        arg1: *mut CSOUND,
        p: *mut *mut c_double,
        name: *const c_char,
        type_: c_int,
    ) -> c_int;

    pub fn csoundListChannels(arg1: *mut CSOUND, lst: *mut *mut controlChannelInfo_t) -> c_int;

    pub fn csoundDeleteChannelList(arg1: *mut CSOUND, lst: *mut controlChannelInfo_t);

    pub fn csoundSetControlChannelHints(
        arg1: *mut CSOUND,
        name: *const c_char,
        hints: controlChannelHints_t,
    ) -> c_int;

    pub fn csoundGetControlChannelHints(
        arg1: *mut CSOUND,
        name: *const c_char,
        hints: *mut controlChannelHints_t,
    ) -> c_int;

    pub fn csoundGetChannelLock(arg1: *mut CSOUND, name: *const c_char) -> *mut c_int;

    pub fn csoundGetControlChannel(
        csound: *mut CSOUND,
        name: *const c_char,
        err: *mut c_int,
    ) -> c_double;

    pub fn csoundSetControlChannel(csound: *mut CSOUND, name: *const c_char, val: c_double);

    pub fn csoundGetAudioChannel(csound: *mut CSOUND, name: *const c_char, samples: *mut c_double);

    pub fn csoundSetAudioChannel(csound: *mut CSOUND, name: *const c_char, samples: *mut c_double);

    pub fn csoundGetStringChannel(csound: *mut CSOUND, name: *const c_char, string: *mut c_char);

    pub fn csoundSetStringChannel(csound: *mut CSOUND, name: *const c_char, string: *mut c_char);

    pub fn csoundGetChannelDatasize(csound: *mut CSOUND, name: *const c_char) -> c_int;

    pub fn csoundSetInputChannelCallback(
        csound: *mut CSOUND,
        inputChannelCalback: Option<csound_channel_callback>,
    );

    pub fn csoundSetOutputChannelCallback(
        csound: *mut CSOUND,
        outputChannelCalback: Option<csound_channel_callback>,
    );

    pub fn csoundSetPvsChannel(
        arg1: *mut CSOUND,
        fin: *const PVSDATEXT,
        name: *const c_char,
    ) -> c_int;

    pub fn csoundGetPvsChannel(
        csound: *mut CSOUND,
        fout: *mut PVSDATEXT,
        name: *const c_char,
    ) -> c_int;

    pub fn csoundScoreEvent(
        arg1: *mut CSOUND,
        type_: c_char,
        pFields: *const c_double,
        numFields: c_long,
    ) -> c_int;

    pub fn csoundScoreEventAbsolute(
        arg1: *mut CSOUND,
        type_: c_char,
        pfields: *const c_double,
        numFields: c_long,
        time_ofs: c_double,
    ) -> c_int;

    pub fn csoundScoreEventAsync(
        arg1: *mut CSOUND,
        type_: c_char,
        pFields: *const c_double,
        numFields: c_long,
    ) -> c_int;

    pub fn csoundScoreEventAbsoluteAsync(
        arg1: *mut CSOUND,
        type_: c_char,
        pfields: *const c_double,
        numFields: c_long,
        time_ofs: c_double,
    ) -> c_int;

    pub fn csoundInputMessage(arg1: *mut CSOUND, message: *const c_char);

    pub fn csoundInputMessageAsync(arg1: *mut CSOUND, message: *const c_char);

    pub fn csoundKillInstance(
        arg1: *mut CSOUND,
        arg2: c_double,
        arg3: *const c_char,
        arg4: c_int,
        arg5: c_int,
    ) -> c_int;

    pub fn csoundRegisterSenseEventCallback(
        arg1: *mut CSOUND,
        func: ::std::option::Option<unsafe extern "C" fn(arg1: *mut CSOUND, arg2: *mut c_void)>,
        userData: *mut c_void,
    ) -> c_int;

    pub fn csoundKeyPress(arg1: *mut CSOUND, c: c_char);

    pub fn csoundRegisterKeyboardCallback(
        arg1: *mut CSOUND,
        func: ::std::option::Option<
            unsafe extern "C" fn(userData: *mut c_void, p: *mut c_void, type_: c_uint) -> c_int,
        >,
        userData: *mut c_void,
        type_: c_uint,
    ) -> c_int;

    pub fn csoundRemoveKeyboardCallback(
        csound: *mut CSOUND,
        func: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut c_void, arg2: *mut c_void, arg3: c_uint) -> c_int,
        >,
    );

    pub fn csoundTableLength(arg1: *mut CSOUND, table: c_int) -> c_int;

    pub fn csoundTableGet(arg1: *mut CSOUND, table: c_int, index: c_int) -> c_double;

    pub fn csoundTableSet(arg1: *mut CSOUND, table: c_int, index: c_int, value: c_double);

    pub fn csoundTableCopyOut(csound: *mut CSOUND, table: c_int, dest: *mut c_double);
    pub fn csoundTableCopyOutAsync(csound: *mut CSOUND, table: c_int, dest: *mut c_double);

    pub fn csoundTableCopyIn(csound: *mut CSOUND, table: c_int, src: *const c_double);
    pub fn csoundTableCopyInAsync(csound: *mut CSOUND, table: c_int, src: *const c_double);

    pub fn csoundGetTable(arg1: *mut CSOUND, tablePtr: *mut *mut c_double, tableNum: c_int) -> c_int;

    pub fn csoundGetTableArgs(
        csound: *mut CSOUND,
        argsPtr: *mut *mut c_double,
        tableNum: c_int,
    ) -> c_int;

    pub fn csoundIsNamedGEN(csound: *mut CSOUND, num: c_int) -> c_int;

    pub fn csoundGetNamedGEN(csound: *mut CSOUND, num: c_int, name: *mut c_char, len: c_int);

    pub fn csoundSetIsGraphable(arg1: *mut CSOUND, isGraphable: c_int) -> c_int;

    pub fn csoundSetMakeGraphCallback(
        arg1: *mut CSOUND,
        makeGraphCallback_: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut CSOUND, windat: *mut WINDAT, name: *const c_char),
        >,
    );

    pub fn csoundSetDrawGraphCallback(
        arg1: *mut CSOUND,
        drawGraphCallback_: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut CSOUND, windat: *mut WINDAT),
        >,
    );

    pub fn csoundSetKillGraphCallback(
        arg1: *mut CSOUND,
        killGraphCallback_: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut CSOUND, windat: *mut WINDAT),
        >,
    );

    pub fn csoundSetExitGraphCallback(
        arg1: *mut CSOUND,
        exitGraphCallback_: ::std::option::Option<unsafe extern "C" fn(arg1: *mut CSOUND) -> c_int>,
    );

    pub fn csoundGetNamedGens(arg1: *mut CSOUND) -> *mut c_void;

    pub fn csoundNewOpcodeList(arg1: *mut CSOUND, opcodelist: *mut *mut opcodeListEntry) -> c_int;

    pub fn csoundDisposeOpcodeList(arg1: *mut CSOUND, opcodelist: *mut opcodeListEntry);

    pub fn csoundAppendOpcode(
        arg1: *mut CSOUND,
        opname: *const c_char,
        dsblksiz: c_int,
        flags: c_int,
        thread: c_int,
        outypes: *const c_char,
        intypes: *const c_char,
        iopadr: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut CSOUND, arg2: *mut c_void) -> c_int,
        >,
        kopadr: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut CSOUND, arg2: *mut c_void) -> c_int,
        >,
        aopadr: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut CSOUND, arg2: *mut c_void) -> c_int,
        >,
    ) -> c_int;

    pub fn csoundSetYieldCallback(
        arg1: *mut CSOUND,
        yieldCallback_: ::std::option::Option<unsafe extern "C" fn(arg1: *mut CSOUND) -> c_int>,
    );

    pub fn csoundCreateThread(
        threadRoutine: ::std::option::Option<unsafe extern "C" fn(arg1: *mut c_void) -> usize>,
        userdata: *mut c_void,
    ) -> *mut c_void;

    pub fn csoundGetCurrentThreadId() -> *mut c_void;

    pub fn csoundJoinThread(thread: *mut c_void) -> usize;

    pub fn csoundCreateThreadLock() -> *mut c_void;

    pub fn csoundWaitThreadLock(lock: *mut c_void, milliseconds: usize) -> c_int;

    pub fn csoundWaitThreadLockNoTimeout(lock: *mut c_void);

    pub fn csoundNotifyThreadLock(lock: *mut c_void);

    pub fn csoundDestroyThreadLock(lock: *mut c_void);

    pub fn csoundCreateMutex(isRecursive: c_int) -> *mut c_void;

    pub fn csoundLockMutex(mutex_: *mut c_void);

    pub fn csoundLockMutexNoWait(mutex_: *mut c_void) -> c_int;

    pub fn csoundUnlockMutex(mutex_: *mut c_void);

    pub fn csoundDestroyMutex(mutex_: *mut c_void);

    pub fn csoundCreateBarrier(max: c_uint) -> *mut c_void;

    pub fn csoundDestroyBarrier(barrier: *mut c_void) -> c_int;

    pub fn csoundWaitBarrier(barrier: *mut c_void) -> c_int;

    pub fn csoundSleep(milliseconds: usize);

    pub fn csoundRunCommand(argv: *const *const c_char, noWait: c_int) -> c_long;

    pub fn csoundInitTimerStruct(arg1: *mut RTCLOCK);

    pub fn csoundGetRealTime(arg1: *mut RTCLOCK) -> c_double;

    pub fn csoundGetCPUTime(arg1: *mut RTCLOCK) -> c_double;

    pub fn csoundGetRandomSeedFromTime() -> u32;

    pub fn csoundSetLanguage(lang_code: csLenguage_t);

    pub fn csoundGetEnv(csound: *mut CSOUND, name: *const c_char) -> *const c_char;

    pub fn csoundSetGlobalEnv(name: *const c_char, value: *const c_char) -> c_int;

    pub fn csoundCreateGlobalVariable(
        arg1: *mut CSOUND,
        name: *const c_char,
        nbytes: usize,
    ) -> c_int;

    pub fn csoundQueryGlobalVariable(arg1: *mut CSOUND, name: *const c_char) -> *mut c_void;

    pub fn csoundQueryGlobalVariableNoCheck(arg1: *mut CSOUND, name: *const c_char) -> *mut c_void;

    pub fn csoundDestroyGlobalVariable(arg1: *mut CSOUND, name: *const c_char) -> c_int;

    pub fn csoundRunUtility(
        arg1: *mut CSOUND,
        name: *const c_char,
        argc: c_int,
        argv: *mut *mut c_char,
    ) -> c_int;

    pub fn csoundListUtilities(arg1: *mut CSOUND) -> *mut *mut c_char;

    pub fn csoundDeleteUtilityList(arg1: *mut CSOUND, lst: *mut *mut c_char);

    pub fn csoundGetUtilityDescription(arg1: *mut CSOUND, utilName: *const c_char)
        -> *const c_char;

    pub fn csoundRand31(seedVal: *mut c_int) -> c_int;

    pub fn csoundSeedRandMT(p: *mut CsoundRandMTState, initKey: *const u32, keyLength: u32);

    pub fn csoundRandMT(p: *mut CsoundRandMTState) -> u32;

    pub fn csoundCreateConfigurationVariable(
        csound: *mut CSOUND,
        name: *const c_char,
        p: *mut c_void,
        type_: c_int,
        flags: c_int,
        min: *mut c_void,
        max: *mut c_void,
        shortDesc: *const c_char,
        longDesc: *const c_char,
    ) -> c_int;

    pub fn csoundSetConfigurationVariable(
        csound: *mut CSOUND,
        name: *const c_char,
        value: *mut c_void,
    ) -> c_int;

    pub fn csoundParseConfigurationVariable(
        csound: *mut CSOUND,
        name: *const c_char,
        value: *const c_char,
    ) -> c_int;

    pub fn csoundCfgErrorCodeToString(errcode: c_int) -> *const c_char;

    pub fn csoundCreateCircularBuffer(
        csound: *mut CSOUND,
        numelem: c_int,
        elemsize: c_int,
    ) -> *mut c_void;

    pub fn csoundReadCircularBuffer(
        csound: *mut CSOUND,
        circular_buffer: *mut c_void,
        out: *mut c_void,
        items: c_int,
    ) -> c_int;

    pub fn csoundPeekCircularBuffer(
        csound: *mut CSOUND,
        circular_buffer: *mut c_void,
        out: *mut c_void,
        items: c_int,
    ) -> c_int;

    pub fn csoundWriteCircularBuffer(
        csound: *mut CSOUND,
        p: *mut c_void,
        inp: *const c_void,
        items: c_int,
    ) -> c_int;

    pub fn csoundFlushCircularBuffer(csound: *mut CSOUND, p: *mut c_void);

    pub fn csoundDestroyCircularBuffer(csound: *mut CSOUND, circularbuffer: *mut c_void);

    pub fn csoundOpenLibrary(library: *mut *mut c_void, libraryPath: *const c_char) -> c_int;

    pub fn csoundCloseLibrary(library: *mut c_void) -> c_int;

    pub fn csoundGetLibrarySymbol(library: *mut c_void, symbolName: *const c_char) -> *mut c_void;

    pub fn csoundSetCscoreCallback(csound: *mut CSOUND, call: cscore_callback_type);

}
