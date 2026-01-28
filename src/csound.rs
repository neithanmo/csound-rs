use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};
use std::slice;
use std::sync::OnceLock;

use crate::callbacks::*;
use crate::channels::{
    ChannelBehavior, ChannelHints, ChannelInfo, InputChannel, IsChannel, OutputChannel,
};
use crate::enums::{ChannelData, ControlChannelType, Language, MessageType, Status};
use crate::error::{Error, Result};
use crate::rtaudio::{CsAudioDevice, CsMidiDevice, RtAudioParams};

use csound_sys::{CSOUND_STATUS, RTCLOCK, controlChannelType};

use std::ffi::{CStr, CString};
use std::str;

use libc::{c_char, c_double, c_int, c_long, c_void};

/// Struct with information about a csound opcode.
///
/// Used to get the complete csound opcodes list, so the
/// [`Csound::get_opcode_list_entry`](struct.Csound.html#method.get_opcode_list_entry) method will return
/// a list of OpcodeListEntry, where each of this struct contain information relative
/// a specific csound opcode.
#[derive(Default, Debug)]
pub struct OpcodeListEntry {
    /// The opcode name.
    pub opname: Option<String>,
    /// The opcode ouput type.
    pub outypes: Option<String>,
    /// The opcode input type.
    pub intypes: Option<String>,
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
static CSOUND_INIT: OnceLock<()> = OnceLock::new();

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
    /// ```no_run
    /// use csound::{Csound, MessageType};
    ///
    /// // Creates a Csound instance with a custom callback handler
    /// let csound = Csound::new().expect("Failed to create Csound instance");
    ///
    /// // Enable the message callback by passing a closure
    /// csound.message_string_callback(|mtype: MessageType, message: &str| {
    ///     println!("message type: {:?} message content: {}", mtype, message);
    /// });
    ///
    /// # let csd_filename = "file.csd";
    /// csound.compile_csd(csd_filename, 0, 0).unwrap();
    /// csound.start().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        // Initialize csound library exactly once (thread-safe)
        CSOUND_INIT.get_or_init(|| {
            let flags = (csound_sys::CSOUNDINIT_NO_SIGNAL_HANDLER
                | csound_sys::CSOUNDINIT_NO_ATEXIT) as c_int;
            unsafe {
                csound_sys::csoundInitialize(flags);
            }
        });

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

