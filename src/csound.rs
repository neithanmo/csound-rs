use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};
use std::slice;
use std::sync::OnceLock;

use crate::channels::{
    ChannelBehavior, ChannelDir, ChannelHandle, ChannelHints, ChannelInfo, ChannelSpec,
    InputChannel, InputDir, OutputChannel, OutputDir,
};
use crate::enums::{
    ChannelData, ControlChannelType, Language, MessageType, ScoreEventType, Status,
};
use crate::error::{Error, Result};
use crate::rtaudio::{CsAudioDevice, CsMidiDevice, RtAudioParams};
use crate::{Myflt, TableId, callbacks::*};

use csound_sys::{CSOUND_STATUS, RTCLOCK, controlChannelType};

use std::ffi::{CStr, CString};
use std::str;

use libc::{c_char, c_int, c_long, c_void};

/// Struct with information about a csound opcode.
///
/// Used to get the complete csound opcodes list, so the
/// [`Csound::get_opcode_list_entry`](struct.Csound.html#method.get_opcode_list_entry) method will return
/// a list of OpcodeListEntry, where each of this struct contain information relative
/// a specific csound opcode.
#[derive(Debug, Clone)]
pub struct OpcodeListEntry {
    /// The opcode name (always present).
    pub opname: String,
    /// The opcode output type signature (e.g., "a" for audio, "k" for control).
    /// None if the opcode produces no output.
    pub outypes: String,
    /// The opcode input type signature.
    /// None if the opcode takes no input.
    pub intypes: String,
    /// Opcode flags.
    pub flags: i32,
}

#[derive(Default)]
pub(crate) struct CallbackHandler<'c> {
    pub callbacks: Callbacks<'c>,
    pub panic_state: PanicState,
}

/// Opaque struct representing an csound object
///
/// This is the main struct used to access the libcsound API functions.
/// The Engine element is the inner representation of the CSOUND opaque pointer and is
/// the object wich talk directly with the libcsound c library.
///
#[derive(Debug)]
pub struct Csound {
    /// Inner representation of the CSOUND opaque pointer
    pub(crate) engine: Inner,
}

/// Opaque struct representing a csound object
#[derive(Debug)]
pub(crate) struct Inner {
    /// Pointer to the CSOUND instance (guaranteed non-null after construction)
    pub(crate) csound: NonNull<csound_sys::CSOUND>,
    /// Pointer to the callback handler (owned, freed on drop)
    host_data: NonNull<CallbackHandler<'static>>,
}

/// Global initialization guard - csound is initialized exactly once
static CSOUND_INIT: OnceLock<i32> = OnceLock::new();

// SAFETY: The CSOUND pointer can be safely sent between threads when:
// 1. Access is externally synchronized (e.g., via Mutex), OR
// 2. Only thread-safe Csound APIs are used (channels, message buffer)
// The CallbackHandler contains only function pointers which are Send.
unsafe impl Send for Inner {}

impl Csound {
    /// Create a new csound object.
    ///
    /// This is the core of almost all operations in the csound library.
    /// A new instance of csound will be created by this function with a custom callback handler.
    /// The callback handler will be active only if the user calls one of the callback setting
    /// functions which receive a closure for a specific callback.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The csound library fails to initialize
    /// - Memory allocation for the csound instance fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// use csound::{Csound, MessageType};
    ///
    /// // Creates a Csound instance with a custom callback handler
    /// let csound = Csound::new().expect("Failed to create Csound instance");
    ///
    /// // Enable the message callback by passing a closure
    /// csound.message_string_callback(|mtype: MessageType, message: &str| {
    ///     println!("message type: {:?} message content: {}", mtype, message);
    /// });
    /// ```
    pub fn new() -> Result<Self> {
        // Initialize csound library exactly once (thread-safe)
        let flags =
            (csound_sys::CSOUNDINIT_NO_SIGNAL_HANDLER | csound_sys::CSOUNDINIT_NO_ATEXIT) as c_int;
        let status = *CSOUND_INIT.get_or_init(|| unsafe { csound_sys::csoundInitialize(flags) });
        match Status::from(status) {
            Status::Success => {}
            Status::Signal => return Err(Error::Signal),
            Status::Memory => return Err(Error::Memory),
            Status::Performance => return Err(Error::Performance),
            Status::Initialization => return Err(Error::Initialization),
            _ => return Err(Error::InitFailed),
        }

        // Ensure MYFLT size matches the linked Csound library.
        let expected = std::mem::size_of::<crate::Myflt>();
        let actual = unsafe { csound_sys::csoundGetSizeOfMYFLT() as usize };
        if expected != actual {
            return Err(Error::MyfltMismatch { expected, actual });
        }

        // Create the callback handler
        let callback_handler = Box::new(CallbackHandler {
            callbacks: Callbacks::default(),
            panic_state: PanicState::new(),
        });
        let host_data = NonNull::new(Box::into_raw(callback_handler))
            .ok_or(Error::NullPointer("callback handler allocation"))?;

        // Create the csound instance
        let csound_ptr =
            unsafe { csound_sys::csoundCreate(host_data.as_ptr() as *mut c_void, ptr::null()) };

        let csound = NonNull::new(csound_ptr).ok_or_else(|| {
            // Clean up the callback handler if csound creation failed
            unsafe {
                drop(Box::from_raw(host_data.as_ptr()));
            }
            Error::NullPointer("csound instance creation")
        })?;

        let instance = Csound {
            engine: Inner { csound, host_data },
        };

        #[cfg(feature = "cs-message-tracing")]
        instance.enable_message_tracing();

        Ok(instance)
    }

    /// Installs a message callback that routes Csound messages through `tracing`.
    ///
    /// This maps [`MessageType`] to tracing levels:
    /// - `Error` → `tracing::error!`
    /// - `Warning` → `tracing::warn!`
    /// - `Default`, `Orch`, `Realtime`, `Stdout` → `tracing::trace!`
    ///
    /// Enabled automatically when the `cs-message-tracing` feature is active.
    /// Calling [`message_string_callback`](Self::message_string_callback) afterwards
    /// replaces this with the user-provided callback.
    #[cfg(feature = "cs-message-tracing")]
    pub fn enable_message_tracing(&self) {
        self.message_string_callback(|msg_type: MessageType, message: &str| {
            let msg = message.trim_end();
            if msg.is_empty() {
                return;
            }
            match msg_type {
                MessageType::Error => tracing::error!(target: "csound", "{msg}"),
                MessageType::Warning => tracing::warn!(target: "csound", "{msg}"),
                _ => tracing::trace!(target: "csound", "{msg}"),
            }
        });
    }

