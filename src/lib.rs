//*    /* enable the csound's message callback and  set a closure to be called
//*    whenever a new message is available*/
#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]
extern crate libc;
#[macro_use]
extern crate bitflags;
extern crate csound_sys;
pub use csound_sys::RTCLOCK;

mod callbacks;
mod channels;
mod csound;
mod enums;
mod rtaudio;
pub use callbacks::FileInfo;
pub use channels::{PvsDataExt, ChannelHints, ChannelInfo};
pub use csound::{BufferPtr, CircularBuffer, ControlChannelPtr, Csound, OpcodeListEntry, Table};
pub use enums::{ChannelData, ControlChannelType, FileTypes, Language, MessageType, Status};
pub use rtaudio::{CsAudioDevice, CsMidiDevice, RtAudioParams};
