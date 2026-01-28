//! Error types for the csound crate.

use std::ffi::NulError;

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
            Status::CS_SUCCESS => Ok(CsoundStatus::Success),
            Status::CS_OK(v) => Ok(CsoundStatus::Done(v)),
            Status::CS_SIGNAL => Err(Error::Signal),
            Status::CS_MEMORY => Err(Error::Memory),
            Status::CS_PERFORMANCE => Err(Error::Performance),
            Status::CS_INITIALIZATION => Err(Error::Initialization),
            Status::CS_ERROR => Err(Error::OperationFailed),
        }
    }
}