    /// Initializes the csound library with specific flags.
    ///
    /// This function is called internally by [`Csound::new()`], so there is generally no need
    /// to use it explicitly unless you need to avoid default initialization that sets signal
    /// handlers and atexit() callbacks.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub fn initialize(flags: i32) -> Result<()> {
        unsafe {
            match csound_sys::csoundInitialize(flags as c_int) {
                CSOUND_STATUS::CSOUND_ERROR => {
                    tracing::error!(flags, "failed to initialize csound");
                    Err(Error::InitFailed)
                }
                CSOUND_STATUS::CSOUND_SUCCESS => Ok(()),
                _ => Ok(()), // Already initialized is not an error
            }
        }
    }

    /// Sets a single csound option (flag).
    ///
    /// NB: blank spaces are not allowed.
    ///
    /// # Errors
    ///
    /// Returns an error if the option string contains a NUL byte or if csound rejects the option.
    pub fn set_option(&self, option: &str) -> Result<()> {
        let op = CString::new(option)?;
        unsafe {
            match csound_sys::csoundSetOption(self.csound_ptr(), op.as_ptr()) {
                CSOUND_STATUS::CSOUND_SUCCESS => Ok(()),
                _ => {
                    tracing::error!(option, "invalid csound option");
                    Err(Error::InvalidOption(option.to_string()))
                }
            }
        }
    }

    /// Returns the raw csound pointer for FFI calls.
    #[inline]
    pub(crate) fn csound_ptr(&self) -> *mut csound_sys::CSOUND {
        self.engine.csound.as_ptr()
    }

    /// Prepares Csound for performance.
    ///
    /// Normally called after compiling a csd file or an orc file, in which case score preprocessing is performed and
    /// performance terminates when the score terminates.
    /// However, if called before compiling a csd file or an orc file,
    /// score preprocessing is not performed and "i" statements are dispatched as real-time events,
    /// the <CsOptions> tag is ignored, and performance continues indefinitely or until ended using the API.
    /// # Example
    ///
    /// ```ignore
    /// use csound::Csound;
    ///
    /// # let csd_filename = "file.csd";
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd(csd_filename, 0, 0).unwrap();
    /// csound.start();
    /// // ...
    /// ```
    ///
    pub fn start(&self) -> Result<()> {
        let status = unsafe { csound_sys::csoundStart(self.csound_ptr()) };
        match Status::from(status) {
            Status::Success => Ok(()),
            Status::Signal => Err(Error::Signal),
            Status::Memory => Err(Error::Memory),
            Status::Performance => Err(Error::Performance),
            Status::Initialization => Err(Error::Initialization),
            Status::Error => Err(Error::AlreadyStarted),
            Status::Ok(x) => {
                tracing::error!("csoundStart failed with code {x}");
                Err(Error::OperationFailed)
            }
        }
    }

    /// Returns the version number times 1000
    /// for example, if the current csound version is 6.12.0
    /// this function will return 6120.
    pub fn version(&self) -> u32 {
        unsafe { csound_sys::csoundGetVersion() as u32 }
    }

    /// Returns the API version number times 100
    pub fn api_version(&self) -> u32 {
        unsafe { csound_sys::csoundGetVersion() as u32 }
    }

    /* Engine performance functions implementations ********************************************************* */

    /// Resets all internal memory and state in preparation for a new performance.
    /// Enables external software to run successive Csound performances without reloading Csound.
    ///
    /// # Why this takes `&mut self`
    ///
    /// Resetting frees the engine memory that outstanding [`BufferPtr`],
    /// [`crate::PvsChannel`], [`crate::ArrayChannel`], and channel handles point
    /// into. Those handles borrow the Csound instance, so taking `&mut self`
    /// makes the compiler reject a reset while any of them is still in use.
    /// Function-table access is either copied into owned storage or scoped by
    /// [`Csound::with_table`], so a table view cannot remain alive across a
    /// reset.
    pub fn reset(&mut self) {
        unsafe {
            csound_sys::csoundReset(self.csound_ptr());
        }
    }

    /// Compiles Csound input files (such as an orchestra and score, or CSD) as directed by the supplied command-line arguments , but does not perform them.
    /// This function cannot be called during performance, and before a repeated call, csoundReset() needs to be called.
    /// # Arguments
    /// * `args` A slice containing the arguments  to be passed to csound
    /// # Returns
    /// A error message in case of failure
    pub fn compile<T>(&self, args: &[T]) -> Result<()>
    where
        T: AsRef<str>,
    {
        if args.is_empty() {
            tracing::error!("compile requires at least one argument");
            return Err(Error::InvalidArgument(
                "compile requires at least one argument",
            ));
        }

        let arguments: Vec<CString> = args
            .iter()
            .map(|arg| CString::new(arg.as_ref()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut args_raw: Vec<*const c_char> = arguments.iter().map(|arg| arg.as_ptr()).collect();
        let argv: *mut *const c_char = args_raw.as_mut_ptr();
        unsafe {
            match csound_sys::csoundCompile(self.csound_ptr(), args_raw.len() as c_int, argv) {
                CSOUND_STATUS::CSOUND_SUCCESS => Ok(()),
                _ => {
                    tracing::error!("failed to compile csound arguments");
                    Err(Error::CompileFailed("failed to compile csound arguments"))
                }
            }
        }
    }

    /// Compiles a Csound input file (CSD, .csd file or text), but does not perform it.
    /// If [`Csound::start`](struct.Csound.html#method.start) is called before `compile_csd`, the <CsOptions> element is ignored
    /// (but set_option can be called any number of times),
    /// the <CsScore> element is not pre-processed, but dispatched as real-time events;
    /// and performance continues indefinitely, or until ended by calling [`Csound::stop`](struct.Csound.html#method.stop) or some other logic.
    /// In this "real-time" mode, the sequence of calls should be:
    /// Mode = 0 is file, Mode = 1 is text
    /// async_ = 1 is async
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let csound  = Csound::new().unwrap();
    /// csound.set_option("-n");
    /// csound.set_option("-another_option");
    /// csound.start();
    /// # let csd_filename = "file.csd";
    /// csound.compile_csd(csd_filename, 0, 0);
    /// let pfields = [1.0, 0.0, 5.0, 4.5, 6.2];
    /// loop {
    ///     // Send realtime events
    ///     csound.send_sound_event(0, &pfields, 0);
    ///     //...
    ///     // some logic to break the loop after a performance of realtime events
    /// }
    /// ```
    /// *Note*: this function can be called repeatedly during performance to replace or add new instruments and events.
    /// But if csoundCompileCsd is called before csoundStart, the <CsOptions> element is used,the <CsScore> section is pre-processed and dispatched normally,
    /// and performance terminates when the score terminates, or [`Csound::stop`](struct.Csound.html#method.stop)  is called.
    /// In this "non-real-time" mode (which can still output real-time audio and handle real-time events), the sequence of calls should be:
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let csound  = Csound::new().unwrap();
    /// # let csd_filename = "file.csd";
    /// csound.compile_csd(csd_filename, 0, 0);
    /// csound.start();
    /// while !csound.perform_ksmps() {
    /// }
    /// ```
    /// # Arguments
    /// * `csd` A reference to .csd file name
    pub fn compile_csd<T>(&self, csd: T, mode: i32, async_: i32) -> Result<()>
    where
        T: AsRef<str>,
    {
        let csd_ref = csd.as_ref();
        if csd_ref.is_empty() {
            tracing::error!("empty csd string provided");
            return Err(Error::EmptyString);
        }
        let path = CString::new(csd_ref)?;
        unsafe {
            match csound_sys::csoundCompileCSD(self.csound_ptr(), path.as_ptr(), mode, async_) {
                CSOUND_STATUS::CSOUND_SUCCESS => Ok(()),
                _ => {
                    tracing::error!(csd = csd_ref, "failed to compile csd");
                    Err(Error::CompileFailed("failed to compile csd file"))
                }
            }
        }
    }

    /// Parses and compiles the given orchestra from an ASCII string, also evaluating any global space code (i-time only)
    /// this can be called during performance to compile a new orchestra.
    /// ```
    /// use csound::Csound;
    ///
    /// let csound  = Csound::new().unwrap();
    /// let orc_code = "instr 1
    ///                 a1 rand 0dbfs/4
    ///                 out a1
    ///                 endin";
    /// csound.compile_orc(orc_code, 0);
    /// ```
    /// # Arguments
    /// * `orcPath` A reference to orchestra strings
    pub fn compile_orc<T>(&self, orc: T, async_: i32) -> Result<()>
    where
        T: AsRef<str>,
    {
        let orc_ref = orc.as_ref();
        if orc_ref.is_empty() {
            tracing::error!("empty orchestra string provided");
            return Err(Error::EmptyString);
        }
        let code = CString::new(orc_ref)?;
        unsafe {
            match csound_sys::csoundCompileOrc(self.csound_ptr(), code.as_ptr(), async_) {
                CSOUND_STATUS::CSOUND_SUCCESS => Ok(()),
                _ => {
                    tracing::error!("failed to compile orchestra");
                    Err(Error::CompileFailed("failed to compile orchestra"))
                }
            }
        }
    }

    ///   Parse and compile an orchestra given on a string,
    ///   evaluating any global space code (i-time only).
    /// # Returns
    ///   On SUCCESS it returns a value passed to the
    ///   'return' opcode in global space.
    ///       code = "i1 = 2 + 2 \n return i1 \n"
    ///       retval = csound.eval_code(code)
    pub fn eval_code<T>(&self, code: T) -> Result<Myflt>
    where
        T: AsRef<str>,
    {
        let code_ref = code.as_ref();
        if code_ref.is_empty() {
            return Err(Error::EmptyString);
        }
        let cd = CString::new(code_ref)?;
        unsafe {
            Ok(csound_sys::csoundEvalCode(
                self.csound_ptr(),
                cd.as_ptr() as _,
            ))
        }
    }

    // TODO Implement csoundCompileTree functions

    /// Senses input events, and performs one control sample worth ```ksmps * number of channels * size_of::<Myflt> bytes``` of audio output.
    ///
    /// Note that some csd file, text or score have to be compiled first and then [`Csound::start`](struct.Csound.html#method.start).
    /// Enables external software to control the execution of Csound, and to synchronize
    /// performance with audio input and output(see: [`Csound::read_spin_buffer`](struct.Csound.html#method.read_spin_buffer), [`Csound::read_spout_buffer`](struct.Csound.html#method.read_spout_buffer))
    /// # Returns
    /// *false* during performance, and true when performance has finished. If called until it returns *true*, will perform an entire score.
    pub fn perform_ksmps(&self) -> bool {
        unsafe { csound_sys::csoundPerformKsmps(self.csound_ptr()) != 0 }
    }

    /*********************************** UDP ****************************************************/

    /// Starts the UDP server
    ///
    /// # Arguments
    /// * `port` The server port number.
    ///
    /// # Errors
    /// Returns [`Error::OperationFailed`] if the server could not be started.
    pub fn udp_server_start(&self, port: u32) -> Result<()> {
        let status = unsafe { csound_sys::csoundUDPServerStart(self.csound_ptr(), port) };
        match Status::from(status as i32) {
            Status::Success => Ok(()),
            _ => Err(Error::OperationFailed),
        }
    }

    /// # Returns
    /// The port number on which the server is running, or None if the server has not been started.
    pub fn udp_server_status(&self) -> Option<u32> {
        unsafe {
            let status = csound_sys::csoundUDPServerStatus(self.csound_ptr());
            if status == CSOUND_STATUS::CSOUND_ERROR {
                None
            } else {
                Some(status as u32)
            }
        }
    }

    /// Closes the UDP server
    ///
    /// # Errors
    /// Returns [`Error::OperationFailed`] if the server could not be closed.
    pub fn udp_server_close(&self) -> Result<()> {
        let status = unsafe { csound_sys::csoundUDPServerClose(self.csound_ptr()) };
        match Status::from(status as i32) {
            Status::Success => Ok(()),
            _ => Err(Error::OperationFailed),
        }
    }

    /// Turns on the transmission of console messages
    ///
    /// # Arguments
    /// * `addr` The UDP server destination address.
    /// * `port` The UDP server port number.
    /// * `mirror` If it is true, the messages will continue to be sent to the usual destination
    ///   (see [`Csound::message_string_callback`](struct.Csound.html#method.message_string_callback) ) as well as to UDP.
    ///
    /// # Errors
    /// - [`Error::Nul`] if the address contains an interior NUL byte
    /// - [`Error::OperationFailed`] if the UDP transmission could not be set up
    pub fn udp_console(&self, addr: &str, port: u32, mirror: bool) -> Result<()> {
        let ip = CString::new(addr)?;
        let status = unsafe {
            csound_sys::csoundUDPConsole(
                self.csound_ptr(),
                ip.as_ptr(),
                port as c_int,
                mirror as c_int,
            )
        };
        if status == CSOUND_STATUS::CSOUND_SUCCESS {
            Ok(())
        } else {
            Err(Error::OperationFailed)
        }
    }

    /// Stop transmitting console messages via UDP
    pub fn udp_stop_console(&self) {
        unsafe {
            csound_sys::csoundStopUDPConsole(self.csound_ptr());
        }
    }
    /* Engine Attributes functions implmentations ********************************************************* */

    /// # Returns
    /// The number of audio sample frames per second.
    pub fn get_sample_rate(&self) -> Myflt {
        unsafe { csound_sys::csoundGetSr(self.csound_ptr()) as Myflt }
    }

    /// # Returns
    /// The number of control samples per second.
    pub fn get_control_rate(&self) -> Myflt {
        unsafe { csound_sys::csoundGetKr(self.csound_ptr()) as Myflt }
    }

    /// # Returns
    /// The number of audio sample frames per control sample.
    pub fn get_ksmps(&self) -> u32 {
        unsafe { csound_sys::csoundGetKsmps(self.csound_ptr()) }
    }

    /// # Returns
    /// The current control-cycle count, in control frames, since the
    /// performance started.
    ///
    /// Useful for scheduling host-side work against Csound's own clock rather
    /// than wall time. Reset by [`Csound::reset`].
    pub fn get_kcounter(&self) -> u64 {
        unsafe { csound_sys::csoundGetKcounter(self.csound_ptr()) }
    }

    /// # Returns
    /// The total error count of the current performance.
    pub fn error_count(&self) -> i32 {
        unsafe { csound_sys::csoundErrCnt(self.csound_ptr()) }
    }

    /// Returns the system hardware sample rate the engine has recorded.
    ///
    /// # Arguments
    /// * `value` - if greater than 0, stores this as the system hardware sample
    ///   rate before returning. Pass `0.0` to query without modifying it.
    ///
    /// # Returns
    /// The stored system hardware sample rate.
    pub fn system_sr(&self, value: Myflt) -> Myflt {
        unsafe { csound_sys::csoundSystemSr(self.csound_ptr(), value) }
    }

    /// Returns the currently configured output name.
    ///
    /// This is the resolved output target, such as a soundfile path or `dac`.
    ///
    /// # Returns
    /// `None` if no output name is set, or if it is not valid UTF-8.
    pub fn get_output_name(&self) -> Option<String> {
        unsafe { Trampoline::ptr_to_string(csound_sys::csoundGetOutputName(self.csound_ptr())) }
            .ok()
    }

    /// Returns the currently configured input name.
    ///
    /// # Returns
    /// `None` if no input name is set, or if it is not valid UTF-8.
    pub fn get_input_name(&self) -> Option<String> {
        unsafe { Trampoline::ptr_to_string(csound_sys::csoundGetInputName(self.csound_ptr())) }.ok()
    }

    /// Looks up the instrument number for a named instrument.
    ///
    /// Score events carry numeric instrument identifiers, so a named
    /// instrument must be resolved before it can be triggered through
    /// [`Csound::send_score_event`].
    ///
    /// # Errors
    /// - [`Error::EmptyString`] if `name` is empty
    /// - [`Error::Nul`] if `name` contains an interior NUL byte
    /// - [`Error::NotFound`] if no instrument with that name exists
    pub fn get_instrument_number(&self, name: &str) -> Result<i32> {
        if name.is_empty() {
            return Err(Error::EmptyString);
        }
        let cname = CString::new(name)?;
        let number =
            unsafe { csound_sys::csoundGetInstrNumber(self.csound_ptr(), cname.as_ptr()) as i32 };
        if number < 0 {
            tracing::error!(instrument = name, "named instrument not found");
            return Err(Error::NotFound("named instrument not found"));
        }
        Ok(number)
    }

    /// # Returns
    /// The number of audio output channels. Set through the nchnls header variable in the csd file.
    /// is_input can be 1 or 0
    pub fn get_channels(&self, is_input: i32) -> u32 {
        unsafe { csound_sys::csoundGetChannels(self.csound_ptr(), is_input) }
    }

    /// # Returns
    /// The 0dBFS level of the spin/spout buffers.
    pub fn get_0d_bfs(&self) -> Myflt {
        unsafe { csound_sys::csoundGet0dBFS(self.csound_ptr()) as Myflt }
    }

    /// # Returns
    /// The A4 frequency reference
    pub fn get_freq(&self) -> Myflt {
        unsafe { csound_sys::csoundGetA4(self.csound_ptr()) as Myflt }
    }

    /// #Returns
    /// The current performance time in samples
    pub fn get_current_sample_time(&self) -> usize {
        unsafe { csound_sys::csoundGetCurrentTimeSamples(self.csound_ptr()) as usize }
    }

    /// # Returns
    /// The size of MYFLT in bytes.
    pub fn get_size_myflt(&self) -> u32 {
        unsafe { csound_sys::csoundGetSizeOfMYFLT() as u32 }
    }

    /// # Returns
    /// Whether Csound is set to print debug messages.
    /// sents through the *DebugMsg()* csouns's internal API function.
    /// Anything different to 0 means true.
    pub fn get_debug_level(&self) -> u32 {
        unsafe { csound_sys::csoundGetDebug(self.csound_ptr()) as u32 }
    }

    /// Sets whether Csound prints debug messages from the *DebugMsg()* csouns's internal API function.
    /// # Arguments
    /// * `level` The debug level to assign, anything different to 0 means true.
    pub fn set_debug_level(&self, level: i32) {
        unsafe {
            csound_sys::csoundSetDebug(self.csound_ptr(), level as c_int);
        }
    }

    /* Engine general Realtime Audio I/O functions implmentations ********************************************************* */

    /// Generic helper for enumerating devices (audio or MIDI).
    ///
    /// This reduces code duplication between get_audio_devices and get_midi_devices.
    ///
    /// # Type Parameters
    /// * `RawDevice` - The raw C device struct type (CS_AUDIODEVICE or CS_MIDIDEVICE)
    /// * `RustDevice` - The Rust wrapper type (CsAudioDevice or CsMidiDevice)
    ///
    /// # Arguments
    /// * `get_list` - Function pointer to the C enumeration function
    /// * `convert` - Closure that converts a raw C device to a Rust device
    fn enumerate_devices<RawDevice, RustDevice, F>(
        &self,
        get_list: unsafe extern "C" fn(*mut csound_sys::CSOUND, *mut RawDevice, c_int) -> c_int,
        mut convert: F,
    ) -> Result<(Vec<RustDevice>, Vec<RustDevice>), Error>
    where
        RawDevice: Default + Clone,
        F: FnMut(&RawDevice, u32) -> Result<RustDevice, Error>,
    {
        let mut input_devices = Vec::new();
        let mut output_devices = Vec::new();

        unsafe {
            // Query counts for input and output devices
            let num_of_idevices = get_list(self.csound_ptr(), ptr::null_mut(), 0);
            let num_of_odevices = get_list(self.csound_ptr(), ptr::null_mut(), 1);

            // Check for errors (negative return values indicate failure)
            // Note: 0 is valid and means no devices are available
            if num_of_idevices < 0 || num_of_odevices < 0 {
                return Ok((vec![], vec![]));
            }

            // Allocate buffers
            let mut in_vec = vec![RawDevice::default(); num_of_idevices as usize];
            let mut out_vec = vec![RawDevice::default(); num_of_odevices as usize];

            // Fill buffers
            get_list(self.csound_ptr(), in_vec.as_mut_ptr(), 0);
            get_list(self.csound_ptr(), out_vec.as_mut_ptr(), 1);

            // Convert input devices
            for dev in &in_vec {
                input_devices.push(convert(dev, 0)?); // 0 = input device
            }

            // Convert output devices
            for dev in &out_vec {
                output_devices.push(convert(dev, 1)?); // 1 = output device
            }
        }

        Ok((input_devices, output_devices))
    }

    /// Sets the current RT audio module
    pub fn set_rt_audio_module(&self, name: &str) -> Result<()> {
        let dev_name = CString::new(name)?;
        unsafe {
            csound_sys::csoundSetRTAudioModule(self.csound_ptr(), dev_name.as_ptr());
        }
        Ok(())
    }

    /// Enables external software to write audio into Csound before calling perform_ksmps.
    /// # Returns
    /// An Option containing either the [`BufferPtr`](struct.BufferPtr.html) or None if the
    /// csound's spin buffer has not been initialized. The returned *BufferPtr* is Writable.
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd("some_file_path", 0, 0);
    /// csound.start();
    /// let spin = csound.get_spin();
    /// while !csound.perform_ksmps() {
    ///     // fills the spin buffer with audio samples that you want to pass into csound
    ///     // foo_fill_buffer(spin.as_mut_slice());
    ///     // ...
    /// }
    /// ```
    #[must_use]
    pub fn get_spin(&self) -> Option<BufferPtr<'_, Writable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpin(self.csound_ptr()) as *mut Myflt;
            let len = (self.get_ksmps() * self.get_channels(1)) as usize;
            if !ptr.is_null() {
                return Some(BufferPtr {
                    ptr,
                    len,
                    phantom: PhantomData,
                });
            }
            None
        }
    }

    /// Enables external software to read audio from  Csound before calling perform_ksmps.
    /// # Returns
    /// An Option containing either the [`BufferPtr`](struct.BufferPtr.html) or None if the
    /// csound's spout buffer has not been initialized. The returned *BufferPtr* is only Readable.
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd("some_file_path", 0, 0);
    /// csound.start();
    /// let spout = csound.get_spout();
    /// while !csound.perform_ksmps() {
    ///     // Deref the spout pointer and read its content
    ///     // foo_read_buffer(&*spout);
    ///     // ...
    /// }
    /// ```
    #[must_use]
    pub fn get_spout(&self) -> Option<BufferPtr<'_, Readable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpout(self.csound_ptr()) as *mut Myflt;
            let len = (self.get_ksmps() * self.get_channels(0)) as usize;
            if !ptr.is_null() {
                return Some(BufferPtr {
                    ptr,
                    len,
                    phantom: PhantomData,
                });
            }
            None
        }
    }

    /// Enables external software to read audio from Csound after calling [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps)
    /// # Returns
    /// The number of samples copied  or an
    /// error message if the internal csound's buffer has not been initialized.
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd("some_file_path", 0, 0);
    /// csound.start();
    /// let spout_length = csound.get_ksmps() * csound.get_channels(0); // get output channels
    /// let mut spout_buffer = vec![0 as Myflt; spout_length as usize];
    /// while !csound.perform_ksmps() {
    ///     // fills your buffer with audio samples you want to pass into csound
    ///     // foo_fill_buffer(&mut spout_buffer);
    ///     csound.read_spout_buffer(&mut spout_buffer);
    ///     // ...
    /// }
    /// ```
    /// # Deprecated
    /// Use [`Csound::get_spout`](struct.Csound.html#method.get_spout) to get a [`BufferPtr`](struct.BufferPtr.html)
    /// object.
    #[deprecated(since = "0.1.5", note = "please use Csound::get_spout object instead")]
    pub fn read_spout_buffer(&self, output: &mut [Myflt]) -> Result<usize> {
        let size = self.get_ksmps() as usize * self.get_channels(0) as usize;
        let spout = unsafe { csound_sys::csoundGetSpout(self.csound_ptr()) as *mut Myflt };
        let mut len = output.len();
        if size < len {
            len = size;
        }
        if !spout.is_null() {
            unsafe {
                std::ptr::copy(spout, output.as_mut_ptr(), len);
                return Ok(len);
            }
        }
        Err(Error::BufferNotInitialized(
            "spout buffer not initialized, call compile() and start() first",
        ))
    }

    /// Enables external software to write audio into Csound before calling [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps)
    /// [`Csound::get_ksmps`](struct.Csound.html#method.get_ksmps) * [`Csound::input_channels`](struct.Csound.html#method.input_channels).
    /// # Returns
    /// The number of samples copied  or an
    /// error message if the internal csound's buffer has not been initialized.
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd("some_file_path", 0, 0);
    /// csound.start();
    /// let spin_length = csound.get_ksmps() * csound.get_channels(1); // get input channels
    /// let mut spin_buffer = vec![0 as Myflt; spin_length as usize];
    /// while !csound.perform_ksmps() {
    ///     // fills your buffer with audio samples you want to pass into csound
    ///     // foo_fill_buffer(&mut spin_buffer);
    ///     csound.write_spin_buffer(&spin_buffer);
    ///     // ...
    /// }
    /// ```
    /// # Deprecated
    /// Use [`Csound::get_spin`](struct.Csound.html#method.get_spin) to get a [`BufferPtr`](struct.BufferPtr.html)
    /// object.
    #[deprecated(since = "0.1.5", note = "please use Csound::get_spin object instead")]
    pub fn write_spin_buffer(&self, input: &[Myflt]) -> Result<usize> {
        let size = self.get_ksmps() as usize * self.get_channels(1) as usize;
        let spin = unsafe { csound_sys::csoundGetSpin(self.csound_ptr()) as *mut Myflt };
        let mut len = input.len();
        if size < len {
            len = size;
        }
        if !spin.is_null() {
            unsafe {
                std::ptr::copy(input.as_ptr(), spin, len);
                return Ok(len);
            }
        }
        Err(Error::BufferNotInitialized(
            "spin buffer not initialized, call compile() and start() first",
        ))
    }

    ///Calling this function after csoundCreate()
    /// and before the start of performance will disable all default\
    /// handling of sound I/O by the Csound library via its audio backend module.
    /// Host application should in this case use the spin/spout buffers directly.
    pub fn set_host_audio_io(&self) {
        unsafe {
            csound_sys::csoundSetHostAudioIO(self.csound_ptr());
        }
    }

    /// This function can be called to obtain a list of available input and output audio devices.
    ///
    /// # Important
    /// You must call [`set_rt_audio_module`](Self::set_rt_audio_module) before calling this function
    /// to select an RT audio backend (e.g., "portaudio", "alsa", "pulse"). Without setting a module,
    /// this function will return empty lists even if audio devices are present.
    ///
    /// # Returns
    /// A tuple containing (input_devices, output_devices):
    /// - The first element contains input audio devices
    /// - The second element contains output audio devices
    ///
    /// # Example
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.set_rt_audio_module("portaudio").unwrap();
    ///
    /// let (inputs, outputs) = cs.get_audio_devices().unwrap();
    /// println!("Found {} input and {} output devices", inputs.len(), outputs.len());
    /// ```
    pub fn get_audio_devices(&self) -> Result<(Vec<CsAudioDevice>, Vec<CsAudioDevice>), Error> {
        self.enumerate_devices(
            csound_sys::csoundGetAudioDevList,
            |dev: &csound_sys::CS_AUDIODEVICE, is_output| -> Result<CsAudioDevice, Error> {
                Ok(CsAudioDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr())?,
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr())?,
                    rt_module: Trampoline::ptr_to_string(dev.rt_module.as_ptr())?,
                    max_nchnls: dev.max_nchnls as u32,
                    is_output,
                })
            },
        )
    }

    /* Real time MIDI IO functions implementations *************************************************************** */

    /// Sets the current MIDI IO module
    pub fn set_midi_module(&self, name: &str) {
        unsafe {
            let dev_name = CString::new(name);
            if let Ok(dev) = dev_name {
                csound_sys::csoundSetMIDIModule(self.csound_ptr(), dev.as_ptr());
            }
        }
    }

    /// Calling this function after csoundCreate()
    /// and before the start of performance to implement
    /// MIDI via the callbacks below.
    pub fn set_host_midi_io(&self) {
        unsafe {
            csound_sys::csoundSetHostMIDIIO(self.csound_ptr());
        }
    }

    /// This function can be called to obtain a list of available input or output MIDI devices.
    ///
    /// # Important
    /// You should call [`set_midi_module`](Self::set_midi_module) before calling this function
    /// to select a MIDI backend (e.g., "portmidi", "alsa", "coremidi"). Without setting a module,
    /// device enumeration may not work properly.
    ///
    /// # Returns
    /// A tuple containing (input_devices, output_devices):
    /// - The first element contains input MIDI devices
    /// - The second element contains output MIDI devices
    ///
    /// # Example
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.set_midi_module("portmidi");
    ///
    /// let (inputs, outputs) = cs.get_midi_devices().unwrap();
    /// println!("Found {} input and {} output MIDI devices", inputs.len(), outputs.len());
    /// ```
    pub fn get_midi_devices(&self) -> Result<(Vec<CsMidiDevice>, Vec<CsMidiDevice>), Error> {
        self.enumerate_devices(
            csound_sys::csoundGetMIDIDevList,
            |dev: &csound_sys::CS_MIDIDEVICE, is_output| -> Result<CsMidiDevice, Error> {
                Ok(CsMidiDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr())?,
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr())?,
                    midi_module: Trampoline::ptr_to_string(dev.midi_module.as_ptr())?,
                    interface_name: Trampoline::ptr_to_string(dev.interface_name.as_ptr())?,
                    is_output,
                })
            },
        )
    }

    /* Score Handling functions implmentations ********************************************************* */

    /// Send a new event as a NULL-terminated string
    /// Multiple events separated by newlines are possible
    /// and score preprocessing (carry, etc) is applied.
    /// Optionally run asynchronously (async = 1)
    pub fn send_string_event(&self, string: &str, async_: i32) -> Result<()> {
        if string.is_empty() {
            return Err(Error::EmptyString);
        }
        let s = CString::new(string)?;
        unsafe {
            csound_sys::csoundEventString(self.csound_ptr(), s.as_ptr(), async_);
        }
        Ok(())
    }

    /// # Returns
    /// The current score time in seconds since the beginning of the performance.
    pub fn get_score_time(&self) -> f64 {
        unsafe { csound_sys::csoundGetScoreTime(self.csound_ptr()) as f64 }
    }

    /// Sets whether Csound score events are performed or not.
    /// Independently of real-time MIDI events (see [`Csound::set_score_pending`](struct.Csound.html#method.set_score_pending)).
    pub fn is_score_pending(&self) -> i32 {
        unsafe { csound_sys::csoundIsScorePending(self.csound_ptr()) as i32 }
    }

    /// Sets whether Csound score events are performed or not (real-time events will continue to be performed).
    ///  Can be used by external software, such as a VST host, to turn off performance of score events (while continuing to perform real-time events),
    ///  for example to mute a Csound score while working on other tracks of a piece, or to play the Csound instruments live.
    pub fn set_score_pending(&self, pending: i32) {
        unsafe {
            csound_sys::csoundSetScorePending(self.csound_ptr(), pending as c_int);
        }
    }

    /// Gets the current score's time.
    /// # Returns
    /// The score time beginning at which score events will actually immediately be performed
    /// (see  [`Csound::set_score_offset_seconds`](struct.Csound.html#method.set_score_offset_seconds)).
    pub fn get_score_offset_seconds(&self) -> Myflt {
        unsafe { csound_sys::csoundGetScoreOffsetSeconds(self.csound_ptr()) as Myflt }
    }

    /// Csound score events prior to the specified time are not performed.
    /// And performance begins immediately at the specified time
    /// (real-time events will continue to be performed as they are received).
    /// Can be used by external software, such as a VST host, to begin score performance midway through a Csound score,
    ///  for example to repeat a loop in a sequencer or to synchronize other events with the Csound score.
    pub fn set_score_offset_seconds(&self, offset: Myflt) {
        unsafe {
            csound_sys::csoundSetScoreOffsetSeconds(self.csound_ptr(), offset as Myflt);
        }
    }

    /// Rewinds a compiled Csound score to the time specified with [`Csound::set_score_offset_seconds`](struct.Csound.html#method.set_score_offset_seconds)
    pub fn rewind_score(&self) {
        unsafe {
            csound_sys::csoundRewindScore(self.csound_ptr());
        }
    }
    // TODO SCORE SORT FUNCTIONS

    /* Engine general messages functions implmentations ********************************************************* */

    /// # Returns
    /// The Csound message level (from 0 to 231).
    pub fn get_message_level(&self) -> u8 {
        unsafe { csound_sys::csoundGetMessageLevel(self.csound_ptr()) as u8 }
    }

    /// Sets the Csound message level (from 0 to 231).
    pub fn set_message_level(&self, level: u8) {
        unsafe {
            csound_sys::csoundSetMessageLevel(self.csound_ptr(), level as c_int);
        }
    }

    /// Creates a buffer for storing messages printed by Csound.
    ///
    /// Should be called after creating a Csound instance. The buffer is automatically
    /// freed when the Csound instance is dropped (following the proper shutdown sequence).
    ///
    /// # Arguments
    /// * `stdout` If is non-zero, the messages are also printed to stdout and stderr
    ///   (depending on the type of the message), in addition to being stored in the buffer.
    ///
    /// # Note
    /// Using the message buffer ties up the internal message callback, so
    /// [`Csound::message_string_callback`](struct.Csound.html#method.message_string_callback)
    /// should not be called after creating the message buffer.
    pub fn create_message_buffer(&mut self, stdout: i32) {
        unsafe {
            csound_sys::csoundCreateMessageBuffer(self.csound_ptr(), stdout as c_int);
        }
    }

    /// Returns whether a message buffer has been created.
    ///
    /// This uses a behavior of `csoundGetMessageCnt()` which returns -1 when
    /// no message buffer exists, and >= 0 when a buffer is allocated (even if empty).
    #[inline]
    fn has_message_buffer(&self) -> bool {
        unsafe { csound_sys::csoundGetMessageCnt(self.csound_ptr()) >= 0 }
    }

    /// # Returns
    /// The first message from the buffer.
    pub fn get_first_message(&self) -> Option<String> {
        if !self.has_message_buffer() {
            return None;
        }
        unsafe {
            let ptr = csound_sys::csoundGetFirstMessage(self.csound_ptr());
            if ptr.is_null() {
                return None;
            }
            match CStr::from_ptr(ptr).to_str() {
                Ok(m) => Some(m.to_owned()),
                _ => None,
            }
        }
    }

    /// # Returns
    /// The attribute parameter ([`MessageType`](enum.MessageType.html)) of the first message in the buffer.
    pub fn get_first_message_attr(&self) -> Option<MessageType> {
        if !self.has_message_buffer() {
            return None;
        }
        if unsafe { csound_sys::csoundGetMessageCnt(self.csound_ptr()) } <= 0 {
            return None;
        }
        Some(unsafe {
            MessageType::from(csound_sys::csoundGetFirstMessageAttr(self.csound_ptr()) as u32)
        })
    }

    /// Removes the first message from the buffer.
    pub fn pop_first_message(&self) {
        unsafe {
            csound_sys::csoundPopFirstMessage(self.csound_ptr());
        }
    }

    /// # Returns
    /// The number of pending messages in the buffer.
    pub fn get_message_count(&self) -> Option<u32> {
        if !self.has_message_buffer() {
            return None;
        }
        let count = unsafe { csound_sys::csoundGetMessageCnt(self.csound_ptr()) };
        if count < 0 { None } else { Some(count as u32) }
    }

    /* Engine general Channels, Control and Events implementations ********************************************** */

    /// Requests a list of all control channels.
    ///
    /// # Returns
    /// - `Ok(Vec<ChannelInfo>)` - A vector of channel information (may be empty if no channels exist)
    /// - `Err(Error::OperationFailed)` - Csound failed to retrieve the channel list
    /// - `Err(Error::UtfError)` - A channel name or attribute contains invalid UTF-8
    ///
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_orc("chn_k \"myChannel\", 1", 0).unwrap();
    /// cs.start().unwrap();
    ///
    /// let channels = cs.list_channels().unwrap();
    /// for channel in channels {
    ///     println!("Channel: {}, Type: {}", channel.name, channel.type_);
    /// }
    /// ```
    pub fn list_channels(&self) -> Result<Vec<ChannelInfo>> {
        let mut ptr = ptr::null_mut() as *mut csound_sys::controlChannelInfo_t;
        let ptr2: *mut *mut csound_sys::controlChannelInfo_t = &mut ptr as *mut *mut _;

        unsafe {
            let count = csound_sys::csoundListChannels(self.csound_ptr(), ptr2) as i32;

            // Negative count indicates an error
            if count < 0 {
                tracing::error!("failed to list channels");
                return Err(Error::OperationFailed);
            }

            // Zero count means no channels - return empty vec
            if count == 0 {
                return Ok(Vec::new());
            }

            // Use slice instead of manual pointer arithmetic for safety
            let channel_slice = slice::from_raw_parts(*ptr2, count as usize);
            let mut list = Vec::with_capacity(count as usize);

            for channel_info in channel_slice {
                let name = Trampoline::ptr_to_string(channel_info.name)?;
                let attributes = if channel_info.hints.attributes.is_null() {
                    None
                } else {
                    Some(Trampoline::ptr_to_string(channel_info.hints.attributes)?)
                };

                list.push(ChannelInfo {
                    name,
                    type_: channel_info.type_,
                    hints: ChannelHints {
                        behav: ChannelBehavior::from(channel_info.hints.behav),
                        dflt: channel_info.hints.dflt,
                        min: channel_info.hints.min,
                        max: channel_info.hints.max,
                        x: channel_info.hints.x,
                        y: channel_info.hints.y,
                        width: channel_info.hints.width,
                        height: channel_info.hints.height,
                        attributes,
                    },
                });
            }

            // Clean up the C-allocated list
            csound_sys::csoundDeleteChannelList(self.csound_ptr(), *ptr2);

            Ok(list)
        }
    }

    /// Returns a channel handle using a generic direction and spec.
    ///
    /// This is the shared implementation used by [`Csound::get_input_channel`] and
    /// [`Csound::get_output_channel`].
    ///
    /// # Errors
    /// - [`Error::Memory`] if memory allocation failed
    /// - [`Error::InvalidArgument`] if the name or type is invalid
    /// - [`Error::ChannelTypeMismatch`] if a channel with the same name but incompatible type exists
    /// - [`Error::NullPointer`] if the channel pointer could not be created
    pub fn get_channel<S, D>(&self, name: &str) -> Result<ChannelHandle<'_, S, D>>
    where
        S: ChannelSpec,
        D: ChannelDir,
    {
        let mut ptr: *mut c_void = ptr::null_mut();
        let ptr_ref = &mut ptr as *mut *mut c_void;
        let (len, type_bits) = match S::c_type() {
            ControlChannelType::Audio => (
                self.get_ksmps() as usize,
                controlChannelType::CSOUND_AUDIO_CHANNEL as c_int,
            ),
            ControlChannelType::Control => (1, controlChannelType::CSOUND_CONTROL_CHANNEL as c_int),
            ControlChannelType::String => {
                // Defer datasize lookup until after csoundGetChannelPtr,
                // so string channels can be created if missing.
                (0, controlChannelType::CSOUND_STRING_CHANNEL as c_int)
            }
            _ => {
                tracing::error!(
                    channel = name,
                    direction = D::NAME,
                    "unsupported channel type"
                );
                return Err(Error::InvalidArgument(
                    "unsupported channel type (only Audio, Control, and String channels are supported)",
                ));
            }
        };

        let bits = type_bits | D::FLAG;
        let cname = CString::new(name)?;
        let status = self.get_raw_channel_ptr(&cname, ptr_ref, bits);
        match Status::from(status) {
            Status::Success => {
                let len = if matches!(S::c_type(), ControlChannelType::String) {
                    self.get_channel_data_size(name)?
                } else {
                    len
                };
                let null_msg = if D::NAME == "input" {
                    "failed to create input channel"
                } else {
                    "failed to create output channel"
                };
                unsafe {
                    ChannelHandle::from_raw(self.csound_ptr(), cname, ptr as *mut S::Raw, len)
                        .ok_or(Error::NullPointer(null_msg))
                }
            }
            Status::Memory => {
                tracing::error!(
                    channel = name,
                    direction = D::NAME,
                    "memory allocation failed"
                );
                Err(Error::Memory)
            }
            Status::Error => {
                tracing::error!(
                    channel = name,
                    direction = D::NAME,
                    "invalid channel name or type"
                );
                Err(Error::InvalidArgument("invalid channel name or type"))
            }
            // Positive value indicates existing channel type mismatch
            Status::Ok(existing_type) => {
                tracing::error!(
                    channel = name,
                    direction = D::NAME,
                    existing_type,
                    "channel type mismatch"
                );
                Err(Error::ChannelTypeMismatch(existing_type))
            }
            _ => {
                tracing::error!(channel = name, direction = D::NAME, "failed to get channel");
                Err(Error::OperationFailed)
            }
        }
    }

    /// Return a [`InputChannel`](struct.InputChannel.html) which represent a csound's input channel ptr.
    /// creating the channel first if it does not exist yet.
    /// # Arguments
    /// * `name` The channel name.
    /// *
    /// The generic parameter `T` in this function can be one of the following types:
    ///  - ControlChannel
    ///    control data (one MYFLT value)
    ///  - AudioChannel
    ///    audio data (get_ksmps() MYFLT values)
    ///  - StrChannel:
    ///    string data (u8 values with enough space to store
    ///    get_channel_data_size() characters, including the
    ///    NULL character at the end of the string)
    ///
    /// If the channel already exists, it must match the data type
    /// (control, audio, or string)
    /// # Note
    ///  Audio and String channels
    /// can only be created after calling compile(), because the
    /// storage size is not known until then.
    /// # Returns
    /// A  Writable InputChannel on success or a Status code,
    ///   "Not enough memory for allocating the channel" (CS_MEMORY)
    ///   "The specified name or type is invalid" (CS_ERROR)
    /// or, if a channel with the same name but incompatible type
    /// already exists, the type of the existing channel.
    ///
    /// * Note: to find out the type of a channel without actually
    ///   creating or changing it, set 'channel_type' argument  to CSOUND_UNKNOWN_CHANNEL, so that the error
    ///   value will be either the type of the channel, or CSOUND_STATUS::CSOUND_ERROR
    ///   if it does not exist.
    ///
    /// Operations on the channel pointer are not thread-safe by default. The host is
    /// required to take care of threadsafety by
    ///   1) with control channels use __sync_fetch_and_add() or
    ///      __sync_fetch_and_or() gcc atomic builtins to get or set a channel,
    ///      if available.
    ///   2) For string and audio channels (and controls if option 1 is not
    ///      available), retrieve the channel lock with ChannelLock()
    ///      and use SpinLock() and SpinUnLock() to protect access
    ///      to the channel.
    ///
    /// See Top/threadsafe.c in the Csound library sources for
    /// examples. Optionally, use the channel get/set functions
    /// which are threadsafe by default.
    ///
    /// # Example
    /// ```text
    /// extern crate csound;
    /// use csound::{Csound, InputChannel, AudioChannel, StrChannel, ControlChannel};
    ///  // Creates a Csound instance
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd(csd_filename).unwrap();
    /// csound.start();
    /// // Request a csound's input control channel
    /// let control_channel = csound.get_input_channel::<ControlChannel>("myChannel").unwrap();
    /// // Writes some data to the channel
    /// control_channel.lock().write(0.5);
    /// // Request a csound's input audio channel
    /// let audio_channel = csound.get_input_channel::<AudioChannel>("myAudioChannel").unwrap();
    /// let mut audio_guard = audio_channel.lock();
    /// println!("audio channel samples {:?}", audio_guard.as_slice());
    /// // Request a csound's input string channel
    /// let string_channel = csound.get_input_channel::<StrChannel>("myStringChannel").unwrap();
    /// let mut string_guard = string_channel.lock();
    /// string_guard.write_str("hello").unwrap();
    ///
    /// ```
    ///
    /// # Errors
    /// - [`Error::Memory`] if memory allocation failed
    /// - [`Error::InvalidArgument`] if the name or type is invalid
    /// - [`Error::ChannelTypeMismatch`] if a channel with the same name but incompatible type exists
    /// - [`Error::NullPointer`] if the channel pointer could not be created
    pub fn get_input_channel<T>(&self, name: &str) -> Result<InputChannel<'_, T>>
    where
        T: ChannelSpec,
    {
        self.get_channel::<T, InputDir>(name)
    }

    /// Return a [`OutputChannel`](struct.OutputChannel.html) which represent a csound's output channel ptr.
    /// creating the channel first if it does not exist yet.
    /// # Arguments
    /// * `name` The channel name.
    ///
    /// The generic parameter `T` in this function can be one of the following types:
    ///  - ControlChannel
    ///    control data (one MYFLT value)
    ///  - AudioChannel
    ///    audio data (get_ksmps() MYFLT values)
    ///  - StrChannel:
    ///    string data (u8 values with enough space to store
    ///    get_channel_data_size() characters, including the
    ///    NULL character at the end of the string)
    ///
    /// If the channel already exists, it must match the data type
    /// (control, audio, or string)
    /// # Note
    ///  Audio and String channels
    /// can only be created after calling compile(), because the
    /// storage size is not known until then.
    ///
    /// Operations on the channel pointer are not thread-safe by default. The host is
    /// required to take care of threadsafety by
    ///   1) with control channels use __sync_fetch_and_add() or
    ///      __sync_fetch_and_or() gcc atomic builtins to get or set a channel,
    ///      if available.
    ///   2) For string and audio channels (and controls if option 1 is not
    ///      available), retrieve the channel lock with ChannelLock()
    ///      and use SpinLock() and SpinUnLock() to protect access
    ///      to the channel.
    ///
    /// See Top/threadsafe.c in the Csound library sources for
    /// examples. Optionally, use the channel get/set functions
    /// which are threadsafe by default.
    /// # Example
    /// ```text
    /// extern crate csound;
    /// use csound::{Csound, OutputChannel, AudioChannel, StrChannel, ControlChannel};
    ///
    ///  // Creates a Csound instance
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd(csd_filename).unwrap();
    /// csound.start();
    /// // Request a csound's output control channel
    /// let control_channel = csound.get_output_channel::<ControlChannel>("myChannel").unwrap();
    /// // Reads data from the channel
    /// println!("channel value {}", control_channel.lock().read());
    /// // Request a csound's output audio channel
    /// let audio_channel = csound.get_output_channel::<AudioChannel>("myAudioChannel").unwrap();
    /// let audio_guard = audio_channel.lock();
    /// println!("audio channel samples {:?}", audio_guard.as_slice());
    /// // Request a csound's output string channel
    /// let string_channel = csound.get_output_channel::<StrChannel>("myStringChannel").unwrap();
    /// let string_guard = string_channel.lock();
    /// println!("string value {:?}", string_guard.as_str());
    ///
    /// ```
    ///
    /// # Errors
    /// - [`Error::Memory`] if memory allocation failed
    /// - [`Error::InvalidArgument`] if the name or type is invalid
    /// - [`Error::ChannelTypeMismatch`] if a channel with the same name but incompatible type exists
    /// - [`Error::NullPointer`] if the channel pointer could not be created
    pub fn get_output_channel<T>(&self, name: &str) -> Result<OutputChannel<'_, T>>
    where
        T: ChannelSpec,
    {
        self.get_channel::<T, OutputDir>(name)
    }

    pub(crate) fn get_raw_channel_ptr(
        &self,
        cname: &CString,
        ptr: *mut *mut c_void,
        channel_type: c_int,
    ) -> c_int {
        unsafe {
            csound_sys::csoundGetChannelPtr(self.csound_ptr(), ptr, cname.as_ptr(), channel_type)
        }
    }

    /// Set parameters hints for a control channel.
    /// These hints have no internal function but can be used by front ends to construct GUIs or to constrain values.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if the channel does not exist, is not a control channel, or the parameters are invalid
    /// - [`Error::Memory`] if memory allocation failed
    /// - [`Error::Nul`] if the name or attributes contain an interior NUL byte
    pub fn set_channel_hints(&self, name: &str, hint: &ChannelHints) -> Result<()> {
        let attr = match &hint.attributes {
            Some(s) => Some(CString::new(s.as_str())?),
            None => None,
        };
        let cname = CString::new(name)?;
        let channel_hint = csound_sys::controlChannelHints_t {
            behav: ChannelBehavior::to_u32(hint.behav),
            dflt: hint.dflt,
            min: hint.min,
            max: hint.max,
            x: hint.x,
            y: hint.y,
            width: hint.width as c_int,
            height: hint.height as c_int,
            attributes: attr
                .as_ref()
                .map(|s| s.as_ptr() as *mut c_char)
                .unwrap_or(std::ptr::null_mut()),
        };
        let status = unsafe {
            csound_sys::csoundSetControlChannelHints(
                self.csound_ptr(),
                cname.as_ptr(),
                channel_hint,
            )
        };
        match Status::from(status as i32) {
            Status::Success => Ok(()),
            Status::Memory => {
                tracing::error!(
                    channel = name,
                    "memory allocation failed setting channel hints"
                );
                Err(Error::Memory)
            }
            _ => {
                tracing::error!(channel = name, "failed to set channel hints");
                Err(Error::NotFound(
                    "channel does not exist, is not a control channel, or parameters are invalid",
                ))
            }
        }
    }

    /// Returns special parameters (or None if there are not any) of a control channel.
    /// Previously set with csoundSetControlChannelHints() or the
    /// [chnparams](http://www.csounds.com/manualOLPC/chnparams.html) opcode.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if the channel does not exist or is not a control channel
    /// - [`Error::Memory`] if memory allocation failed
    /// - [`Error::Nul`] if the channel name contains an interior NUL byte
    pub fn get_channel_hints(&self, name: &str) -> Result<ChannelHints> {
        let cname = CString::new(name)?;
        let mut hint = csound_sys::controlChannelHints_t::default();
        let status = unsafe {
            csound_sys::csoundGetControlChannelHints(
                self.csound_ptr(),
                cname.as_ptr() as *mut c_char,
                &mut hint as *mut _,
            )
        };

        match Status::from(status) {
            Status::Success => {
                let attributes = if hint.attributes.is_null() {
                    None
                } else {
                    let result = Trampoline::ptr_to_string(hint.attributes);
                    // csoundGetControlChannelHints allocates attributes with csound's allocator.
                    // We free it here to avoid leaking per call.
                    unsafe { libc::free(hint.attributes as *mut c_void) };
                    Some(result?)
                };
                Ok(ChannelHints {
                    behav: ChannelBehavior::from(hint.behav),
                    dflt: hint.dflt,
                    min: hint.min,
                    max: hint.max,
                    x: hint.x,
                    y: hint.y,
                    width: hint.width,
                    height: hint.height,
                    attributes,
                })
            }
            Status::Memory => {
                tracing::error!(
                    channel = name,
                    "memory allocation failed getting channel hints"
                );
                Err(Error::Memory)
            }
            // CSOUND_ERROR or any other non-zero: channel doesn't exist or isn't a control channel
            _ => {
                tracing::error!(channel = name, "channel not found or not a control channel");
                Err(Error::NotFound(
                    "channel does not exist or is not a control channel",
                ))
            }
        }
    }

    /// Retrieves the value of a control channel.
    /// # Arguments
    /// * `name`  The channel name.
    ///   An error message will be returned if the channel is not a control channel, the channel not exist or if the name is invalid.
    pub fn get_control_channel(&self, name: &str) -> Result<Myflt> {
        let cname = CString::new(name)?;
        let mut err: c_int = 0;
        unsafe {
            let ret = csound_sys::csoundGetControlChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                &mut err as *mut _,
            ) as Myflt;
            if (err) == CSOUND_STATUS::CSOUND_SUCCESS {
                Ok(ret)
            } else {
                tracing::error!(channel = name, "control channel not found");
                Err(Error::NotFound(
                    "channel does not exist or is not a control channel",
                ))
            }
        }
    }

    /// Sets the value of a control channel.
    /// # Arguments
    /// * `name`  The channel name.
    ///
    /// This function is thread-safe and can be called concurrently with
    /// [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps).
    pub fn set_control_channel(&self, name: &str, value: Myflt) -> Result<()> {
        let cname = CString::new(name)?;
        unsafe { csound_sys::csoundSetControlChannel(self.csound_ptr(), cname.as_ptr(), value) };
        Ok(())
    }

    /// Copies samples from an audio channel.
    /// # Arguments
    /// * `name` The channel name.
    /// * `output` The slice where the data contained in the internal audio channel buffer
    ///   will be copied. Should contain enough memory for ksmps MYFLT samples.
    ///
    /// # Errors
    /// - [`Error::Nul`] if the channel name contains an interior NUL byte
    /// - [`Error::InsufficientCapacity`] if the output buffer is too small
    pub fn read_audio_channel(&self, name: &str, output: &mut [Myflt]) -> Result<()> {
        let ksmps = self.get_ksmps() as usize;
        let cname = CString::new(name)?;

        if output.len() < ksmps {
            tracing::error!(
                channel = name,
                expected = ksmps,
                actual = output.len(),
                "audio channel read buffer too small"
            );
            return Err(Error::InsufficientCapacity {
                expected: ksmps,
                actual: output.len(),
            });
        }

        unsafe {
            csound_sys::csoundGetAudioChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                output.as_mut_ptr(),
            );
        }
        Ok(())
    }

    /// Writes data into an audio channel buffer.
    /// # Arguments
    /// * `name` The channel name.
    /// * `input` The slice with data to be copied into the audio channel buffer. Must contain at least ksmps samples.
    ///   If more than ksmps samples are provided, the extra data will be ignored.
    ///
    /// # Errors
    /// - [`Error::Nul`] if the channel name contains an interior NUL byte
    /// - [`Error::InsufficientCapacity`] if the input data exceeds channel capacity
    pub fn write_audio_channel(&mut self, name: &str, input: &[Myflt]) -> Result<()> {
        let size = self.get_ksmps() as usize;
        let cname = CString::new(name)?;

        if input.len() < size {
            tracing::error!(
                channel = name,
                expected = size,
                actual = input.len(),
                "audio channel write buffer too small"
            );
            return Err(Error::InsufficientCapacity {
                expected: size,
                actual: input.len(),
            });
        }
        if input.len() > size {
            tracing::warn!(
                channel = name,
                expected = size,
                actual = input.len(),
                "audio channel write buffer larger than ksmps; extra data will be ignored"
            );
        }

        unsafe {
            csound_sys::csoundSetAudioChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                input.as_ptr() as *mut Myflt,
            );
        }
        Ok(())
    }

    /// Returns the content of the string channel identified by *name*
    ///
    /// # Errors
    /// - [`Error::Nul`] if the channel name contains an interior NUL byte
    /// - [`Error::UtfError`] if the channel contains invalid UTF-8
    pub fn get_string_channel(&self, name: &str) -> Result<String> {
        let cname = CString::new(name)?;
        let capacity = self.get_channel_data_size(name)?;
        let mut buffer = vec![0u8; capacity];

        unsafe {
            csound_sys::csoundGetStringChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                buffer.as_mut_ptr() as *mut _,
            );
        }

        // Find the null terminator to get the actual string length
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(capacity);
        buffer.truncate(len);

        String::from_utf8(buffer).map_err(|e| Error::UtfError(e.utf8_error()))
    }

    /// Sets the string channel identified by *name* with *content*
    ///
    /// # Errors
    /// - [`Error::Nul`] if the channel name or content contains an interior NUL byte
    pub fn set_string_channel(&mut self, name: &str, content: &str) -> Result<()> {
        let cname = CString::new(name)?;
        let content = CString::new(content)?;
        unsafe {
            csound_sys::csoundSetStringChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                content.as_ptr() as *mut _,
            );
        }
        Ok(())
    }

    /// Returns the size of data stored in the channel identified by *name*
    ///
    /// # Errors
    /// - [`Error::Nul`] if the channel name contains an interior NUL byte
    pub fn get_channel_data_size(&self, name: &str) -> Result<usize> {
        let cname = CString::new(name)?;
        let size =
            unsafe { csound_sys::csoundGetChannelDatasize(self.csound_ptr(), cname.as_ptr()) };
        if size <= 0 {
            tracing::error!(channel = name, "channel not found or has invalid data size");
            return Err(Error::NotFound("channel does not exist"));
        }
        Ok(size as usize)
    }

    /// Sends a score event to Csound synchronously.
    ///
    /// The event is processed immediately in the current thread, blocking until queued.
    ///
    /// # Arguments
    /// * `event_type` - The type of score event to send.
    /// * `pfields` - A slice of Myflt values containing the p-fields for this event.
    ///   For instrument events, this typically includes instrument number, start time, duration,
    ///   and any additional parameters.
    ///
    /// # Example
    /// ```ignore
    /// use csound::{Csound, ScoreEventType};
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_orc(orc, 0).unwrap();
    /// cs.start().unwrap();
    ///
    /// // Trigger instrument 1 at time 0 for 1 second
    /// let pfields = [1.0, 0.0, 1.0];
    /// cs.send_score_event(ScoreEventType::Instrument, &pfields);
    ///
    /// while !cs.perform_ksmps() {
    ///     // Performance loop
    /// }
    /// ```
    pub fn send_score_event(&self, event_type: ScoreEventType, pfields: &[Myflt]) {
        unsafe {
            csound_sys::csoundEvent(
                self.csound_ptr(),
                event_type.as_i32(),
                pfields.as_ptr() as *mut Myflt,
                pfields.len() as c_int,
                0,
            );
        }
    }

    /// Sends a score event to Csound asynchronously.
    ///
    /// The event is queued and processed by the performance thread, returning immediately.
    /// This is useful when sending events from a different thread than the performance thread.
    ///
    /// # Arguments
    /// * `event_type` - The type of score event to send.
    /// * `pfields` - A slice of Myflt values containing the p-fields for this event.
    ///   For instrument events, this typically includes instrument number, start time, duration,
    ///   and any additional parameters.
    ///
    /// # Example
    /// ```ignore
    /// use csound::{Csound, ScoreEventType};
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_orc(orc, 0).unwrap();
    /// cs.start().unwrap();
    ///
    /// // Trigger instrument 1 at time 0 for 1 second (async)
    /// let pfields = [1.0, 0.0, 1.0];
    /// cs.send_score_event_async(ScoreEventType::Instrument, &pfields);
    /// ```
    pub fn send_score_event_async(&self, event_type: ScoreEventType, pfields: &[Myflt]) {
        unsafe {
            csound_sys::csoundEvent(
                self.csound_ptr(),
                event_type.as_i32(),
                pfields.as_ptr() as *mut Myflt,
                pfields.len() as c_int,
                1,
            );
        }
    }

    /// Sends a score event to Csound (deprecated).
    ///
    /// # Arguments
    /// * `event_type` - The event type as raw i32 (0=instrument, 1=table, 2=end).
    /// * `pfields` - A slice of Myflt values with all the pfields for this event.
    /// * `async_` - If non-zero, the event is processed asynchronously.
    #[deprecated(
        since = "0.2.0",
        note = "Use `send_score_event` or `send_score_event_async` with `ScoreEventType` instead"
    )]
    pub fn send_sound_event(&self, event_type: i32, pfields: &[Myflt], async_: i32) {
        unsafe {
            csound_sys::csoundEvent(
                self.csound_ptr(),
                event_type,
                pfields.as_ptr() as *mut Myflt,
                pfields.len() as c_int,
                async_,
            );
        }
    }

    /// Set the ASCII code of the most recent key pressed.
    /// # Arguments
    /// * `key` The ASCII identifier for the key pressed.
    pub fn key_press(&self, key: char) {
        unsafe {
            csound_sys::csoundKeyPress(self.csound_ptr(), key as c_char);
        }
    }

    /* Engine general Table function  implementations **************************************************************************************** */

    /// Returns the length of a function table (not including the guard point).
    ///
    /// # Guard Point
    ///
    /// Csound function tables include an extra "guard point" for efficient wraparound
    /// interpolation in wavetable oscillators. When you create a table with size N,
    /// Csound internally allocates N+1 points where the guard point (at index N) is
    /// a copy of the first point (at index 0).
    ///
    /// This function returns the logical table size N (without the guard point),
    /// which is what you should use when iterating over table data.
    ///
    /// The returned integer is an instantaneous size query and does not borrow
    /// table memory, so this method takes `&self`. Csound does not lock the
    /// underlying table-pointer lookup; a separate performance thread or
    /// asynchronous compilation may replace the table immediately after this
    /// method returns. Use [`Csound::read_table`] when an owned snapshot is
    /// needed, and do not use this value to size a later raw copy while the
    /// table may be resized concurrently.
    ///
    /// # Arguments
    /// * `table` - The function table identifier.
    ///
    /// # Returns
    /// - `Ok(length)` - The table length (number of usable data points, excluding guard point)
    /// - `Err(Error::NotFound)` - The table does not exist
    ///
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// // If table was created with "f1 0 1024 10 1"
    /// let len = cs.table_length(1).unwrap();
    /// assert_eq!(len, 1024); // Returns 1024, not 1025
    /// ```
    pub fn table_length(&self, table: TableId) -> Result<usize> {
        unsafe {
            let value = csound_sys::csoundTableLength(self.csound_ptr(), table as c_int) as i32;
            if value > 0 {
                Ok(value as usize)
            } else {
                tracing::error!(table_id = table, "table does not exist");
                Err(Error::NotFound("table does not exist"))
            }
        }
    }

    /// Returns an owned snapshot of a function table.
    ///
    /// The returned vector contains the table's logical data points and does
    /// not include Csound's guard point. Because the data is owned, it remains
    /// valid if Csound later replaces, resizes, or deletes the table.
    ///
    /// # Synchronization
    ///
    /// This method uses the synchronous form of [`Csound::table_copy_out`].
    /// Csound holds its API lock for the copy, and no reference into engine
    /// memory escapes the call; this is why snapshot access only requires
    /// `&self`. An asynchronous variant is intentionally not exposed because
    /// Csound would retain a raw pointer to the Rust allocation after its
    /// borrow ended.
    ///
    /// The table length is queried before the copy acquires Csound's API lock.
    /// A separate performance thread or pending asynchronous compilation must
    /// therefore not resize or delete this table concurrently with the call.
    ///
    /// # Arguments
    /// * `table` - The function table identifier.
    ///
    /// # Returns
    /// A vector containing the table data, excluding the guard point.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if the table does not exist
    /// - [`Error::InsufficientCapacity`] if the table grows between the length
    ///   query and the checked copy
    ///
    /// # Performance
    ///
    /// The copy is synchronous. Copying a large table may delay performance or
    /// other Csound API operations while the API lock is held.
    pub fn read_table(&self, table: TableId) -> Result<Vec<Myflt>> {
        let capacity = self.table_length(table)?;

        let mut dest = vec![Myflt::default(); capacity];

        self.table_copy_out(table, dest.as_mut_slice())?;

        Ok(dest)
    }

    /// Copies a function table's contents into `dest` synchronously.
    ///
    /// The copy uses Csound's synchronous table-copy API, which holds the API
    /// lock until the operation completes. No reference into Csound memory is
    /// returned, so the method can use `&self`: mutation of `dest` happens
    /// entirely within the call and engine access is synchronized internally.
    ///
    /// `dest` must hold at least [`Csound::table_length`] elements. Csound
    /// copies exactly the table length and does not include the guard point.
    /// This differs from [`Csound::table_copy_in`], which also transfers the
    /// guard point and therefore requires one additional element.
    ///
    /// The asynchronous C API variant is intentionally not exposed. It would
    /// retain `dest` as a raw pointer after this method returned, allowing Rust
    /// to read, modify, or drop the allocation before Csound wrote to it.
    ///
    /// # Concurrent table replacement
    ///
    /// The capacity check happens before Csound acquires its API lock. A
    /// separate performance thread or pending asynchronous compilation must
    /// not resize or delete this table concurrently; otherwise the checked
    /// length may become stale before the C copy begins.
    ///
    /// # Arguments
    /// * `table` - The function table identifier.
    /// * `dest` - Destination buffer, at least `table_length(table)` long.
    ///
    /// # Returns
    /// The number of elements copied, excluding the guard point.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if the table does not exist
    /// - [`Error::InsufficientCapacity`] if `dest` is shorter than the table
    ///
    /// # Performance
    ///
    /// The copy is synchronous. Copying a large table may delay performance or
    /// other Csound API operations while the API lock is held.
    pub fn table_copy_out(&self, table: TableId, dest: &mut [Myflt]) -> Result<usize> {
        let length = self.table_length(table)?;
        if dest.len() < length {
            return Err(Error::InsufficientCapacity {
                expected: length,
                actual: dest.len(),
            });
        }
        unsafe {
            csound_sys::csoundTableCopyOut(
                self.csound_ptr(),
                table as c_int,
                dest.as_mut_ptr(),
                0 as _,
            );
        }
        Ok(length)
    }

    /// Copies `src` into a function table synchronously.
    ///
    /// The copy uses Csound's synchronous table-copy API, which holds the API
    /// lock until the operation completes. Although this method changes engine
    /// data, it returns no reference into Csound memory and the mutation is
    /// synchronized internally; this is why it can take `&self`.
    ///
    /// `src` must hold at least [`Csound::table_length`] **plus one** elements.
    /// Csound copies `len + 1` values so that the final value replaces the
    /// table's guard point. For a wrapping wavetable, that final value should
    /// normally repeat the first data point. In contrast,
    /// [`Csound::table_copy_out`] excludes the guard point.
    ///
    /// The asynchronous C API variant is intentionally not exposed. It would
    /// retain `src` as a raw pointer after this method returned, allowing Rust
    /// to modify or drop the allocation before Csound read it.
    ///
    /// # Concurrent table replacement
    ///
    /// The length check happens before Csound acquires its API lock. A separate
    /// performance thread or pending asynchronous compilation must not resize
    /// or delete this table concurrently; otherwise Csound could read beyond
    /// the validated portion of `src`.
    ///
    /// # Arguments
    /// * `table` - The function table identifier.
    /// * `src` - Source buffer, at least `table_length(table) + 1` long; the
    ///   final element is copied into the guard point.
    ///
    /// # Returns
    /// The number of elements copied, including the guard point.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if the table does not exist
    /// - [`Error::InsufficientCapacity`] if `src` holds fewer than
    ///   `table_length + 1` elements
    ///
    /// # Performance
    ///
    /// The copy is synchronous. Copying a large table may delay performance or
    /// other Csound API operations while the API lock is held.
    pub fn table_copy_in(&self, table: TableId, src: &[Myflt]) -> Result<usize> {
        let length = self.table_length(table)?;
        // Csound copies len + 1 elements to write the guard point as well.
        let required = length + 1;
        if src.len() < required {
            return Err(Error::InsufficientCapacity {
                expected: required,
                actual: src.len(),
            });
        }
        unsafe {
            csound_sys::csoundTableCopyIn(self.csound_ptr(), table as c_int, src.as_ptr(), 0 as _);
        }
        Ok(required)
    }

    /// Provides scoped, zero-copy mutable access to a function table.
    ///
    /// The closure receives the table's logical data points directly in
    /// Csound-owned memory. The slice does not include the guard point and
    /// cannot escape the closure. Unlike [`Csound::table_copy_in`] and
    /// [`Csound::table_copy_out`], this method performs no copy.
    ///
    /// # Why this takes `&mut self`
    ///
    /// Csound documents [`csoundGetTable`](csound_sys::csoundGetTable) and its
    /// returned pointer as non-thread-safe, and no Csound lock is held while
    /// the closure executes. The exclusive borrow prevents any other safe Rust
    /// call from accessing the same Csound instance for the lifetime of the
    /// table slice. The higher-ranked closure lifetime also prevents the slice
    /// from being returned or stored for later safe use.
    ///
    /// The exclusive Rust borrow does not stop work already running inside
    /// Csound. Do not call this method while a separate performance thread is
    /// active or asynchronous compilation capable of replacing the table is
    /// pending.
    ///
    /// # Arguments
    /// * `id` - The function table identifier.
    /// * `f` - Closure executed with mutable access to the table data.
    ///
    /// # Returns
    /// The value returned by `f`.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if the table does not exist
    /// - [`Error::NullPointer`] if Csound reports a table but returns a null
    ///   data pointer
    ///
    /// # Example
    /// ```ignore
    /// # use csound::Csound;
    /// # let mut cs = Csound::new().unwrap();
    /// cs.with_table(1, |table| {
    ///     for value in table {
    ///         *value *= 0.5;
    ///     }
    /// })?;
    ///
    /// // The mutable table borrow ended with the closure.
    /// cs.perform_ksmps();
    /// # Ok::<(), csound::Error>(())
    /// ```
    pub fn with_table<R>(
        &mut self,
        id: TableId,
        f: impl for<'table> FnOnce(&'table mut [Myflt]) -> R,
    ) -> Result<R> {
        let mut ptr = ptr::null_mut() as *mut Myflt;
        let len = unsafe {
            csound_sys::csoundGetTable(self.csound_ptr(), &mut ptr as *mut *mut Myflt, id as c_int)
                as i32
        };

        if len < 0 {
            return Err(Error::NotFound("Table not found"));
        }

        let ptr =
            NonNull::new(ptr).ok_or(Error::NullPointer("Csound returned a null table pointer"))?;

        let table = unsafe { std::slice::from_raw_parts_mut(ptr.as_ptr(), len as usize) };
        Ok(f(table))
    }

    /// Returns an owned snapshot of the arguments used to define a function table.
    ///
    /// The argument list starts with the GEN number and is followed by its
    /// parameters. For example, `f 1 0 1024 10 1 0.5` produces
    /// `[10.0, 1.0, 0.5]`.
    ///
    /// Because the returned vector owns its data, it remains valid if Csound
    /// later redefines the table. The Csound argument-pointer lookup itself is
    /// non-thread-safe, however, so a separate performance thread or pending
    /// asynchronous compilation must not redefine or delete the table while
    /// this method is copying the arguments.
    ///
    /// # Arguments
    /// * `table` - The function table identifier.
    ///
    /// # Returns
    /// `Some` with the table-generation arguments, or `None` if the table does
    /// not exist.
    pub fn get_table_args(&self, table: TableId) -> Option<Vec<Myflt>> {
        let slice = self.get_table_args_slice(table)?;
        Some(slice.to_vec())
    }

    /// Gets the arguments used to construct or define a function table
    /// Similar to [`Csound::get_table_args`](struct.Csound.html#method.get_table_args)
    /// but no memory will be allocated, instead a slice is returned.
    fn get_table_args_slice(&self, table: TableId) -> Option<&[Myflt]> {
        let mut ptr = ptr::null_mut() as *mut Myflt;
        unsafe {
            let length = csound_sys::csoundGetTableArgs(
                self.csound_ptr(),
                &mut ptr as *mut *mut Myflt,
                table as c_int,
            );
            if length < 0 {
                None
            } else {
                Some(slice::from_raw_parts(ptr as *const Myflt, length as usize))
            }
        }
    }

    /* Engine general Opcode function  implementations **************************************************************************************** */

    /// Gets an alphabetically sorted list of all opcodes.
    /// Should be called after externals are loaded by csoundCompile().
    /// The opcode information is contained in a [`Csound::OpcodeListEntry`](struct.Csound.html#struct.OpcodeListEntry)
    pub fn get_opcode_list_entry(&self) -> Result<Vec<OpcodeListEntry>, Error> {
        let mut ptr: *mut csound_sys::opcodeListEntry = ptr::null_mut();
        let length = unsafe {
            csound_sys::csoundNewOpcodeList(
                self.csound_ptr(),
                &mut ptr as *mut *mut csound_sys::opcodeListEntry,
            )
        };

        if length < 0 {
            return Ok(vec![]);
        }

        // SAFETY: csoundNewOpcodeList returns a valid pointer when length >= 0
        let entries = unsafe { slice::from_raw_parts(ptr, length as usize) };
        let result = entries
            .iter()
            .map(|entry| {
                Ok(OpcodeListEntry {
                    opname: Trampoline::ptr_to_string(entry.opname)?,
                    outypes: Trampoline::ptr_to_string(entry.outypes)?,
                    intypes: Trampoline::ptr_to_string(entry.intypes)?,
                    flags: entry.flags,
                })
            })
            .collect();

        // Free the C-allocated opcode list
        unsafe {
            csound_sys::csoundDisposeOpcodeList(self.csound_ptr(), ptr);
        }

        result
    }

    // TODO genName and appendOpcode functions

    /* Engine miscellaneous functions **************************************************************************************** */

    /// `lang_code` can be for example any of [`Language`](enum.Language.html) variants.
    /// This affects all Csound instances running in the address
    /// space of the current process. The special language code
    /// [`Language::Default`] can be used to disable translation of messages and
    /// free all memory allocated by a previous call to this function.
    /// set_language() loads all files for the selected language from the directory specified by the **CSSTRNGS** environment
    /// variable.
    pub fn set_language(lang_code: Language) {
        unsafe {
            csound_sys::csoundSetLanguage(lang_code as u32);
        }
    }

    /// Generates a random seed from time
    /// # Returns
    /// A 32-bit unsigned integer to be used as random seed.
    pub fn get_random_seed_from_time() -> u32 {
        unsafe { csound_sys::csoundGetRandomSeedFromTime() as u32 }
    }

    /// Simple linear congruential random number generator: seed = seed * 742938285 % 2147483647
    /// # Returns
    /// The next number from the pseudo-random sequence, in the range 1 to 2147483646.
    /// if the value of seed is not in the range 1 to 2147483646 an error message will
    /// be returned.
    pub fn get_rand31(seed: &mut u32) -> Result<u32> {
        unsafe {
            match seed {
                1..=2_147_483_646 => {
                    let ptr: *mut u32 = &mut *seed;
                    let res = csound_sys::csoundRand31(ptr as *mut c_int) as u32;
                    Ok(res)
                }
                _ => Err(Error::InvalidSeed),
            }
        }
    }

    /// Returns an initialised timer structure.
    pub fn init_timer() -> RTCLOCK {
        let mut timer = RTCLOCK::default();
        unsafe {
            let ptr: *mut RTCLOCK = &mut timer as *mut RTCLOCK;
            csound_sys::csoundInitTimerStruct(ptr);
        }
        timer
    }

    /// Calculates a time offset
    /// # Arguments
    /// * `timer` time struct since the elapsed time will be calculated.
    /// # Returns
    /// The elapsed real time (in seconds) since the specified timer
    pub fn get_real_time(timer: &RTCLOCK) -> f64 {
        unsafe {
            let ptr: *mut csound_sys::RTCLOCK = &mut csound_sys::RTCLOCK {
                starttime_real: timer.starttime_real as c_long,
                starttime_CPU: timer.starttime_CPU as c_long,
            };
            csound_sys::csoundGetRealTime(ptr) as f64
        }
    }

    /// Return the elapsed CPU time (in seconds) since the specified *timer* structure was initialised.
    /// # Arguments
    /// * `gen` The GEN number identifier.
    pub fn get_cpu_time(timer: &mut RTCLOCK) -> f64 {
        unsafe { csound_sys::csoundGetCPUTime(timer as *mut RTCLOCK) as f64 }
    }

    /// Creates a circular buffer.
    /// # Arguments
    /// * `len` The buffer length.
    /// # Returns
    /// A [`CircularBuffer`], or an error if allocation fails.
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// let circular_buffer = csound.create_circular_buffer::<Myflt>(1024).unwrap();
    /// ```
    pub fn create_circular_buffer<'a, T: 'a + Copy>(
        &'a self,
        len: u32,
    ) -> Result<CircularBuffer<'a, T>> {
        unsafe {
            let ptr: *mut T = csound_sys::csoundCreateCircularBuffer(
                self.csound_ptr(),
                len as c_int,
                mem::size_of::<T>() as c_int,
            ) as *mut T;
            if ptr.is_null() {
                return Err(Error::Memory);
            }
            Ok(CircularBuffer {
                csound: self.csound_ptr(),
                ptr,
                phantom: PhantomData,
            })
        }
    }

    // Threading function

    pub fn sleep(&self, milli_seconds: usize) {
        unsafe {
            csound_sys::csoundSleep(milli_seconds);
        }
    }

    // TODO global variables functions

    /********************************** Callback settings using the custom callback Handler implementation******/

    /// Sets a function that is called to obtain a list of audio devices.
    /// This should be set by rtaudio modules and should not be set by hosts.
    pub fn audio_device_list_callback<'c, F>(&self, f: F)
    where
        F: FnMut(CsAudioDevice) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_devlist_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for opening real-time audio playback.
    /// This callback is used to inform the user about the current audio device Which
    /// Csound will use to play the audio samples.
    /// `user_func` A function/closure which will receive a reference
    ///  to a RtAudioParams struct.
    pub fn play_open_audio_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_play_open_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for opening real-time audio recording.
    /// This callback is used to inform the user about the current audio device Which
    /// Csound will use for opening realtime audio recording. You have to return Status::Success
    pub fn rec_open_audio_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_rec_open_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for performing real-time audio playback.
    /// A reference to a buffer with audio samples is passed
    /// to the user function in the callback. These samples have to be processed and sent
    /// to a proper audio device.
    pub fn rt_audio_play_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&[crate::Myflt]) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_rt_play_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for performing real-time audio recording.
    /// With this callback the user can fill a buffer with samples from a custom
    /// audio module, and pass it into csound.
    pub fn rt_audio_rec_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&mut [crate::Myflt]) -> usize + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_rt_rec_cb(self.csound_ptr(), f);
        }
    }

    /// Indicates to the user when csound has closed the rtaudio device.
    pub fn rt_close_callback<'c, F>(&self, f: F)
    where
        F: FnMut() + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_rt_close_cb(self.csound_ptr(), f);
        }
    }

    /*fn cscore_callback<'c, F>(&mut self, f:F)
        where F: FnMut() + 'c
    {
        self.engine.inner.handler.callbacks.cscore_cb = Some(Box::new(f));
        self.engine.enable_callback(CSCORE_CB);
    }*/

    /// Sets a callback which will be called by csound to print an informational message.
    /// ´f´ Function which implement the FnMut trait.
    /// The callback arguments are *u32* which indicates the message atributte,
    /// and a reference to the message content.
    /// # Example
    /// ```ignore
    /// use csound::{Csound, MessageType};
    /// let mut cs = Csound::new().unwrap();
    /// cs.message_string_callback(|att: MessageType, message: &str| print!("{}", message));
    /// ```
    pub fn message_string_callback<'c, F>(&'c self, f: F)
    where
        F: FnMut(MessageType, &str) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_message_cb(self.csound_ptr(), f);
        }
    }

    /*fn keyboard_callback<'c, F>(&self, f: F)
    where
        F: FnMut() -> char + 'c,
    {
        unsafe{(&mut *(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler)).callbacks.keyboard_cb = Some(Box::new(f));}
        self.enable_callback(KEYBOARD_CB);
    }*/

    /// Sets the function which will be called whenever the [*invalue*](http://www.csounds.com/manual/html/invalue.html) opcode is used.
    /// ´f´ Function which implement the FnMut trait. The invalue opcode will trigger this callback passing
    /// the channel name which requiere the data. This function/closure have to return the data which will be
    /// passed to that specific channel if not only return ChannelData::Unknown. Only *String* and *control* Channels are supported.
    /// # Example
    /// ```ignore
    /// use csound::{Csound, ChannelData};
    ///
    /// let input_channel = |name: &str| -> ChannelData {
    ///      if name == "myStringChannel"{
    ///          let myString = "my data".to_owned();
    ///          ChannelData::String(myString);
    ///      }
    ///      ChannelData::Unknown
    /// };
    /// let mut cs = Csound::new().unwrap();
    /// cs.input_channel_callback(input_channel);
    /// ```
    pub fn input_channel_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str) -> ChannelData + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_input_channel_cb(self.csound_ptr(), f);
        }
    }

    /// Sets the function which will be called whenever the [*outvalue*](http://www.csounds.com/manual/html/outvalue.html) opcode is used.
    /// ´f´ Function which implement the FnMut trait. The outvalue opcode will trigger this callback passing
    ///  the channel ##name and the channel's output data encoded in the ChannelData. Only *String* and *control* Channels are supported.
    /// # Example
    /// ```ignore
    /// use csound::{Csound, ChannelData};
    ///
    /// let output_channel = |name: &str, data:ChannelData|{
    ///      print!("channel name:{}  data: {:?}", name, data);
    /// };
    /// let mut cs = Csound::new().unwrap();
    /// cs.output_channel_callback(output_channel);
    /// ```
    pub fn output_channel_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str, ChannelData) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_output_channel_cb(self.csound_ptr(), f);
        }
    }

    /// Sets an external callback for receiving notices whenever Csound opens a file.
    /// The callback is made after the file is successfully opened.
    /// The following information is passed to the callback:
    /// ## `file_info`
    /// A [`FileInfo`](struct.FileInfo.html) struct containing the relevant file info.
    pub fn file_open_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&FileInfo) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_file_open_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for opening real-time MIDI input.
    /// This callback is used to inform to the user about the current MIDI input device.
    /// # Arguments
    /// * `user_func` A function/closure which will receive a reference to a str with the device name.
    pub fn midi_in_open_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_midi_in_open_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for opening real-time MIDI output.
    /// This callback is used to inform to the user about the current MIDI output device.
    /// # Arguments
    /// * `user_func` A function/closure which will receive a reference to a str with the device name.
    pub fn midi_out_open_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_midi_out_open_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for reading from real time MIDI input.
    /// A reference to a buffer with audio samples is passed
    /// to the user function in the callback.  The callback have to return the number of elements written to the buffer.
    pub fn midi_read_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&mut [u8]) -> usize + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_midi_read_cb(self.csound_ptr(), f);
        }
    }

    /// Sets a function to be called by Csound for Writing to real time MIDI input.
    /// A reference to the device buffer is passed
    /// to the user function in the callback. The passed buffer have the max length that
    /// the user is able to use, and the callback have to return the number of element written into the buffer.
    pub fn midi_write_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&[u8]) -> usize + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_midi_write_cb(self.csound_ptr(), f);
        }
    }

    /// Indicates to the user when csound has closed the midi input device.
    pub fn midi_in_close_callback<'c, F>(&self, f: F)
    where
        F: FnMut() + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_midi_in_close_cb(self.csound_ptr(), f);
        }
    }

    /// Indicates to the user when csound has closed the midi output device.
    pub fn midi_out_close_callback<'c, F>(&self, f: F)
    where
        F: FnMut() + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_midi_out_close_cb(self.csound_ptr(), f);
        }
    }

    /// Returns a reference to the panic state for this Csound instance.
    ///
    /// This is useful for checking which callbacks have panicked.
    /// The panic state tracks which callbacks have panicked and should
    /// be skipped on subsequent invocations.
    pub fn panic_state(&self) -> &PanicState {
        unsafe {
            &(*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .panic_state
        }
    }
} //End impl block

