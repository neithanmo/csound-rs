//! Error types for the csound crate.

use std::ffi::NulError;
use std::str::Utf8Error;

use crate::enums::Status;

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

/// Errors that can occur when using the csound library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to initialize the csound library.
    #[error("failed to initialize csound")]
    InitFailed,

    /// A null pointer was encountered where a valid pointer was expected.
    #[error("null pointer encountered: {0}")]
    NullPointer(&'static str),

    /// A string contained an interior NUL byte.
    #[error("string contains interior NUL byte")]
    Nul(#[from] NulError),

    /// A Utf8 error encountered
    #[error("string contains interior Non Utf8 Characters: {0}")]
    UtfError(#[from] Utf8Error),

    /// An invalid option was passed to csound.
    #[error("invalid csound option: {0}")]
    InvalidOption(String),

    /// Csound has not been started yet.
    #[error("csound has not been started")]
    NotStarted,

    /// Failed to compile the provided code.
    #[error("compilation failed: {0}")]
    CompileFailed(&'static str),

    /// The requested resource (table, channel, etc.) was not found.
    #[error("not found: {0}")]
    NotFound(&'static str),

    /// An invalid argument was provided.
    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),

    /// Csound is already started.
    #[error("csound is already started, call reset() before starting again")]
    AlreadyStarted,

    /// An internal buffer has not been initialized.
    #[error("buffer not initialized: {0}")]
    BufferNotInitialized(&'static str),

    /// An empty string was provided where content was expected.
    #[error("empty string provided")]
    EmptyString,

    /// An invalid seed value was provided.
    #[error("invalid seed value: must be in range 1..=2147483646")]
    InvalidSeed,

    /// Insufficient buffer capacity for the requested operation.
    #[error("insufficient buffer capacity: expected {expected}, got {actual}")]
    InsufficientCapacity { expected: usize, actual: usize },

    /// MYFLT size mismatch between Rust bindings and linked Csound library.
    #[error("MYFLT size mismatch: bindings use {expected} bytes, csound reports {actual} bytes")]
    MyfltMismatch { expected: usize, actual: usize },

    /// A channel with the same name but incompatible type already exists.
    /// The contained value is the type of the existing channel.
    #[error("channel type mismatch: existing channel has type {0}")]
    ChannelTypeMismatch(i32),

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
