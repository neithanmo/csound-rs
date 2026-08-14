//! # Csound
//! This crate contains safe Csound bindings for the csound's C API.
//! The supported csound's version is 7.0. Csound 6.x is not supported.
//!
//! ## What is Csound?
//!
//! Csound is a sound and music computing system. If you want to known more visit:
//!
//! - [Csound webside](https://csound.com/index.html)
//! - [Documentation](http://www.csounds.com/resources/documentation/)
//! - [Community](https://csound.com/community.html)
//! - [Audio examples](https://csound.com/community.html)
//! - [Floss](http://write.flossmanuals.net/csound/preface/)
//!
//! # Execution Model
//!
//! Csound has a **flexible execution model** that supports multiple valid call sequences.
//! There is no strict linear state machine - the API allows different workflows depending
//! on your use case.
//!
//! ## Traditional Score-Based Mode
//!
//! Compile first, then start. Performance terminates when the score ends:
//!
//! ```no_run
//! # use csound::Csound;
//! let cs = Csound::new().unwrap();
//! cs.compile_csd("my_piece.csd", 0, 0).unwrap();  // Compile first
//! cs.start().unwrap();                             // Then start
//! while !cs.perform_ksmps() {                      // Runs until score ends
//!     // Process audio...
//! }
//! cs.reset();                                      // Reset for next performance
//! ```
//!
//! ## Real-Time Event Mode
//!
//! Start first, then compile. `<CsOptions>` is ignored, score events are dispatched
//! in real-time, and performance continues indefinitely:
//!
//! ```no_run
//! # use csound::{Csound, ScoreEventType};
//! let cs = Csound::new().unwrap();
//! cs.set_option("-odac").unwrap();                // Set options manually
//! cs.start().unwrap();                             // Start FIRST
//! cs.compile_csd("instruments.csd", 0, 0).unwrap(); // Then compile (can repeat!)
//!
//! loop {
//!     // Trigger instrument 1 at time 0, duration 1, frequency 440
//!     cs.send_score_event(ScoreEventType::Instrument, &[1.0, 0.0, 1.0, 440.0]);
//!     if cs.perform_ksmps() { break; }
//!     // Break when done (performance doesn't auto-terminate)
//! }
//! ```
//!
//! ## Key Points
//!
//! - **[`Csound::start`] can be called before or after [`Csound::compile_csd`]** - the order
//!   determines the execution mode
//! - **[`Csound::compile_csd`] and [`Csound::compile_orc`] can be called repeatedly during
//!   performance** to add new instruments and events dynamically
//! - **[`Csound::reset`] returns to the initial state**, allowing successive performances
//!   without recreating the Csound instance
//! - **[`Csound::perform_ksmps`] requires [`Csound::start`] to have been called first**
//!
//! ## Thread Safety
//!
//! Csound uses internal locking for thread-safe operations:
//!
//! - **Control channels** ([`Csound::get_control_channel`], [`Csound::set_control_channel`]):
//!   Use atomic operations, safe to call from any thread
//! - **Audio/String channels** ([`Csound::read_audio_channel`], [`Csound::get_string_channel`]):
//!   Use spinlocks internally, safe to call from any thread
//! - **Score events** ([`Csound::send_score_event`], [`Csound::send_string_event`]):
//!   Protected by API mutex, safe to call from any thread
//!
//! **Note**: Direct buffer access via [`Csound::get_spin`], [`Csound::get_spout`], and
//! [`Csound::get_table`] returns raw pointers to Csound's internal buffers. These are
//! **not thread-safe** - the caller must ensure proper synchronization when accessing
//! these buffers concurrently with [`Csound::perform_ksmps`].
//!
//! # Hello World
//!
//! A simple Hello world example which reproduces a simple sine wave signal.
//! The call to the csound's perform() method will block the application until
//! the end of the score have been reached.
//!
//! There are another alternatives for non blocking calls to perform csound's scores
//! or csd files. see the examples in the project's source directory or go to
//! [*csound's examples repository*](https://github.com/csound/csoundAPI_examples/tree/master/rust)
//! for more advanced examples and use cases.
//!
//! ```no_run
//! use csound::Csound;
//!
//! static ORC: &str = "
//! sr = 44100
//! ksmps = 32
//! nchnls = 2
//! 0dbfs  = 1
//!
//! instr 1
//!   kamp = .6
//!   kcps = 440
//!   asig oscil kamp, kcps
//!   outs asig, asig
//! endin
//! ";
//!
//! fn main() -> Result<(), csound::Error> {
//!     let cs = Csound::new()?;
//!
//!     cs.message_string_callback(|_, msg: &str| print!("{}", msg));
//!     cs.compile_orc(ORC, 0).unwrap();
//!     cs.start().unwrap();
//!
//!     // Run the performance loop
//!     while !cs.perform_ksmps() {
//!         // Process audio...
//!     }
//!     Ok(())
//! }
//! ```

pub use csound_sys::RTCLOCK;

mod callbacks;
mod channels;
mod csound;
mod enums;
mod error;
mod pvs_channel;
mod rtaudio;
mod table;

pub use crate::csound::{BufferPtr, CircularBuffer, Csound, OpcodeListEntry};
pub use callbacks::{FileInfo, PanicState, PanickedCallbacks};
pub use channels::{
    ChannelDir, ChannelHandle, ChannelHints, ChannelInfo, ChannelLock, ChannelSpec, InputChannel,
    InputDir, OutputChannel, OutputDir,
};
pub use enums::{
    AudioChannel, ChannelData, ControlChannel, FileTypes, Language, MessageType, ScoreEventType,
    Status, StrChannel,
};
pub use error::{CsoundStatus, Error, Result};
pub use pvs_channel::{
    PvsChannel, PvsChannelInfo, PvsChannelLock, PvsChannelParams, PvsFormat, PvsFrame,
    PvsWindowType,
};
pub use rtaudio::{CsAudioDevice, CsMidiDevice, RtAudioParams};
pub use table::{Table, TableId};

/// Csound sample type (MYFLT) as defined by the linked Csound build.
pub type Myflt = csound_sys::MYFLT;

// Re-export tracing for users who want to configure logging
pub use tracing;
