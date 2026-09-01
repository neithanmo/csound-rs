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
//! let mut cs = Csound::new().unwrap();             // `mut` is needed to reset
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
//!   without recreating the Csound instance. It takes `&mut self`, because it frees the
//!   engine memory that outstanding handles ([`BufferPtr`], [`PvsChannel`],
//!   [`ArrayChannel`], channel handles) point into; the borrow checker will reject a
//!   reset while any of them is alive.
//!
//! - **[`Csound::perform_ksmps`] requires [`Csound::start`] to have been called first**
//!
//! ## Handle lifetimes
//!
//! [`Csound::get_spin`] and [`Csound::get_spout`] return views over engine memory rather
//! than copies. Their allocations remain engine-owned and are invalidated by
//! [`Csound::reset`]. Callers must also sequence access with [`Csound::perform_ksmps`],
//! which reads or rewrites their contents.
//!
//! Function-table pointers are not exposed as persistent safe handles because Csound can
//! replace them during recompilation or score performance. Use [`Csound::read_table`] for
//! an owned snapshot, [`Csound::table_copy_in`] and [`Csound::table_copy_out`] for
//! synchronous copies under Csound's API lock, or [`Csound::with_table`] for scoped,
//! zero-copy access while the engine is quiescent.
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
//! **Note**: Direct buffer access via [`Csound::get_spin`] and [`Csound::get_spout`]
//! returns raw pointers to Csound's internal buffers. Scoped zero-copy table access via
//! [`Csound::with_table`] also uses a non-thread-safe Csound pointer internally. The caller
//! must ensure that these buffers are not accessed concurrently with
//! [`Csound::perform_ksmps`] or a separate performance thread.
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
mod debugger;
mod enums;
mod error;
mod ffi_adapter;
mod params;
mod rtaudio;

pub use crate::csound::{BufferPtr, CircularBuffer, Csound, OpcodeListEntry};
pub use callbacks::{FileInfo, PanicState, PanickedCallbacks};
pub use channels::{
    ArrayChannel, ArrayChannelInfo, ArrayChannelLock, ArrayType, ChannelDir, ChannelHandle,
    ChannelHints, ChannelInfo, ChannelLock, ChannelSpec, InputChannel, InputDir, OutputChannel,
    OutputDir, PvsChannel, PvsChannelInfo, PvsChannelLock, PvsChannelParams, PvsFormat, PvsFrame,
    PvsWindowType,
};
pub use debugger::{
    ArrayInfo, BreakpointInfo, DebugVariable, Debugger, FsigInfo, InstrInstance, InstrInstances,
    UdoFrame, UdoFrames, Variables,
};
pub use enums::{
    AudioChannel, ChannelData, ControlChannel, ControlChannelType, FileTypes, Language,
    MessageType, ScoreEventType, Status, StrChannel,
};
pub use error::{CsoundStatus, Error, Result};
pub use params::CsoundParams;
pub use rtaudio::{CsAudioDevice, CsMidiDevice, RtAudioParams};

/// Csound sample type (MYFLT) as defined by the linked Csound build.
pub type Myflt = csound_sys::MYFLT;

/// csound table identifier
pub type TableId = u32;

// Re-export tracing for users who want to configure logging
pub use tracing;
