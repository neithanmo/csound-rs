//! Error types for the csound crate.

use std::ffi::NulError;
use std::str::Utf8Error;

use crate::enums::{ControlChannelType, Status};

/// A specialized Result type for csound operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Represents successful csound operation results that may carry additional information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsoundStatus {
    /// Operation completed successfully.
    Success,
    /// Operation completed with additional info (e.g., channel type, count).
    Done(i32),
}

/// A Csound status code returned by the C API.
///
/// Unknown values are retained so callers do not lose information when linked
/// against a newer or otherwise unexpected Csound build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CsoundErrorCode {
    Signal,
    Memory,
    Performance,
    Initialization,
    Error,
    Unknown(i32),
}

impl CsoundErrorCode {
    /// Converts a raw Csound return code without discarding unknown values.
    pub const fn from_raw(code: i32) -> Self {
        match code {
            -5 => Self::Signal,
            -4 => Self::Memory,
            -3 => Self::Performance,
            -2 => Self::Initialization,
            -1 => Self::Error,
            code => Self::Unknown(code),
        }
    }

    /// Returns the underlying Csound return code.
    pub const fn as_raw(self) -> i32 {
        match self {
            Self::Signal => -5,
            Self::Memory => -4,
            Self::Performance => -3,
            Self::Initialization => -2,
            Self::Error => -1,
            Self::Unknown(code) => code,
        }
    }
}

impl std::fmt::Display for CsoundErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Signal => "CSOUND_SIGNAL",
            Self::Memory => "CSOUND_MEMORY",
            Self::Performance => "CSOUND_PERFORMANCE",
            Self::Initialization => "CSOUND_INITIALIZATION",
            Self::Error => "CSOUND_ERROR",
            Self::Unknown(_) => "unknown Csound status",
        };
        write!(f, "{name} ({})", self.as_raw())
    }
}

impl std::error::Error for CsoundErrorCode {}

/// Integer domains used by the Rust-to-Csound FFI boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntegerTarget {
    /// C's signed `int` type.
    CInt,
    /// A signed 32-bit integer used explicitly by the Csound API.
    I32,
    /// Rust's pointer-sized unsigned integer.
    Usize,
    /// The valid UDP port domain (`0..=65535`).
    UdpPort,
    /// The portable non-negative C enum domain.
    PortableCEnum,
}

impl std::fmt::Display for IntegerTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::CInt => "c_int",
            Self::I32 => "i32",
            Self::Usize => "usize",
            Self::UdpPort => "UDP port (0..=65535)",
            Self::PortableCEnum => "portable C enum range",
        })
    }
}

/// Errors that can occur when using the csound library.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Failed to initialize the csound library.
    #[error("failed to initialize csound")]
    InitFailed,

    /// A null pointer was encountered where a valid pointer was expected.
    #[error("null pointer encountered: {0}")]
    NullPointer(&'static str),

    /// A string contained an interior NUL byte.
    #[error("string contains an interior NUL byte: {0}")]
    Nul(#[from] NulError),

    /// A string returned by Csound was not valid UTF-8.
    #[error("string contains invalid UTF-8: {0}")]
    UtfError(#[from] Utf8Error),

    /// An invalid option was passed to csound.
    #[error("invalid csound option: {0}")]
    InvalidOption(String),

    /// Csound has not been started yet.
    #[error("csound has not been started")]
    NotStarted,

    /// Failed to compile the provided code.
    #[error("{operation} failed with {status}")]
    CompileFailed {
        operation: &'static str,
        #[source]
        status: CsoundErrorCode,
    },

    /// The requested resource (table, channel, etc.) was not found.
    #[error("not found: {0}")]
    NotFound(&'static str),

    /// An invalid argument was provided.
    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),

    /// A non-negative Rust integer cannot be represented by the C ABI type.
    #[error("argument `{argument}` with value {value} cannot be represented as {target}")]
    IntegerOutOfRange {
        argument: &'static str,
        value: u128,
        target: IntegerTarget,
    },

    /// A size calculation overflowed or exceeded the range supported by Csound.
    #[error("size overflow while computing {context}")]
    SizeOverflow { context: &'static str },

    /// A value has a different type from the one required by the operation.
    #[error("type mismatch for {context}: expected {expected}, got {actual}")]
    TypeMismatch {
        context: &'static str,
        expected: &'static str,
        actual: String,
    },

    /// Csound is already started.
    #[error("csound is already started, call reset() before starting again")]
    AlreadyStarted,

    /// An internal buffer has not been initialized.
    #[error("buffer not initialized")]
    BufferNotInitialized,

    /// An empty string was provided where content was expected.
    #[error("empty string provided")]
    EmptyString,

    /// An invalid seed value was provided.
    #[error("invalid seed value: must be in range 1..=2147483646")]
    InvalidSeed,

    /// A buffer is shorter than the minimum required by the operation.
    #[error("{buffer} is too small: requires at least {required} elements, got {actual}")]
    BufferTooSmall {
        buffer: &'static str,
        required: usize,
        actual: usize,
    },

    /// A buffer must have exactly the requested length.
    #[error("{buffer} has the wrong length: expected {expected} elements, got {actual}")]
    BufferLengthMismatch {
        buffer: &'static str,
        expected: usize,
        actual: usize,
    },

    /// MYFLT size mismatch between Rust bindings and linked Csound library.
    #[error("MYFLT size mismatch: bindings use {expected} bytes, csound reports {actual} bytes")]
    MyfltMismatch { expected: usize, actual: usize },

    /// A channel with the same name but an incompatible type already exists.
    #[error(
        "channel `{name}` type mismatch: expected {expected:?}, existing channel has {actual:?}"
    )]
    ChannelTypeMismatch {
        name: String,
        expected: ControlChannelType,
        actual: ControlChannelType,
    },

    /// A Csound function returned a documented or otherwise recoverable error.
    /// The status is exposed through the standard error source chain.
    #[error("Csound operation `{operation}` failed with {status}")]
    CsoundCall {
        operation: &'static str,
        #[source]
        status: CsoundErrorCode,
    },

    /// Csound returned a value outside the function's documented contract.
    #[error("Csound function `{function}` returned unexpected value {value}")]
    UnexpectedCValue { function: &'static str, value: i64 },

    // Flattened from Status - csound C API error codes
    /// Termination requested by SIGINT or SIGTERM.
    #[error("termination requested by signal")]
    Signal,

    /// Failed to allocate requested memory.
    #[error("memory allocation failed")]
    Memory,

    /// Failed during performance.
    #[error("performance error")]
    Performance,

    /// Failed during initialization.
    #[error("initialization error")]
    Initialization,

    /// Unspecified csound error.
    #[error("csound operation failed")]
    OperationFailed,
}

impl Status {
    /// Converts a Status into a Result, mapping success/ok variants to Ok
    /// and error variants to Err.
    pub fn into_result(self) -> Result<CsoundStatus, Error> {
        match self {
            Status::Success => Ok(CsoundStatus::Success),
            Status::Ok(v) => Ok(CsoundStatus::Done(v)),
            Status::Signal => Err(Error::Signal),
            Status::Memory => Err(Error::Memory),
            Status::Performance => Err(Error::Performance),
            Status::Initialization => Err(Error::Initialization),
            Status::Error => Err(Error::OperationFailed),
        }
    }
}
