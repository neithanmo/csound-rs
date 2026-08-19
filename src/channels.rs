//! Typed wrappers for Csound's named channel system.
//!
//! The common handle and lock machinery lives in this module family while
//! each channel representation keeps its own access and sizing rules.

mod array;
mod audio;
mod control;
mod handle;
mod lock;
mod metadata;
mod named_channel;
mod pvs;
mod string;

pub use array::{ArrayChannel, ArrayChannelInfo, ArrayChannelLock, ArrayType};
pub use handle::{
    ChannelDir, ChannelHandle, ChannelSpec, InputChannel, InputDir, OutputChannel, OutputDir,
};
pub use lock::ChannelLock;
pub use metadata::{ChannelBehavior, ChannelHints, ChannelInfo};
pub use pvs::{
    PvsChannel, PvsChannelInfo, PvsChannelLock, PvsChannelParams, PvsFormat, PvsFrame,
    PvsWindowType,
};
