use std::mem::transmute;

#[derive(Debug, PartialEq)]
pub enum MessageType {
    CSOUNDMSG_DEFAULT,

    CSOUNDMSG_ERROR,

    CSOUNDMSG_ORCH,

    CSOUNDMSG_REALTIME,

    CSOUNDMSG_WARNING,

    CSOUNDMSG_STDOUT,
}

impl MessageType {
    pub fn from_u32(value: u32) -> MessageType {
        match value {
            0x0000 => MessageType::CSOUNDMSG_DEFAULT,
            0x1000 => MessageType::CSOUNDMSG_ERROR,
            0x2000 => MessageType::CSOUNDMSG_ORCH,
            0x3000 => MessageType::CSOUNDMSG_REALTIME,
            0x4000 => MessageType::CSOUNDMSG_WARNING,
            0x5000 => MessageType::CSOUNDMSG_STDOUT,
            _ => MessageType::CSOUNDMSG_ERROR,
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum Status {
    CS_SIGNAL,

    CS_MEMORY,

    CS_PERFORMANCE,

    CS_INITIALIZATION,

    CS_ERROR,

    CS_SUCCESS,

    CS_OK(i32),
}

impl From<i32> for Status {
    fn from(value: i32) -> Self {
        match value {
            -5 => Status::CS_SIGNAL,
            -4 => Status::CS_MEMORY,
            -3 => Status::CS_PERFORMANCE,
            -2 => Status::CS_INITIALIZATION,
            -1 => Status::CS_ERROR,
            0 => Status::CS_SUCCESS,
            value => Status::CS_OK(value),
        }
    }
}

impl Status {
    pub fn to_i32(&self) -> i32 {
        match self {
            Status::CS_SIGNAL => -5,
            Status::CS_MEMORY => -4,
            Status::CS_PERFORMANCE => -3,
            Status::CS_INITIALIZATION => -2,
            Status::CS_ERROR => -1,
            Status::CS_SUCCESS => 0,
            Status::CS_OK(value) => *value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelData {
    CS_CONTROL_CHANNEL(f64),
    CS_STRING_CHANNEL(String),
    CS_UNKNOWN_CHANNEL,
}

bitflags! {
    pub struct ControlChannelType: u32 {
        const CSOUND_UNKNOWN_CHANNEL =     0;

        const CSOUND_CONTROL_CHANNEL =     1;
        const CSOUND_AUDIO_CHANNEL  =      2;
        const CSOUND_STRING_CHANNEL =      3;
        const CSOUND_PVS_CHANNEL =         4;
        const CSOUND_VAR_CHANNEL =         5;

        const CSOUND_CHANNEL_TYPE_MASK =   15;

        const CSOUND_INPUT_CHANNEL =       16;

        const CSOUND_OUTPUT_CHANNEL =      32;
    }
}

bitflags! {
    pub struct KeyCallbackType: u8 {
        const CSOUND_CALLBACK_KBD_EVENT = 1;
        const CSOUND_CALLBACK_KBD_TEXT =  2;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    CSLANGUAGE_DEFAULT = 0,
    CSLANGUAGE_AFRIKAANS,
    CSLANGUAGE_ALBANIAN,
    CSLANGUAGE_ARABIC,
    CSLANGUAGE_ARMENIAN,
    CSLANGUAGE_ASSAMESE,
    CSLANGUAGE_AZERI,
    CSLANGUAGE_BASQUE,
    CSLANGUAGE_BELARUSIAN,
    CSLANGUAGE_BENGALI,
    CSLANGUAGE_BULGARIAN,
    CSLANGUAGE_CATALAN,
    CSLANGUAGE_CHINESE,
    CSLANGUAGE_CROATIAN,
    CSLANGUAGE_CZECH,
    CSLANGUAGE_DANISH,
    CSLANGUAGE_DUTCH,
    CSLANGUAGE_ENGLISH_UK,
    CSLANGUAGE_ENGLISH_US,
    CSLANGUAGE_ESTONIAN,
    CSLANGUAGE_FAEROESE,
    CSLANGUAGE_FARSI,
    CSLANGUAGE_FINNISH,
    CSLANGUAGE_FRENCH,
    CSLANGUAGE_GEORGIAN,
    CSLANGUAGE_GERMAN,
    CSLANGUAGE_GREEK,
    CSLANGUAGE_GUJARATI,
    CSLANGUAGE_HEBREW,
    CSLANGUAGE_HINDI,
    CSLANGUAGE_HUNGARIAN,
    CSLANGUAGE_ICELANDIC,
    CSLANGUAGE_INDONESIAN,
    CSLANGUAGE_ITALIAN,
    CSLANGUAGE_JAPANESE,
    CSLANGUAGE_KANNADA,
    CSLANGUAGE_KASHMIRI,
    CSLANGUAGE_KONKANI,
    CSLANGUAGE_KOREAN,
    CSLANGUAGE_LATVIAN,
    CSLANGUAGE_LITHUANIAN,
    CSLANGUAGE_MACEDONIAN,
    CSLANGUAGE_MALAY,
    CSLANGUAGE_MALAYALAM,
    CSLANGUAGE_MANIPURI,
    CSLANGUAGE_MARATHI,
    CSLANGUAGE_NEPALI,
    CSLANGUAGE_NORWEGIAN,
    CSLANGUAGE_ORIYA,
    CSLANGUAGE_POLISH,
    CSLANGUAGE_PORTUGUESE,
    CSLANGUAGE_PUNJABI,
    CSLANGUAGE_ROMANIAN,
    CSLANGUAGE_RUSSIAN,
    CSLANGUAGE_SANSKRIT,
    CSLANGUAGE_SERBIAN,
    CSLANGUAGE_SINDHI,
    CSLANGUAGE_SLOVAK,
    CSLANGUAGE_SLOVENIAN,
    CSLANGUAGE_SPANISH,
    CSLANGUAGE_SWAHILI,
    CSLANGUAGE_SWEDISH,
    CSLANGUAGE_TAMIL,
    CSLANGUAGE_TATAR,
    CSLANGUAGE_TELUGU,
    CSLANGUAGE_THAI,
    CSLANGUAGE_TURKISH,
    CSLANGUAGE_UKRAINIAN,
    CSLANGUAGE_URDU,
    CSLANGUAGE_UZBEK,
    CSLANGUAGE_VIETNAMESE,
    CSLANGUAGE_COLUMBIAN,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FileTypes {
    /* This should only be used internally by the original FileOpen()
    API call or for temp files written with <CsFileB> */
    CSFTYPE_UNKNOWN = 0,
    CSFTYPE_UNIFIED_CSD = 1, /* Unified Csound document */
    CSFTYPE_ORCHESTRA,       /* the primary orc file (may be temporary) */
    CSFTYPE_SCORE,           /* the primary sco file (may be temporary)
                             or any additional score opened by Cscore */
    CSFTYPE_ORC_INCLUDE,   /* a file #included by the orchestra */
    CSFTYPE_SCO_INCLUDE,   /* a file #included by the score */
    CSFTYPE_SCORE_OUT,     /* used for score.srt, score.xtr, cscore.out */
    CSFTYPE_SCOT,          /* Scot score input format */
    CSFTYPE_OPTIONS,       /* for .csoundrc and -@ flag */
    CSFTYPE_EXTRACT_PARMS, /* extraction file specified by -x */

    /* audio file types that Csound can write (10-19) or read */
    CSFTYPE_RAW_AUDIO,
    CSFTYPE_IRCAM,
    CSFTYPE_AIFF,
    CSFTYPE_AIFC,
    CSFTYPE_WAVE,
    CSFTYPE_AU,
    CSFTYPE_SD2,
    CSFTYPE_W64,
    CSFTYPE_WAVEX,
    CSFTYPE_FLAC,
    CSFTYPE_CAF,
    CSFTYPE_WVE,
    CSFTYPE_OGG,
    CSFTYPE_MPC2K,
    CSFTYPE_RF64,
    CSFTYPE_AVR,
    CSFTYPE_HTK,
    CSFTYPE_MAT4,
    CSFTYPE_MAT5,
    CSFTYPE_NIST,
    CSFTYPE_PAF,
    CSFTYPE_PVF,
    CSFTYPE_SDS,
    CSFTYPE_SVX,
    CSFTYPE_VOC,
    CSFTYPE_XI,
    CSFTYPE_UNKNOWN_AUDIO, /* used when opening audio file for reading
                           or temp file written with <CsSampleB> */

    /* miscellaneous music formats */
    CSFTYPE_SOUNDFONT,
    CSFTYPE_STD_MIDI,   /* Standard MIDI file */
    CSFTYPE_MIDI_SYSEX, /* Raw MIDI codes, eg. SysEx dump */

    /* analysis formats */
    CSFTYPE_HETRO,
    CSFTYPE_HETROT,
    CSFTYPE_PVC,   /* original PVOC format */
    CSFTYPE_PVCEX, /* PVOC-EX format */
    CSFTYPE_CVANAL,
    CSFTYPE_LPC,
    CSFTYPE_ATS,
    CSFTYPE_LORIS,
    CSFTYPE_SDIF,
    CSFTYPE_HRTF,

    /* Types for plugins and the files they read/write */
    CSFTYPE_UNUSED,
    CSFTYPE_LADSPA_PLUGIN,
    CSFTYPE_SNAPSHOT,

    /* Special formats for Csound ftables or scanned synthesis
    matrices with header info */
    CSFTYPE_FTABLES_TEXT,   /* for ftsave and ftload  */
    CSFTYPE_FTABLES_BINARY, /* for ftsave and ftload  */
    CSFTYPE_XSCANU_MATRIX,  /* for xscanu opcode  */

    /* These are for raw lists of numbers without header info */
    CSFTYPE_FLOATS_TEXT,    /* used by GEN23, GEN28, dumpk, readk */
    CSFTYPE_FLOATS_BINARY,  /* used by dumpk, readk, etc. */
    CSFTYPE_INTEGER_TEXT,   /* used by dumpk, readk, etc. */
    CSFTYPE_INTEGER_BINARY, /* used by dumpk, readk, etc. */

    /* image file formats */
    CSFTYPE_IMAGE_PNG,

    /* For files that don't match any of the above */
    CSFTYPE_POSTSCRIPT,  /* EPS format used by graphs */
    CSFTYPE_SCRIPT_TEXT, /* executable script files (eg. Python) */
    CSFTYPE_OTHER_TEXT,
    CSFTYPE_OTHER_BINARY,
}

impl From<u8> for FileTypes {
    fn from(item: u8) -> Self {
        if item > 63 {
            FileTypes::CSFTYPE_UNKNOWN
        } else {
            unsafe { transmute(item) }
        }
    }
}