/// Drop implementation follows the proper Csound shutdown sequence:
/// 1. Reset csound state
/// 2. Destroy message buffer (if created) - this clears host_data, which is fine since we're shutting down
/// 3. Destroy the csound instance
/// 4. Free the callback handler we own
impl Drop for Csound {
    fn drop(&mut self) {
        unsafe {
            // Reset csound state
            csound_sys::csoundReset(self.csound_ptr());

            // Destroy message buffer if it was created.
            // We detect this using csoundGetMessageCnt() which returns -1 when no buffer
            // exists, and >= 0 when a buffer is allocated (even if empty).
            // This avoids needing to track buffer state ourselves.
            if self.has_message_buffer() {
                csound_sys::csoundDestroyMessageBuffer(self.csound_ptr());
            }

            // Destroy the csound instance
            csound_sys::csoundDestroy(self.csound_ptr());

            // Free the callback handler (we own it via host_data)
            drop(Box::from_raw(self.engine.host_data.as_ptr()));
        }
    }
}

/// Csound's Circular Buffer object.
/// This struct wraps a *mut T pointer to a circular buffer
/// allocated by csound. This Circular buffer won't outlive
/// the csound instance that allocated the buffer.
pub struct CircularBuffer<'a, T: 'a + Copy> {
    csound: *mut csound_sys::CSOUND,
    ptr: *mut T,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> CircularBuffer<'a, T>
where
    T: Copy,
{
    /// Read from circular buffer.
    /// # Arguments
    /// * `out` A mutable slice where the items will be copied.
    /// * `items` The number of elements to read and remove from the buffer.
    /// # Returns
    /// The number of items read **(0 <= n <= items)**.
    /// or an Error if the output buffer doesn't have enough capacity.
    pub fn read(&self, out: &mut [T], items: u32) -> Result<usize> {
        if (items as usize) > out.len() {
            tracing::error!(
                expected = items,
                actual = out.len(),
                "circular buffer read: output buffer too small"
            );
            return Err(Error::InsufficientCapacity {
                expected: items as usize,
                actual: out.len(),
            });
        }
        unsafe {
            Ok(csound_sys::csoundReadCircularBuffer(
                self.csound,
                self.ptr as *mut c_void,
                out.as_mut_ptr() as *mut c_void,
                items as c_int,
            ) as usize)
        }
    }

    /// Read from circular buffer without removing them from the buffer.
    /// # Arguments
    /// * `out` A mutable slice where the items will be copied.
    /// * `items` The number of elements to peek from the buffer.
    /// # Returns
    /// The actual number of items read **(0 <= n <= items)**, or an error if the number of items
    /// to read/write exceeds the buffer's capacity.
    pub fn peek(&self, out: &mut [T], items: u32) -> Result<usize> {
        if (items as usize) > out.len() {
            tracing::error!(
                expected = items,
                actual = out.len(),
                "circular buffer peek: output buffer too small"
            );
            return Err(Error::InsufficientCapacity {
                expected: items as usize,
                actual: out.len(),
            });
        }
        unsafe {
            Ok(csound_sys::csoundPeekCircularBuffer(
                self.csound,
                self.ptr as *mut c_void,
                out.as_mut_ptr() as *mut c_void,
                items as c_int,
            ) as usize)
        }
    }

    /// Write to the circular buffer.
    /// # Arguments
    /// * `input` A slice with the date which will be copied into the buffer.
    /// * `items` The number of elements to wrtie into the buffer.
    /// # Returns
    /// The actual number of items written *(0 <= n <= items)**, or an error if the number of items
    /// to read/write exceeds the buffer's capacity.
    pub fn write(&self, input: &[T], items: u32) -> Result<usize> {
        if (items as usize) > input.len() {
            tracing::error!(
                expected = items,
                actual = input.len(),
                "circular buffer write: input buffer too small"
            );
            return Err(Error::InsufficientCapacity {
                expected: items as usize,
                actual: input.len(),
            });
        }
        unsafe {
            Ok(csound_sys::csoundWriteCircularBuffer(
                self.csound,
                self.ptr as *mut c_void,
                input.as_ptr() as *const c_void,
                items as c_int,
            ) as usize)
        }
    }

    /// Empty circular buffer of any remaining data.
    /// This function should only be used if there is no reader actively getting data from the buffer.
    pub fn flush(&self) {
        unsafe {
            csound_sys::csoundFlushCircularBuffer(self.csound, self.ptr as *mut c_void);
        }
    }
}

impl<'a, T> Drop for CircularBuffer<'a, T>
where
    T: Copy,
{
    fn drop(&mut self) {
        unsafe {
            csound_sys::csoundDestroyCircularBuffer(self.csound, self.ptr as *mut c_void);
        }
    }
}

pub enum Readable {}
pub enum Writable {}

/// Csound buffer pointer representation.
/// This struct is build up to manipulate directly csound's buffers.
pub struct BufferPtr<'a, T> {
    ptr: *mut Myflt,
    len: usize,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> BufferPtr<'a, T> {
    /// # Returns
    /// The buffer length
    pub fn get_size(&self) -> usize {
        self.len
    }

