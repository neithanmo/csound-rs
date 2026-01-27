//! Error types for the csound crate.

use std::ffi::NulError;

use crate::enums::Status;

/// A specialized Result type for csound operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

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

    /// A csound operation returned an error status.
    #[error("csound error: {0:?}")]
    Csound(Status),

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
}


