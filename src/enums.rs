use bitflags::bitflags;

#[derive(Debug, PartialEq)]
/// An audio channel identifier
pub enum AudioChannel {}

#[derive(Debug, PartialEq)]
/// A control channel identifier
pub enum ControlChannel {}

#[derive(Debug, PartialEq)]
/// A string channel identifier
pub enum StrChannel {}

/// Define the type of csound messages
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum MessageType {
    /// Standard message.
    Default,
    /// Error message (initerror, perferror, etc.).
    Error,
    /// Orchestra opcodes (e.g. printks).
    Orch,
    /// Progress display and heartbeat characters.
    Realtime,
    /// Warning messages.
    Warning,
    /// Stdout messages.
    Stdout,
}

impl From<u32> for MessageType {
    fn from(value: u32) -> Self {
        match value {
            0x0000 => MessageType::Default,
            0x1000 => MessageType::Error,
            0x2000 => MessageType::Orch,
            0x3000 => MessageType::Realtime,
            0x4000 => MessageType::Warning,
            0x5000 => MessageType::Stdout,
            _ => MessageType::Error,
        }
    }
}

/// Csound error codes
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum Status {
    /// Termination requested by SIGINT or SIGTERM.
    Signal,
    /// Failed to allocate requested memory.
    Memory,
    /// Failed during performance.
    Performance,
    /// Failed during initialization.
    Initialization,
    /// Unspecified failure.
    Error,
    /// Completed successfully.
    Success,
    /// Completed but with additional info.
    Ok(i32),
}

impl From<i32> for Status {
    fn from(value: i32) -> Self {
        match value {
            -5 => Status::Signal,
            -4 => Status::Memory,
            -3 => Status::Performance,
            -2 => Status::Initialization,
            -1 => Status::Error,
            0 => Status::Success,
            value => Status::Ok(value),
        }
    }
}

impl Status {
    pub fn to_i32(&self) -> i32 {
        match self {
            Status::Signal => -5,
            Status::Memory => -4,
            Status::Performance => -3,
            Status::Initialization => -2,
            Status::Error => -1,
            Status::Success => 0,
            Status::Ok(value) => *value,
        }
    }
}

/// Enum variant which represent channel's types in callbacks.
///
/// Channels which could trigger a callback, that is, channels created using the [*invalue*](http://www.csounds.com/manual/html/invalue.html),
/// [*outvalue*](http://www.csounds.com/manual/html/outvalue.html) opcodes. Only control and string channels are supported.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelData {
    /// Control channel data (single f64 value).
    Control(f64),
    /// String channel data.
    String(String),
    /// Unknown channel type.
    Unknown,
}

bitflags! {
    /// Defines the types of csound bus channels and their direction.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ControlChannelType: u32 {
        /// Unknown channel - use to request the channel type.
        const Unknown = 0;
        /// Control channel (single MYFLT value).
        const Control = 1;
        /// Audio channel (array with ksmps elements).
        const Audio = 2;
        /// String channel.
        const String = 3;
        /// PVS channel.
        const Pvs = 4;
        /// Generic/variable channel.
        const Var = 5;
        /// Mask to extract channel type.
        const TypeMask = 15;
        /// Input channel flag.
        const Input = 16;
        /// Output channel flag.
        const Output = 32;
    }
}

bitflags! {
    /// Keyboard callback types.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct KeyCallbackType: u8 {
        /// Keyboard event callback.
        const Event = 1;
        /// Keyboard text callback.
        const Text = 2;
    }
}