    /// This method is used to copy data from the csound's buffer
    /// into another slice.
    /// # Arguments
    /// * `slice` A mutable slice where the data will be copy
    /// # Returns
    /// The number of elements copied into the slice.
    pub fn copy_to_slice(&self, slice: &mut [Myflt]) -> usize {
        let len = slice.len().min(self.get_size());
        slice[..len].copy_from_slice(&self.as_slice()[..len]);
        len
    }

    /// # Returns
    /// A slice to the buffer internal data
    pub fn as_slice(&self) -> &[Myflt] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<'a> BufferPtr<'a, Writable> {
    /// # Returns
    /// This buffer pointer as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [Myflt] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    /// method used to copy data into this buffer
    /// # Arguments
    /// * `slice` A slice with samples to copy
    /// # Returns
    /// The number of elements copied into the csound's buffer.
    pub fn copy_from_slice(&self, slice: &[Myflt]) -> usize {
        let len = slice.len().min(self.get_size());
        // SAFETY: pointer is valid for the buffer lifetime; length is bounded by buffer size.
        unsafe {
            let dst = slice::from_raw_parts_mut(self.ptr, len);
            dst.copy_from_slice(&slice[..len]);
        }
        len
    }

    /// method used to clear the buffer's data
    pub fn clear(&mut self) {
        self.as_mut_slice().fill(0.0);
    }
}

impl<'a, T> AsRef<[Myflt]> for BufferPtr<'a, T> {
    fn as_ref(&self) -> &[Myflt] {
        self.as_slice()
    }
}

impl<'a> AsMut<[Myflt]> for BufferPtr<'a, Writable> {
    fn as_mut(&mut self) -> &mut [Myflt] {
        self.as_mut_slice()
    }
}

impl<'a, T> Deref for BufferPtr<'a, T> {
    type Target = [Myflt];
    fn deref(&self) -> &[Myflt] {
        self.as_slice()
    }
}

impl<'a> DerefMut for BufferPtr<'a, Writable> {
    fn deref_mut(&mut self) -> &mut [Myflt] {
        self.as_mut_slice()
    }
}