        Ok(Csound {
            engine: Inner { csound, host_data },
        })
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
                CSOUND_STATUS::CSOUND_ERROR => Err(Error::InitFailed),
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
                _ => Err(Error::InvalidOption(option.to_string())),
            }
        }
    }

    /// Returns the raw csound pointer for FFI calls.
    #[inline]
    fn csound_ptr(&self) -> *mut csound_sys::CSOUND {
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
    /// ```no_run
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
        unsafe {
            if csound_sys::csoundStart(self.csound_ptr()) == CSOUND_STATUS::CSOUND_SUCCESS {
                Ok(())
            } else {
                Err(Error::AlreadyStarted)
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
    pub fn reset(&self) {
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
                _ => Err(Error::CompileFailed("failed to compile csound arguments")),
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
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let csound  = Csound::new().unwrap();
    /// csound.set_option("-an_option");
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
    /// ```no_run
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
            return Err(Error::EmptyString);
        }
        let path = CString::new(csd_ref)?;
        unsafe {
            match csound_sys::csoundCompileCSD(self.csound_ptr(), path.as_ptr(), mode, async_) {
                CSOUND_STATUS::CSOUND_SUCCESS => Ok(()),
                _ => Err(Error::CompileFailed("failed to compile csd file")),
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
            return Err(Error::EmptyString);
        }
        let code = CString::new(orc_ref)?;
        unsafe {
            match csound_sys::csoundCompileOrc(self.csound_ptr(), code.as_ptr(), async_) {
                CSOUND_STATUS::CSOUND_SUCCESS => Ok(()),
                _ => Err(Error::CompileFailed("failed to compile orchestra")),
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
    pub fn eval_code<T>(&self, code: T) -> Result<f64>
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

    /// Senses input events, and performs one control sample worth ```ksmps * number of channels * size_off::<f64> bytes``` of audio output.
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
    /// # Returns
    /// *Ok* on success or an error code on failure.
    pub fn udp_server_start(&self, port: u32) -> Result<(), Status> {
        unsafe {
            match Status::from(csound_sys::csoundUDPServerStart(self.csound_ptr(), port) as i32) {
                Status::Success => Ok(()),
                e => Err(e),
            }
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
    /// # Returns
    /// *Ok* if the running server was successfully closed, Status code otherwise.
    pub fn udp_server_close(&self) -> Result<(), Status> {
        unsafe {
            match Status::from(csound_sys::csoundUDPServerClose(self.csound_ptr()) as i32) {
                Status::Success => Ok(()),
                status => Err(status),
            }
        }
    }

    /// Turns on the transmission of console messages
    ///
    /// # Arguments
    /// * `addr` The UDP server destination address.
    /// * `port` The UDP server port number.
    /// * `mirror` If it is true, the messages will continue to be sent to the usual destination
    /// (see [`Csound::message_string_callback`](struct.Csound.html#method.message_string_callback) ) as well as to UDP.
    /// # Returns
    /// *Ok* on success or an Status code if the UDP transmission could not be set up.
    pub fn udp_console(&self, addr: &str, port: u32, mirror: bool) -> Result<(), Status> {
        unsafe {
            let ip = CString::new(addr).map_err(|_e| Status::Error)?;
            if csound_sys::csoundUDPConsole(
                self.csound_ptr(),
                ip.as_ptr(),
                port as c_int,
                mirror as c_int,
            ) == CSOUND_STATUS::CSOUND_SUCCESS
            {
                return Ok(());
            }
            Err(Status::Error)
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
    pub fn get_sample_rate(&self) -> f64 {
        unsafe { csound_sys::csoundGetSr(self.csound_ptr()) as f64 }
    }

    /// # Returns
    /// The number of control samples per second.
    pub fn get_control_rate(&self) -> f64 {
        unsafe { csound_sys::csoundGetKr(self.csound_ptr()) as f64 }
    }

    /// # Returns
    /// The number of audio sample frames per control sample.
    pub fn get_ksmps(&self) -> u32 {
        unsafe { csound_sys::csoundGetKsmps(self.csound_ptr()) }
    }

    /// # Returns
    /// The number of audio output channels. Set through the nchnls header variable in the csd file.
    /// is_input can be 1 or 0
    pub fn get_channels(&self, is_input: i32) -> u32 {
        unsafe { csound_sys::csoundGetChannels(self.csound_ptr(), is_input) }
    }

    /// # Returns
    /// The 0dBFS level of the spin/spout buffers.
    pub fn get_0d_bfs(&self) -> f64 {
        unsafe { csound_sys::csoundGet0dBFS(self.csound_ptr()) as f64 }
    }

    /// # Returns
    /// The A4 frequency reference
    pub fn get_freq(&self) -> f64 {
        unsafe { csound_sys::csoundGetA4(self.csound_ptr()) as f64 }
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
    /// ```no_run
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
    pub fn get_spin(&self) -> Option<BufferPtr<'_, Writable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpin(self.csound_ptr()) as *mut f64;
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
    /// ```no_run
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
    pub fn get_spout(&self) -> Option<BufferPtr<'_, Readable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpout(self.csound_ptr()) as *mut f64;
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
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd("some_file_path", 0, 0);
    /// csound.start();
    /// let spout_length = csound.get_ksmps() * csound.get_channels(0); // get output channels
    /// let mut spout_buffer = vec![0f64; spout_length as usize];
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
    pub fn read_spout_buffer(&self, output: &mut [f64]) -> Result<usize> {
        let size = self.get_ksmps() as usize * self.get_channels(0) as usize;
        let spout = unsafe { csound_sys::csoundGetSpout(self.csound_ptr()) as *const f64 };
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
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// csound.compile_csd("some_file_path", 0, 0);
    /// csound.start();
    /// let spin_length = csound.get_ksmps() * csound.get_channels(1); // get input channels
    /// let mut spin_buffer = vec![0f64; spin_length as usize];
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
    pub fn write_spin_buffer(&self, input: &[f64]) -> Result<usize> {
        let size = self.get_ksmps() as usize * self.get_channels(1) as usize;
        let spin = unsafe { csound_sys::csoundGetSpin(self.csound_ptr()) };
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
    /// # Returns
    /// A tuple, being input devices the first element in the returned tuple, output devices the
    /// second one.
    pub fn get_audio_devices(&self) -> (Vec<CsAudioDevice>, Vec<CsAudioDevice>) {
        let mut input_devices = Vec::new();
        let mut output_devices = Vec::new();

        unsafe {
            let num_of_idevices =
                csound_sys::csoundGetAudioDevList(self.csound_ptr(), ptr::null_mut(), 0);
            let num_of_odevices =
                csound_sys::csoundGetAudioDevList(self.csound_ptr(), ptr::null_mut(), 0);

            let mut in_vec = vec![csound_sys::CS_AUDIODEVICE::default(); num_of_idevices as usize];
            let mut out_vec = vec![csound_sys::CS_AUDIODEVICE::default(); num_of_odevices as usize];

            csound_sys::csoundGetAudioDevList(self.csound_ptr(), in_vec.as_mut_ptr(), 0);
            csound_sys::csoundGetAudioDevList(self.csound_ptr(), out_vec.as_mut_ptr(), 1);

            for dev in &in_vec {
                input_devices.push(CsAudioDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    rt_module: Trampoline::ptr_to_string(dev.rt_module.as_ptr()),
                    max_nchnls: dev.max_nchnls as u32,
                    is_output: 0,
                });
            }
            for dev in &out_vec {
                output_devices.push(CsAudioDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    rt_module: Trampoline::ptr_to_string(dev.rt_module.as_ptr()),
                    max_nchnls: dev.max_nchnls as u32,
                    is_output: 1,
                });
            }
        }
        (input_devices, output_devices)
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

    /// This function can be called to obtain a list of available input or output midi devices.
    /// # Returns
    /// A tuple with two vectors, beign the first one for input MIDI
    /// devices and the second one for output MIDI devices
    pub fn get_midi_devices(&self) -> (Vec<CsMidiDevice>, Vec<CsMidiDevice>) {
        let mut input_devices = Vec::new();
        let mut output_devices = Vec::new();

        unsafe {
            let num_of_idevices =
                csound_sys::csoundGetMIDIDevList(self.csound_ptr(), ptr::null_mut(), 0);
            let num_of_odevices =
                csound_sys::csoundGetMIDIDevList(self.csound_ptr(), ptr::null_mut(), 0);

            let mut in_vec = vec![csound_sys::CS_MIDIDEVICE::default(); num_of_idevices as usize];
            let mut out_vec = vec![csound_sys::CS_MIDIDEVICE::default(); num_of_odevices as usize];

            csound_sys::csoundGetMIDIDevList(self.csound_ptr(), in_vec.as_mut_ptr(), 0);
            csound_sys::csoundGetMIDIDevList(self.csound_ptr(), out_vec.as_mut_ptr(), 1);

            for dev in &in_vec {
                input_devices.push(CsMidiDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    midi_module: Trampoline::ptr_to_string(dev.midi_module.as_ptr()),
                    interface_name: Trampoline::ptr_to_string(dev.interface_name.as_ptr()),
                    is_output: 0,
                });
            }
            for dev in &out_vec {
                output_devices.push(CsMidiDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    midi_module: Trampoline::ptr_to_string(dev.midi_module.as_ptr()),
                    interface_name: Trampoline::ptr_to_string(dev.interface_name.as_ptr()),
                    is_output: 1,
                });
            }
        }
        (input_devices, output_devices)
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
    pub fn get_score_offset_seconds(&self) -> f64 {
        unsafe { csound_sys::csoundGetScoreOffsetSeconds(self.csound_ptr()) as f64 }
    }

    /// Csound score events prior to the specified time are not performed.
    /// And performance begins immediately at the specified time
    /// (real-time events will continue to be performed as they are received).
    /// Can be used by external software, such as a VST host, to begin score performance midway through a Csound score,
    ///  for example to repeat a loop in a sequencer or to synchronize other events with the Csound score.
    pub fn set_score_offset_seconds(&self, offset: f64) {
        unsafe {
            csound_sys::csoundSetScoreOffsetSeconds(self.csound_ptr(), offset as c_double);
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
        unsafe {
            match CStr::from_ptr(csound_sys::csoundGetFirstMessage(self.csound_ptr())).to_str() {
                Ok(m) => Some(m.to_owned()),
                _ => None,
            }
        }
    }

    /// # Returns
    /// The attribute parameter ([`MessageType`](enum.MessageType.html)) of the first message in the buffer.
    pub fn get_first_message_attr(&self) -> MessageType {
        unsafe {
            MessageType::from(csound_sys::csoundGetFirstMessageAttr(self.csound_ptr()) as u32)
        }
    }

    /// Removes the first message from the buffer.
    pub fn pop_first_message(&self) {
        unsafe {
            csound_sys::csoundPopFirstMessage(self.csound_ptr());
        }
    }

    /// # Returns
    /// The number of pending messages in the buffer.
    pub fn get_message_count(&self) -> u32 {
        unsafe { csound_sys::csoundGetMessageCnt(self.csound_ptr()) as u32 }
    }

    /* Engine general Channels, Control and Events implementations ********************************************** */

    /// Requests a list of all control channels.
    /// # Returns
    /// A vector with all control channels info or None if there are not control channels. see: [`ChannelInfo`](struct.ChannelInfo.html)
    pub fn list_channels(&self) -> Option<Vec<ChannelInfo>> {
        let mut ptr = ptr::null_mut() as *mut csound_sys::controlChannelInfo_t;
        let ptr2: *mut *mut csound_sys::controlChannelInfo_t = &mut ptr as *mut *mut _;

        unsafe {
            let count = csound_sys::csoundListChannels(self.csound_ptr(), ptr2) as i32;
            let mut ptr = *ptr2;

            if count > 0 {
                let mut list = Vec::new();
                for _ in 0..count {
                    let name = match Trampoline::ptr_to_string((*ptr).name) {
                        Some(string) => string,
                        None => "".into(),
                    };

                    let ctype = (*ptr).type_ as i32;
                    let hints = (*ptr).hints;

                    let attributes = match Trampoline::ptr_to_string(hints.attributes) {
                        Some(string) => string,
                        None => "".into(),
                    };

                    list.push(ChannelInfo {
                        name,
                        type_: ctype,
                        hints: ChannelHints {
                            behav: ChannelBehavior::from_u32(hints.behav as u32),
                            dflt: hints.dflt as f64,
                            min: hints.min as f64,
                            max: hints.max as f64,
                            x: hints.x as i32,
                            y: hints.y as i32,
                            width: hints.width as i32,
                            height: hints.height as i32,
                            attributes,
                        },
                    });
                    ptr = ptr.add(1);
                }
                csound_sys::csoundDeleteChannelList(self.csound_ptr(), *ptr2);
                return Some(list);
            }
            None
        }
    }

    /// Return a [`InputChannel`](struct.InputChannel.html) which represent a csound's input channel ptr.
    /// creating the channel first if it does not exist yet.
    /// # Arguments
    /// * `name` The channel name.
    /// *
    /// The generic parameter `T` in this function can be one of the following types:
    ///  - ControlChannel
    ///     control data (one MYFLT value)
    ///  - AudioChannel
    ///     audio data (get_ksmps() f64 values)
    ///  - StrChannel:
    ///     string data (u8 values with enough space to store
    ///     get_channel_data_size() characters, including the
    ///     NULL character at the end of the string)
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
    /// * Note: to find out the type of a channel without actually
    /// creating or changing it, set 'channel_type' argument  to CSOUND_UNKNOWN_CHANNEL, so that the error
    /// value will be either the type of the channel, or CSOUND_STATUS::CSOUND_ERROR
    /// if it does not exist.
    /// Operations on the channel pointer are not thread-safe by default. The host is
    /// required to take care of threadsafety by
    ///   1) with control channels use __sync_fetch_and_add() or
    ///      __sync_fetch_and_or() gcc atomic builtins to get or set a channel,
    ///      if available.
    ///   2) For string and audio channels (and controls if option 1 is not
    ///      available), retrieve the channel lock with ChannelLock()
    ///      and use SpinLock() and SpinUnLock() to protect access
    ///      to the channel.
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
    /// println!("channel value {}", control_channel.read());
    /// // Request a csound's input audio channel
    /// let audio_channel = csound.get_input_channel::<AudioChannle>("myAudioChannel").unwrap();
    /// println!("audio channel samples {:?}", audio_channel.read() );
    /// // Request a csound's input string channel
    /// let string_channel = csound.get_input_channel::<StrChannel>("myStringChannel").unwrap();
    ///
    /// ```
    pub fn get_input_channel<T>(&self, name: &str) -> Result<InputChannel<'_, T>, Status>
    where
        T: IsChannel,
    {
        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let len;
        let bits;

        match T::c_type() {
            ControlChannelType::Audio => {
                len = self.get_ksmps() as usize;
                bits = (controlChannelType::CSOUND_AUDIO_CHANNEL
                    | controlChannelType::CSOUND_INPUT_CHANNEL) as c_int;
            }
            ControlChannelType::Control => {
                len = 1;
                bits = (controlChannelType::CSOUND_CONTROL_CHANNEL
                    | controlChannelType::CSOUND_INPUT_CHANNEL) as c_int;
            }
            ControlChannelType::String => {
                len = self.get_channel_data_size(name) as usize;
                bits = (controlChannelType::CSOUND_STRING_CHANNEL
                    | controlChannelType::CSOUND_INPUT_CHANNEL) as c_int;
            }
            _ => unimplemented!(),
        }

        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, bits));
            match result {
                Status::Success => InputChannel::from_raw(*ptr, len).ok_or(Status::Error),
                Status::Ok(channel) => Err(Status::Ok(channel)),
                result => Err(result),
            }
        }
    }

    /// Return a [`OutputChannel`](struct.OutputChannel.html) which represent a csound's output channel ptr.
    /// creating the channel first if it does not exist yet.
    /// # Arguments
    /// * `name` The channel name.
    /// *
    /// The generic parameter `T` in this function can be one of the following types:
    ///  - ControlChannel
    ///     control data (one MYFLT value)
    ///  - AudioChannel
    ///     audio data (get_ksmps() f64 values)
    ///  - StrChannel:
    ///     string data (u8 values with enough space to store
    ///     get_channel_data_size() characters, including the
    ///     NULL character at the end of the string)
    /// If the channel already exists, it must match the data type
    /// (control, audio, or string)
    /// # Note
    ///  Audio and String channels
    /// can only be created after calling compile(), because the
    /// storage size is not known until then.
    /// # Returns
    /// A  Readable OutputChannel on success or a Status code,
    ///   "Not enough memory for allocating the channel" (CS_MEMORY)
    ///   "The specified name or type is invalid" (CS_ERROR)
    /// or, if a channel with the same name but incompatible type
    /// already exists, the type of the existing channel.
    /// * Note: to find out the type of a channel without actually
    /// creating or changing it, set 'channel_type' argument  to CSOUND_UNKNOWN_CHANNEL, so that the error
    /// value will be either the type of the channel, or CSOUND_STATUS::CSOUND_ERROR
    /// if it does not exist.
    /// Operations on the channel pointer are not thread-safe by default. The host is
    /// required to take care of threadsafety by
    ///   1) with control channels use __sync_fetch_and_add() or
    ///      __sync_fetch_and_or() gcc atomic builtins to get or set a channel,
    ///      if available.
    ///   2) For string and audio channels (and controls if option 1 is not
    ///      available), retrieve the channel lock with ChannelLock()
    ///      and use SpinLock() and SpinUnLock() to protect access
    ///      to the channel.
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
    /// // Writes some data to the channel
    /// println!("channel value {}", control_channel.read());
    /// // Request a csound's output audio channel
    /// let audio_channel = csound.get_output_channel::<AudioChannle>("myAudioChannel").unwrap();
    /// println!("audio channel samples {:?}", audio_channel.read() );
    /// // Request a csound's output string channel
    /// let string_channel = csound.get_output_channel::<StrChannel>("myStringChannel").unwrap();
    ///
    /// ```
    pub fn get_output_channel<T>(&self, name: &str) -> Result<OutputChannel<'_, T>, Status>
    where
        T: IsChannel,
    {
        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;

        let len;
        let bits;

        match T::c_type() {
            ControlChannelType::Audio => {
                len = self.get_ksmps() as usize;
                bits = (controlChannelType::CSOUND_AUDIO_CHANNEL
                    | controlChannelType::CSOUND_OUTPUT_CHANNEL) as c_int;
            }
            ControlChannelType::Control => {
                len = 1;
                bits = (controlChannelType::CSOUND_CONTROL_CHANNEL
                    | controlChannelType::CSOUND_OUTPUT_CHANNEL) as c_int;
            }
            ControlChannelType::String => {
                len = self.get_channel_data_size(name) as usize;
                bits = (controlChannelType::CSOUND_STRING_CHANNEL
                    | controlChannelType::CSOUND_OUTPUT_CHANNEL) as c_int;
            }
            _ => unimplemented!(),
        }

        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, bits));
            match result {
                Status::Success => OutputChannel::from_raw(*ptr, len).ok_or(Status::Error),
                Status::Ok(channel) => Err(Status::Ok(channel)),
                result => Err(result),
            }
        }
    }

    pub(crate) fn get_raw_channel_ptr(
        &self,
        name: &str,
        ptr: *mut *mut f64,
        channel_type: c_int,
    ) -> c_int {
        let cname = match CString::new(name) {
            Ok(c) => c,
            Err(_) => return -1,
        };
        unsafe {
            csound_sys::csoundGetChannelPtr(
                self.csound_ptr(),
                ptr as *mut *mut c_void,
                cname.as_ptr(),
                channel_type,
            )
        }
    }

    /// Set parameters hints for a control channel.
    /// These hints have no internal function but can be used by front ends to construct GUIs or to constrain values.
    /// # Returns
    /// CS_SUCCESS on success, or CS_ERROR on failure: the channel does not exist, is not a control channel,
    /// or the specified parameters are invalid or CS_MEMORY: could not allocate memory for the
    /// channel. see: ([`Status`](enum.Status.html))
    pub fn set_channel_hints(&self, name: &str, hint: &ChannelHints) -> Result<(), Status> {
        let attr = &hint.attributes[..];
        let attr = CString::new(attr).map_err(|_| Status::Error)?;
        let cname = CString::new(name).map_err(|_| Status::Error)?;
        let channel_hint = csound_sys::controlChannelHints_t {
            behav: ChannelBehavior::to_u32(&hint.behav),
            dflt: hint.dflt,
            min: hint.min,
            max: hint.max,
            x: hint.x,
            y: hint.y,
            width: hint.width as c_int,
            height: hint.height as c_int,
            attributes: attr.as_ptr() as *mut c_char,
        };
        unsafe {
            match Status::from(csound_sys::csoundSetControlChannelHints(
                self.csound_ptr(),
                cname.as_ptr(),
                channel_hint,
            ) as i32)
            {
                Status::Success => Ok(()),
                status => Err(status),
            }
        }
    }

    /// Returns special parameters (or None if there are not any) of a control channel.
    /// Previously set with csoundSetControlChannelHints() or the
    /// [chnparams](http://www.csounds.com/manualOLPC/chnparams.html) opcode.
    pub fn get_channel_hints(&self, name: &str) -> Result<ChannelHints, Status> {
        let cname = CString::new(name).map_err(|_| Status::Error)?;
        let mut hint = csound_sys::controlChannelHints_t::default();
        unsafe {
            match csound_sys::csoundGetControlChannelHints(
                self.csound_ptr(),
                cname.as_ptr() as *mut c_char,
                &mut hint as *mut _,
            ) {
                CSOUND_STATUS::CSOUND_SUCCESS => {
                    let attributes = match Trampoline::ptr_to_string(hint.attributes) {
                        Some(name) => name,
                        None => "".into(),
                    };

                    let hints = ChannelHints {
                        behav: ChannelBehavior::from_u32(hint.behav as u32),
                        dflt: hint.dflt,
                        min: hint.min,
                        max: hint.max,
                        x: hint.x as i32,
                        y: hint.y as i32,
                        width: hint.width as i32,
                        height: hint.height as i32,
                        attributes,
                    };
                    Ok(hints)
                }

                status => Err(Status::from(status)),
            }
        }
    }

    /// Retrieves the value of a control channel.
    /// # Arguments
    /// * `name`  The channel name.
    /// An error message will be returned if the channel is not a control channel,
    /// the channel not exist or if the name is invalid.
    pub fn get_control_channel(&self, name: &str) -> Result<f64> {
        let cname = CString::new(name)?;
        let mut err: c_int = 0;
        unsafe {
            let ret = csound_sys::csoundGetControlChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                &mut err as *mut _,
            ) as f64;
            if (err) == CSOUND_STATUS::CSOUND_SUCCESS {
                Ok(ret)
            } else {
                Err(Error::NotFound(
                    "channel does not exist or is not a control channel",
                ))
            }
        }
    }

    /// Sets the value of a control channel.
    /// # Arguments
    /// * `name`  The channel name.
    pub fn set_control_channel(&mut self, name: &str, value: f64) {
        let cname = CString::new(name).unwrap();
        unsafe {
            csound_sys::csoundSetControlChannel(self.csound_ptr(), cname.as_ptr(), value);
        }
    }

    /// Copies samples from an audio channel.
    /// # Arguments
    /// * `name` The channel name.
    /// * `out` The slice where the date contained in the internal audio channel buffer
    /// will be copied. Should contain enough memory for ksmps f64 samples.
    /// # Panic
    /// If the buffer passed to this function doesn't have enough memory.
    pub fn read_audio_channel(&self, name: &str, output: &mut [f64]) {
        let ksmps = self.get_ksmps() as usize;
        let size = output.len();
        let cname = CString::new(name).unwrap();
        assert!(
            ksmps <= size,
            "The audio channel's capacity is {} so, it isn't possible to copy {} samples",
            size,
            ksmps
        );
        unsafe {
            csound_sys::csoundGetAudioChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                output.as_ptr() as *mut c_double,
            );
        }
    }

    /// Writes data into an audio channel buffer. audio channel identified by *name* with data from slice *input* which should
    /// contain at least ksmps f64 samples, if not, this method will panic.
    /// # Arguments
    /// * `input` The slice with data to be copied into the audio channel buffer. Could contain up to ksmps samples.
    /// # panic
    /// This method will panic if input.len() > ksmps.
    pub fn write_audio_channel(&mut self, name: &str, input: &[f64]) {
        let size = self.get_ksmps() as usize * self.get_channels(1) as usize;
        let len = input.len();
        let cname = CString::new(name).unwrap();
        assert!(
            len <= size,
            "The audio channel's capacity is {} so, it isn't possible to copy {} bytes",
            size,
            len
        );
        unsafe {
            csound_sys::csoundSetAudioChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                input.as_ptr() as *mut c_double,
            );
        }
    }

    /// Returns the content of the string channel identified by *name*
    pub fn get_string_channel(&self, name: &str) -> String {
        let cname = CString::new(name).unwrap();
        let mut data = String::with_capacity(self.get_channel_data_size(name));
        unsafe {
            let ptr = data.as_mut_vec();
            csound_sys::csoundGetStringChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                ptr.as_ptr() as *mut _,
            );
        }
        data
    }

    /// Sets the string channel identified by *name* with *content*
    pub fn set_string_channel(&mut self, name: &str, content: &str) {
        let cname = CString::new(name).unwrap();
        let content = CString::new(content).unwrap();
        unsafe {
            csound_sys::csoundSetStringChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                content.as_ptr() as *mut _,
            );
        }
    }

    /// returns the size of data stored in the channel identified by *name*
    pub fn get_channel_data_size(&self, name: &str) -> usize {
        let cname = CString::new(name).unwrap();
        unsafe { csound_sys::csoundGetChannelDatasize(self.csound_ptr(), cname.as_ptr()) as usize }
    }

    /// Send a event.
    /// # Arguments
    /// * `event_type` is the event type from CS_INSERT_EVENT = 0, CS_TABLE_EVENT = 1, CS_END_EVENT = 2 (old values in order were 'i', 'f', 'e')
    /// * `pfields` is a slice of f64 values with all the pfields for this event.
    /// # Example
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// let pFields = [1.0, 1.0, 5.0];
    /// while cs.perform_ksmps() == false {
    ///     cs.send_sound_event(0, &pFields, 0);
    /// }
    /// ```
    pub fn send_sound_event(&self, event_type: i32, pfields: &[f64], async_: i32) {
        unsafe {
            csound_sys::csoundEvent(
                self.csound_ptr(),
                event_type,
                pfields.as_ptr() as *mut c_double,
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

    /// Returns the length of a function table (not including the guard point), or an error
    /// message if the table doens't exist.
    /// # Arguments
    /// * `table` The function table identifier.
    pub fn table_length(&self, table: u32) -> Result<usize> {
        unsafe {
            let value = csound_sys::csoundTableLength(self.csound_ptr(), table as c_int) as i32;
            if value > 0 {
                Ok(value as usize)
            } else {
                Err(Error::NotFound("table does not exist"))
            }
        }
    }

    /// Returns a [`Csound::Table`](struct.Table.html).
    /// which could be used to read/write the table content
    /// directly( not using [`Csound:: table_copy_in`](struct.Csound.html#method.table_copy_in) or [`Csound::table_copy_out`](struct.Csound.html#method.table_copy_out)).
    /// this table will be valid along the csound instance. Returns None if the table doesn't
    /// exist.
    /// # Arguments
    /// * `table` The function table identifier.
    /// # Example
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_csd("some.csd", 0, 0);
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table_buff = vec![0f64; cs.table_length(1).unwrap() as usize];
    ///     // Gets the function table 1
    ///     let mut table = cs.get_table(1).unwrap();
    ///     // Copies the table content into table_buff
    ///     // table.read( table_buff.as_mut_slice() ).unwrap();
    ///     // Do some stuffs
    ///     // table.write(&table_buff.into_iter().map(|x| x*2.5).collect::<Vec<f64>>().as_mut_slice());
    ///     // Do some stuffs
    /// }
    /// ```
    /// see [`Table::read`](struct.Table.html#method.read) or [`Table::write`](struct.Table.html#method.write).
    pub fn get_table(&self, table: u32) -> Option<Table<'_>> {
        let mut ptr = ptr::null_mut() as *mut c_double;
        let length;
        unsafe {
            length = csound_sys::csoundGetTable(
                self.csound_ptr(),
                &mut ptr as *mut *mut c_double,
                table as c_int,
            ) as i32;
        }
        match length {
            -1 => None,
            _ => Some(Table {
                ptr,
                length: length as usize,
                phantom: PhantomData,
            }),
        }
    }

    /// Gets the arguments used to construct or define a function table
    /// # Arguments
    /// * `table` The function table identifier.
    /// # Returns
    /// A vector containing the table's arguments.
    /// * Note:* the argument list starts with the GEN number and is followed by its parameters.
    /// eg. f 1 0 1024 10 1 0.5 yields the list {10.0,1.0,0.5}.
    pub fn get_table_args(&self, table: u32) -> Option<Vec<f64>> {
        let mut ptr = ptr::null_mut() as *mut c_double;
        unsafe {
            let length = csound_sys::csoundGetTableArgs(
                self.csound_ptr(),
                &mut ptr as *mut *mut c_double,
                table as c_int,
            );
            if length < 0 {
                None
            } else {
                let mut result = Vec::with_capacity(length as usize);
                for pos in 0..length as isize {
                    result.push(*ptr.offset(pos));
                }
                Some(result)
            }
        }
    }

    /// Gets the arguments used to construct or define a function table
    /// Similar to [`Csound::get_table_args`](struct.Csound.html#method.get_table_args)
    /// but no memory will be allocated, instead a slice is returned.
    pub fn get_table_args_slice(&self, table: u32) -> Option<&[f64]> {
        let mut ptr = ptr::null_mut() as *mut c_double;
        unsafe {
            let length = csound_sys::csoundGetTableArgs(
                self.csound_ptr(),
                &mut ptr as *mut *mut c_double,
                table as c_int,
            );
            if length < 0 {
                None
            } else {
                Some(slice::from_raw_parts(ptr as *const _, length as usize))
            }
        }
    }

    /* Engine general Opcode function  implementations **************************************************************************************** */

    /// Gets an alphabetically sorted list of all opcodes.
    /// Should be called after externals are loaded by csoundCompile().
    /// The opcode information is contained in a [`Csound::OpcodeListEntry`](struct.Csound.html#struct.OpcodeListEntry)
    pub fn get_opcode_list_entry(&self) -> Option<Vec<OpcodeListEntry>> {
        let mut ptr: *mut csound_sys::opcodeListEntry = ptr::null_mut();
        let length;
        unsafe {
            length = csound_sys::csoundNewOpcodeList(
                self.csound_ptr(),
                &mut ptr as *mut *mut csound_sys::opcodeListEntry,
            );
        }
        if length < 0 {
            None
        } else {
            let mut result: Vec<OpcodeListEntry> = Vec::with_capacity(length as usize);
            for pos in 0..length as isize {
                unsafe {
                    let opname = Trampoline::ptr_to_string((*ptr.offset(pos)).opname);
                    let outypes = Trampoline::ptr_to_string((*ptr.offset(pos)).outypes);
                    let intypes = Trampoline::ptr_to_string((*ptr.offset(pos)).intypes);
                    let flags = (*ptr.offset(pos)).flags as i32;
                    result.push(OpcodeListEntry {
                        opname,
                        outypes,
                        intypes,
                        flags,
                    });
                }
            }
            unsafe {
                csound_sys::csoundDisposeOpcodeList(self.csound_ptr(), ptr);
                Some(result)
            }
        }
    }

    /**
    TODO genName and appendOpcode functions
    *****/

    /* Engine miscellaneous functions **************************************************************************************** */

    /// # Argument
    /// * `lang_code` can be for example any of [`Language`](enum.Language.html) variants.
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
    /// A CircularBuffer
    /// # Example
    /// ```
    /// use csound::Csound;
    ///
    /// let csound = Csound::new().unwrap();
    /// let circular_buffer = csound.create_circular_buffer::<f64>(1024);
    /// ```
    pub fn create_circular_buffer<'a, T: 'a + Copy>(&'a self, len: u32) -> CircularBuffer<'a, T> {
        unsafe {
            let ptr: *mut T = csound_sys::csoundCreateCircularBuffer(
                self.csound_ptr(),
                len as c_int,
                mem::size_of::<T>() as c_int,
            ) as *mut T;
            CircularBuffer {
                csound: self.csound_ptr(),
                ptr,
                phantom: PhantomData,
            }
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
    /// # Arguments
    /// * `user_func` A function/closure which will receive a reference
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
        F: FnMut(&[f64]) + 'c,
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
        F: FnMut(&mut [f64]) -> usize + 'c,
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
    /// # Arguments
    /// * ´f´ Function which implement the FnMut trait.
    /// The callback arguments are *u32* which indicates the message atributte,
    /// and a reference to the message content.
    /// # Example
    /// ```
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
    /// # Arguments
    /// * ´f´ Function which implement the FnMut trait. The invalue opcode will trigger this callback passing
    /// the channel name which requiere the data. This function/closure have to return the data which will be
    /// passed to that specific channel if not only return ChannelData::Unknown. Only *String* and *control* Channels
    /// are supported.
    /// # Example
    /// ```
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
    /// # Arguments
    /// * ´f´ Function which implement the FnMut trait. The outvalue opcode will trigger this callback passing
    /// the channel ##name and the channel's output data encoded in the ChannelData. Only *String* and *control* Channels
    /// are supported.
    /// # Example
    /// ```
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

    /// Called by external software to set a function for checking system events, yielding cpu time for coopertative multitasking, etc
    /// This function is optional. It is often used as a way to 'turn off' Csound, allowing it to exit gracefully.
    /// In addition, some operations like utility analysis routines are not reentrant
    /// and you should use this function to do any kind of updating during the operation.
    /// # Returns
    /// If this callback returns *false* it wont be called anymore
    pub fn yield_callback<'c, F>(&self, f: F)
    where
        F: FnMut() -> bool + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.csound_ptr()) as *mut CallbackHandler))
                .callbacks
                .set_yield_cb(self.csound_ptr(), f);
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
            return Err(Error::InsufficientCapacity);
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
            return Err(Error::InsufficientCapacity);
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
            return Err(Error::InsufficientCapacity);
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

/// Csound table representation.
/// This struct is build up to manipulate directly a csound's table.
#[derive(Debug)]
pub struct Table<'a> {
    ptr: *mut f64,
    length: usize,
    phantom: PhantomData<&'a f64>,
}

impl<'a> Table<'a> {
    /// # Returns
    /// The table length
    pub fn get_size(&self) -> usize {
        self.length
    }

    /// # Returns
    /// A slice representation with the table's internal data
    pub fn as_slice(&self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.ptr, self.length) }
    }

    /// # Returns
    /// A mutable slice representation with the table's internal data
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.length) }
    }

    /// method used to copy data from the table internal buffer
    /// into an user buffer. A error message is returned if the Table is not longer valid.
    /// # Arguments
    /// * `slice` A slice where out.len() elements from the table will be copied.
    /// # Returns
    /// The number of elements copied into the output slice.
    /// # Example
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_csd("some.csd", 0, 0);
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table = cs.get_table(1).unwrap();
    ///     let mut table_buff = vec![0f64; table.len()];
    ///     // copy Table::length elements from the table's internal buffer
    ///     table.copy_to_slice( table_buff.as_mut_slice() );
    ///     // Do some stuffs
    /// }
    /// ```
    pub fn copy_to_slice(&self, slice: &mut [f64]) -> usize {
        let mut len = slice.len();
        let size = self.get_size();
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(self.ptr, slice.as_mut_ptr(), len);
            len
        }
    }

    /// method used to copy data into the table internal buffer
    /// from an user slice.
    /// # Arguments
    /// * `slice` A slice where input.len() elements will be copied.
    /// # Returns
    /// The number of elements copied into the table
    /// # Example
    /// ```no_run
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_csd("some.csd", 0, 0);
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table = cs.get_table(1).unwrap();
    ///     let mut table_buff = vec![0f64; table.len()];
    ///     // copy Table::length elements from the table's internal buffer
    ///     // table.read( table_buff.as_mut_slice() ).unwrap();
    ///     // Do some stuffs
    ///     table.copy_from_slice(&table_buff.into_iter().map(|x| x*2.5).collect::<Vec<f64>>().as_mut_slice());
    ///     // Do some stuffs
    /// }
    /// ```
    pub fn copy_from_slice(&self, slice: &[f64]) -> usize {
        let mut len = slice.len();
        let size = self.get_size();
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(slice.as_ptr(), self.ptr, len);
            len
        }
    }
}