/// The languages supported by csound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Language {
    Default = 0,
    Afrikaans = 1,
    Albanian = 2,
    Arabic = 3,
    Armenian = 4,
    Assamese = 5,
    Azeri = 6,
    Basque = 7,
    Belarusian = 8,
    Bengali = 9,
    Bulgarian = 10,
    Catalan = 11,
    Chinese = 12,
    Croatian = 13,
    Czech = 14,
    Danish = 15,
    Dutch = 16,
    EnglishUk = 17,
    EnglishUs = 18,
    Estonian = 19,
    Faeroese = 20,
    Farsi = 21,
    Finnish = 22,
    French = 23,
    Georgian = 24,
    German = 25,
    Greek = 26,
    Gujarati = 27,
    Hebrew = 28,
    Hindi = 29,
    Hungarian = 30,
    Icelandic = 31,
    Indonesian = 32,
    Italian = 33,
    Japanese = 34,
    Kannada = 35,
    Kashmiri = 36,
    Konkani = 37,
    Korean = 38,
    Latvian = 39,
    Lithuanian = 40,
    Macedonian = 41,
    Malay = 42,
    Malayalam = 43,
    Manipuri = 44,
    Marathi = 45,
    Nepali = 46,
    Norwegian = 47,
    Oriya = 48,
    Polish = 49,
    Portuguese = 50,
    Punjabi = 51,
    Romanian = 52,
    Russian = 53,
    Sanskrit = 54,
    Serbian = 55,
    Sindhi = 56,
    Slovak = 57,
    Slovenian = 58,
    Spanish = 59,
    Swahili = 60,
    Swedish = 61,
    Tamil = 62,
    Tatar = 63,
    Telugu = 64,
    Thai = 65,
    Turkish = 66,
    Ukrainian = 67,
    Urdu = 68,
    Uzbek = 69,
    Vietnamese = 70,
    Columbian = 71,
}

/// Describes the different file types supported by csound.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FileTypes {
    /// Unknown file type (internal use or temp files).
    Unknown,
    /// Unified Csound document (.csd).
    UnifiedCSD,
    /// Primary orchestra file (may be temporary).
    Orchestra,
    /// Primary score file or additional score opened by Cscore.
    Score,
    /// File #included by the orchestra.
    OrcInclude,
    /// File #included by the score.
    ScoInclude,
    /// Output score files (score.srt, score.xtr, cscore.out).
    ScoreOut,
    /// Scot score input format.
    Scot,
    /// Options file (.csoundrc or -@ flag).
    Options,
    /// Extraction file specified by -x.
    ExtractParms,

    // Audio file types (10-36)
    /// Raw audio format.
    RawAudio,
    /// IRCAM format.
    Ircam,
    /// AIFF format.
    Aiff,
    /// AIFC format.
    Aifc,
    /// WAV format.
    Wave,
    /// AU format.
    Au,
    /// SD2 format.
    Sd2,
    /// W64 format.
    W64,
    /// WAVEX format.
    WaveX,
    /// FLAC format.
    Flac,
    /// CAF format.
    Caf,
    /// WVE format.
    Wve,
    /// OGG format.
    Ogg,
    /// MPC2K format.
    Mpc2K,
    /// RF64 format.
    Rf64,
    /// AVR format.
    Avr,
    /// HTK format.
    Htk,
    /// MAT4 format.
    Mat4,
    /// MAT5 format.
    Mat5,
    /// NIST format.
    Nist,
    /// PAF format.
    Paf,
    /// PVF format.
    Pvf,
    /// SDS format.
    Sds,
    /// SVX format.
    Svx,
    /// VOC format.
    Voc,
    /// XI format.
    Xi,
    /// Unknown audio format (reading audio or <CsSampleB> temp).
    UnknownAudio,

    // Miscellaneous music formats (37-39)
    /// SoundFont format.
    Soundfont,
    /// Standard MIDI file.
    StdMidi,
    /// Raw MIDI codes (e.g. SysEx dump).
    MidiSysex,

    // Analysis formats (40-51)
    /// Hetro analysis format.
    Hetro,
    /// Hetrot analysis format.
    Hetrot,
    /// Original PVOC format.
    Pvc,
    /// PVOC-EX format.
    PvcEx,
    /// CVANAL format.
    Cvanal,
    /// LPC format.
    Lpc,
    /// ATS format.
    Ats,
    /// Loris format.
    Loris,
    /// SDIF format.
    Sdif,
    /// HRTF format.
    Hrtf,

    // Plugin types (52-54)
    /// Unused type.
    Unused,
    /// LADSPA plugin.
    LadspaPlugin,
    /// Snapshot file.
    Snapshot,

    // Ftable and matrix formats (55-57)
    /// Text format for ftsave/ftload.
    FtablesText,
    /// Binary format for ftsave/ftload.
    FtablesBinary,
    /// Matrix file for xscanu opcode.
    XscanuMatrix,

    // Raw number lists (58-61)
    /// Text floats (GEN23, GEN28, dumpk, readk).
    FloatsText,
    /// Binary floats (dumpk, readk, etc.).
    FloatsBinary,
    /// Text integers (dumpk, readk, etc.).
    IntegerText,
    /// Binary integers (dumpk, readk, etc.).
    IntegerBinary,

    // Image formats (62)
    /// PNG image format.
    ImagePng,

    // Other formats (63-66)
    /// PostScript/EPS format (graphs).
    Postscript,
    /// Executable script files (e.g. Python).
    ScriptText,
    /// Other text format.
    OtherText,
    /// Other binary format.
    OtherBinary,
}