impl<'a> AsRef<[f64]> for Table<'a> {
    fn as_ref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> AsMut<[f64]> for Table<'a> {
    fn as_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}

impl<'a> Deref for Table<'a> {
    type Target = [f64];
    fn deref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> DerefMut for Table<'a> {
    fn deref_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}

pub enum Readable {}
pub enum Writable {}

/// Csound buffer pointer representation.
/// This struct is build up to manipulate directly csound's buffers.
pub struct BufferPtr<'a, T> {
    ptr: *mut f64,
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
    pub fn copy_to_slice(&self, slice: &mut [f64]) -> usize {
        let mut len = slice.len();
        let size = self.get_size();
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(self.ptr, slice.as_mut_ptr(), len);
            len
        }
    }

    /// # Returns
    /// A slice to the buffer internal data
    pub fn as_slice(&self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<'a> BufferPtr<'a, Writable> {
    /// # Returns
    /// This buffer pointer as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    /// method used to copy data into this buffer
    /// # Arguments
    /// * `slice` A slice with samples to copy
    /// # Returns
    /// The number of elements copied into the csound's buffer.
    pub fn copy_from_slice(&self, slice: &[f64]) -> usize {
        let mut len = slice.len();
        let size = self.get_size();
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(slice.as_ptr(), self.ptr, len);
            len
        }
    }

    /// method used to clear the buffer's data
    pub fn clear(&mut self) {
        for s in self.as_mut_slice() {
            *s = 0f64;
        }
    }
}

impl<'a, T> AsRef<[f64]> for BufferPtr<'a, T> {
    fn as_ref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> AsMut<[f64]> for BufferPtr<'a, Writable> {
    fn as_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}

impl<'a, T> Deref for BufferPtr<'a, T> {
    type Target = [f64];
    fn deref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> DerefMut for BufferPtr<'a, Writable> {
    fn deref_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}