impl From<u8> for FileTypes {
    fn from(value: u8) -> Self {
        match value {
            0 => FileTypes::Unknown,
            1 => FileTypes::UnifiedCSD,
            2 => FileTypes::Orchestra,
            3 => FileTypes::Score,
            4 => FileTypes::OrcInclude,
            5 => FileTypes::ScoInclude,
            6 => FileTypes::ScoreOut,
            7 => FileTypes::Scot,
            8 => FileTypes::Options,
            9 => FileTypes::ExtractParms,
            10 => FileTypes::RawAudio,
            11 => FileTypes::Ircam,
            12 => FileTypes::Aiff,
            13 => FileTypes::Aifc,
            14 => FileTypes::Wave,
            15 => FileTypes::Au,
            16 => FileTypes::Sd2,
            17 => FileTypes::W64,
            18 => FileTypes::WaveX,
            19 => FileTypes::Flac,
            20 => FileTypes::Caf,
            21 => FileTypes::Wve,
            22 => FileTypes::Ogg,
            23 => FileTypes::Mpc2K,
            24 => FileTypes::Rf64,
            25 => FileTypes::Avr,
            26 => FileTypes::Htk,
            27 => FileTypes::Mat4,
            28 => FileTypes::Mat5,
            29 => FileTypes::Nist,
            30 => FileTypes::Paf,
            31 => FileTypes::Pvf,
            32 => FileTypes::Sds,
            33 => FileTypes::Svx,
            34 => FileTypes::Voc,
            35 => FileTypes::Xi,
            36 => FileTypes::UnknownAudio,
            37 => FileTypes::Soundfont,
            38 => FileTypes::StdMidi,
            39 => FileTypes::MidiSysex,
            40 => FileTypes::Hetro,
            41 => FileTypes::Hetrot,
            42 => FileTypes::Pvc,
            43 => FileTypes::PvcEx,
            44 => FileTypes::Cvanal,
            45 => FileTypes::Lpc,
            46 => FileTypes::Ats,
            47 => FileTypes::Loris,
            48 => FileTypes::Sdif,
            49 => FileTypes::Hrtf,
            50 => FileTypes::Unused,
            51 => FileTypes::LadspaPlugin,
            52 => FileTypes::Snapshot,
            53 => FileTypes::FtablesText,
            54 => FileTypes::FtablesBinary,
            55 => FileTypes::XscanuMatrix,
            56 => FileTypes::FloatsText,
            57 => FileTypes::FloatsBinary,
            58 => FileTypes::IntegerText,
            59 => FileTypes::IntegerBinary,
            60 => FileTypes::ImagePng,
            61 => FileTypes::Postscript,
            62 => FileTypes::ScriptText,
            63 => FileTypes::OtherText,
            64 => FileTypes::OtherBinary,
            _ => FileTypes::Unknown,
        }
    }
}
