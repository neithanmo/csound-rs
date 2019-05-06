#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

use std::io;
use std::marker::PhantomData;
use std::mem;

use std::cell::RefCell;

use std::ops::{Deref, DerefMut};
use std::ptr;
use std::slice;

use callbacks::*;
use channels::{ChannelBehavior, ChannelHints, ChannelInfo, PvsDataExt};
use csound_sys;

use csound_sys::RTCLOCK;
use enums::{ChannelData, ControlChannelType, Language, MessageType, Status};
use rtaudio::{CsAudioDevice, CsMidiDevice, RtAudioParams};

use std::ffi::{CStr, CString, NulError};
use std::str;
use std::str::Utf8Error;

use libc::{c_char, c_double, c_int, c_long, c_void};

// the length in bytes of the output type name in csound
const OUTPUT_TYPE_LENGTH: usize = 6;

// The length in bytes of the output format name in csound
const OUTPUT_FORMAT_LENGTH: usize = 8;

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
    engine: Inner,
}

/// Opaque struct representing a csound object
#[derive(Debug)]
pub(crate) struct Inner {
    csound: *mut csound_sys::CSOUND,
    use_msg_buffer: RefCell<bool>,
}

unsafe impl Send for Inner {}

impl Default for Csound {
    fn default() -> Self {
        unsafe {
            // Csound must not handle signals
            csound_sys::csoundInitialize(csound_sys::CSOUNDINIT_NO_SIGNAL_HANDLER as c_int);
            csound_sys::csoundInitialize(csound_sys::CSOUNDINIT_NO_ATEXIT as c_int);

            let callback_handler = Box::new(CallbackHandler {
                callbacks: Callbacks::default(),
            });
            let host_data_ptr = Box::into_raw(callback_handler) as *mut c_void;

            let csound_sys = csound_sys::csoundCreate(host_data_ptr);
            assert!(!csound_sys.is_null());

            let engine = Inner {
                csound: csound_sys,
                use_msg_buffer: RefCell::new(false),
            };
            Csound { engine }
        }
    }
}

impl Csound {
    /// Create a new csound object.
    ///
    /// This is the core of almost all operations in the csound library.
    /// A new instance of csound will created by this function, a custom callback handler will be used,
    /// This custom callback handler will be active only if the user calls some of the
    /// callbacks setting functions which receive a closure for a specific callback.
    ///
    /// # Example
    ///
    /// ```
    ///  // Creates a Csound instance and use a custom callback handler
    /// let csound = Csound::new();
    /// // enable the message callback passing a closure to the custom callback handler
    /// csound.message_string_callback( |mtype:u32, message:&str| {
    ///     println!("message type: {} message content:  {}", mtype, message);
    /// });
    /// csound.compile_csd(csd_filename).unwrap();
    /// csound.start();
    /// ```
    pub fn new<'a>() -> Csound {
        Csound::default()
    }

    /// Initializes the csound library with specific flags(see: [anchor text]()).
    /// This function is called internally by Csound::new(), so there is generally no need to use it explicitly unless
    /// you need to avoid default initilization that sets signal handlers and atexit() callbacks.
    /// Return value is Ok() on success or an error message in case of failure
    pub fn initialize(flags: i32) -> Result<(), &'static str> {
        unsafe {
            match csound_sys::csoundInitialize(flags as c_int) as i32 {
                csound_sys::CSOUND_ERROR => Err("Can't to initialize csound "),
                csound_sys::CSOUND_SUCCESS => Ok(()),
                value => {
                    if value > 0 {
                        Err("Initialization was done already")
                    } else {
                        Err("Unknown error - can to initialize")
                    }
                }
            }
        }
    }

    /// Sets a single csound option(flag).
    ///
    /// NB: blank spaces are not allowed.
    /// # Returns
    /// returns Ok on success or a error message in case the option is invalid.
    pub fn set_option(&self, options: &str) -> Result<(), &'static str> {
        let op = CString::new(options).map_err(|_| "Error parsing the string")?;
        unsafe {
            match csound_sys::csoundSetOption(self.engine.csound, op.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Options not valid"),
            }
        }
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
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd(csd_filename).unwrap();
    /// csound.start();
    /// ...
    /// ```
    ///
    pub fn start(&self) -> Result<(), &'static str> {
        unsafe {
            if csound_sys::csoundStart(self.engine.csound) == csound_sys::CSOUND_SUCCESS {
                Ok(())
            } else {
                Err("Csound is already started, call csoundReset() before starting again.")
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
        unsafe { csound_sys::csoundGetAPIVersion() as u32 }
    }

    /* Engine performance functions implementations ********************************************************* */

    /// Stops the performance of a csound's instance
    /// *Note*: It is not guaranteed that [`Csound::perform`](struct.Csound.html#method.perform) has already stopped when this function returns.
    pub fn stop(&self) {
        unsafe {
            csound_sys::csoundStop(self.engine.csound);
        }
    }

    /// Resets all internal memory and state in preparation for a new performance.
    /// Enables external software to run successive Csound performances without reloading Csound.
    pub fn reset(&self) {
        unsafe {
            csound_sys::csoundReset(self.engine.csound);
        }
    }

    /// Compiles Csound input files (such as an orchestra and score, or CSD) as directed by the supplied command-line arguments , but does not perform them.
    /// This function cannot be called during performance, and before a repeated call, csoundReset() needs to be called.
    /// # Arguments
    /// * `args` A slice containing the arguments  to be passed to csound
    /// # Returns
    /// A error message in case of failure
    pub fn compile<T>(&self, args: &[T]) -> Result<(), &'static str>
    where
        T: AsRef<str>,
    {
        if args.is_empty() {
            return Err("Not enough arguments");
        }

        let arguments: Vec<CString> = args
            .iter()
            .map(|arg| CString::new(arg.as_ref()).unwrap())
            .collect();
        let args_raw: Vec<*const c_char> = arguments.iter().map(|arg| arg.as_ptr()).collect();
        let argv: *const *const c_char = args_raw.as_ptr();
        unsafe {
            match csound_sys::csoundCompile(self.engine.csound, args_raw.len() as c_int, argv) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't compile carguments"),
            }
        }
    }

    /// Compiles a Csound input file (CSD, .csd file), but does not perform it.
    /// If [`Csound::start`](struct.Csound.html#method.start) is called before `compile_csd`, the <CsOptions> element is ignored
    /// (but set_option can be called any number of times),
    /// the <CsScore> element is not pre-processed, but dispatched as real-time events;
    /// and performance continues indefinitely, or until ended by calling [`Csound::stop`](struct.Csound.html#method.stop) or some other logic.
    /// In this "real-time" mode, the sequence of calls should be:
    /// ```
    /// let csound  = Csound::new();
    /// csound.set_option("-an_option");
    /// csound.set_option("-another_option");
    /// csound.start();
    /// csound.compile_csd(csd_filename);
    /// while true{
    ///     // Send realtime events
    ///     csound.send_score_event("i 1 0 5 4.5 6.2");
    ///     //...
    ///     // some logic to break the loop after a performance of realtime events
    /// }
    /// ```
    /// *Note*: this function can be called repeatedly during performance to replace or add new instruments and events.
    /// But if csoundCompileCsd is called before csoundStart, the <CsOptions> element is used,the <CsScore> section is pre-processed and dispatched normally,
    /// and performance terminates when the score terminates, or [`Csound::stop`](struct.Csound.html#method.stop)  is called.
    ///  In this "non-real-time" mode (which can still output real-time audio and handle real-time events), the sequence of calls should be:
    ///  ```
    ///  let csound  = Csound::new();
    ///  csound.compile_csd(csd_filename);
    ///  csound.start();
    ///  while !csound.perform_ksmps() {
    ///  }
    ///  ```
    /// # Arguments
    /// * `csd` A reference to .csd file name
    pub fn compile_csd<T>(&self, csd: T) -> Result<(), &'static str>
    where
        T: AsRef<str>,
    {
        let csd = csd.as_ref();
        if csd.is_empty() {
            return Err("Empty file name");
        }
        let path = CString::new(csd).map_err(|_| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileCsd(self.engine.csound, path.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't compile the csd file"),
            }
        }
    }

    /// Behaves the same way as [`Csound::compile_csd`](struct.Csound.html#method.compile_csd),
    /// except that the content of the CSD is read from a string rather than from a file.
    /// This is convenient when it is desirable to package the csd as part of an application or a multi-language piece.
    /// # Arguments
    /// * `csd_text` A reference to the text to be compiled by csound
    pub fn compile_csd_text<T>(&self, csdText: T) -> Result<(), &'static str>
    where
        T: AsRef<str>,
    {
        let csdText = csdText.as_ref();
        if csdText.is_empty() {
            return Err("Empty file name");
        }
        let path = CString::new(csdText).map_err(|_e| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileCsdText(self.engine.csound, path.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't compile the csd file"),
            }
        }
    }

    /// Parses and compiles the given orchestra from an ASCII string, also evaluating any global space code (i-time only)
    /// this can be called during performance to compile a new orchestra.
    /// ```
    /// let csound  = Csound::new();
    /// let orc_code = "instr 1
    ///                 a1 rand 0dbfs/4
    ///                 out a1";
    /// csound.compile_orc(orc_code);
    /// ```
    /// # Arguments
    /// * `orcPath` A reference to orchestra strings
    pub fn compile_orc<T>(&self, orc: T) -> Result<(), &'static str>
    where
        T: AsRef<str>,
    {
        let orc = orc.as_ref();
        if orc.is_empty() {
            return Err("Empty file name");
        }
        let orc = CString::new(orc).map_err(|_e| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileOrc(self.engine.csound, orc.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't to compile orc file"),
            }
        }
    }

    /// Async version of [`Csound::compile_orc`](struct.Csound.html#method.compile_orc). The code is parsed and compiled,
    /// then placed on a queue for asynchronous merge into the running engine, and evaluation.
    /// The function returns following parsing and compilation.
    /// # Arguments
    /// * `orc` A reference to an csound's orchestra definitions
    pub fn compile_orc_async<T>(&self, orc: T) -> Result<(), &'static str>
    where
        T: AsRef<str>,
    {
        let orcPath = orc.as_ref();
        if orcPath.is_empty() {
            return Err("Empty file name");
        }
        let path = CString::new(orcPath).map_err(|_e| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileOrcAsync(self.engine.csound, path.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't to compile orc file"),
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
    pub fn eval_code<T>(&self, code: T) -> f64
    where
        T: AsRef<str>,
    {
        let cd = code.as_ref();
        let cd = CString::new(cd).unwrap();
        unsafe { csound_sys::csoundEvalCode(self.engine.csound, cd.as_ptr() as _) }
    }

    // TODO Imlement csoundCompileTree functions

    /// Senses input events and performs audio output.
    ///
    ///  perform until: 1. the end of score is reached (positive return value), 2. an error occurs (negative return value),
    ///  or 3. performance is stopped by calling *stop()* from another thread (zero return value).
    ///  Note that some csf file, text or score have to be compiled first and then *start()* must be called.
    ///  In the case of zero return value, *perform()* can be called again to continue the stopped performance.
    ///  Otherwise, [`Csound::reset`](struct.Csound.html#method.reset) should be called to clean up after the finished or failed performance.
    pub fn perform(&self) -> i32 {
        unsafe { csound_sys::csoundPerform(self.engine.csound) as i32 }
    }

    /// Senses input events, and performs one control sample worth ```ksmps * number of channels * size_off::<f64> bytes``` of audio output.
    ///
    /// Note that some csd file, text or score have to be compiled first and then [`Csound::start`](struct.Csound.html#method.start).
    /// Enables external software to control the execution of Csound, and to synchronize
    /// performance with audio input and output(see: [`Csound::read_spin_buffer`](struct.Csound.html#method.read_spin_buffer), [`Csound::read_spout_buffer`](struct.Csound.html#method.read_spout_buffer))
    /// # Returns
    /// *false* during performance, and true when performance is finished. If called until it returns *true*, will perform an entire score.
    pub fn perform_ksmps(&self) -> bool {
        unsafe { csound_sys::csoundPerformKsmps(self.engine.csound) != 0 }
    }

    /// Performs Csound, sensing real-time and score events and processing one buffer's worth (-b frames) of interleaved audio.
    /// Note that some csd file, text or score have to be compiled first and then [`Csound::start`](struct.Csound.html#method.start),
    /// you could call [`Csound::read_output_buffer`](struct.Csound.html#method.start) or
    /// [`Csound::write_input_buffer`](struct.Csound.html#method.write_input_buffer) to write/read the csound's I/O buffers content.
    /// #Returns
    /// *false* during performance or *true* when performance is finished.
    pub fn perform_buffer(&self) -> bool {
        unsafe { csound_sys::csoundPerformBuffer(self.engine.csound) != 0 }
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
            match Status::from(
                csound_sys::csoundUDPServerStart(self.engine.csound, port as c_int) as i32,
            ) {
                Status::CS_SUCCESS => Ok(()),
                status => Err(status),
            }
        }
    }

    /// # Returns
    /// The port number on which the server is running, or None if the server has not been started.
    pub fn udp_server_status(&self) -> Option<u32> {
        unsafe {
            let status = csound_sys::csoundUDPServerStatus(self.engine.csound);
            if status == csound_sys::CSOUND_ERROR {
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
            match Status::from(csound_sys::csoundUDPServerClose(self.engine.csound) as i32) {
                Status::CS_SUCCESS => Ok(()),
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
            let ip = CString::new(addr).map_err(|_e| Status::CS_ERROR)?;
            if csound_sys::csoundUDPConsole(
                self.engine.csound,
                ip.as_ptr(),
                port as c_int,
                mirror as c_int,
            ) == csound_sys::CSOUND_SUCCESS
            {
                return Ok(());
            }
            Err(Status::CS_ERROR)
        }
    }

    /// Stop transmitting console messages via UDP
    pub fn udp_stop_console(&self) {
        unsafe {
            csound_sys::csoundStopUDPConsole(self.engine.csound);
        }
    }
    /* Engine Attributes functions implmentations ********************************************************* */

    /// # Returns
    /// The number of audio sample frames per second.
    pub fn get_sample_rate(&self) -> f64 {
        unsafe { csound_sys::csoundGetSr(self.engine.csound) as f64 }
    }

    /// # Returns
    /// The number of control samples per second.
    pub fn get_control_rate(&self) -> f64 {
        unsafe { csound_sys::csoundGetKr(self.engine.csound) as f64 }
    }

    /// # Returns
    /// The number of audio sample frames per control sample.
    pub fn get_ksmps(&self) -> u32 {
        unsafe { csound_sys::csoundGetKsmps(self.engine.csound) }
    }

    /// # Returns
    /// The number of audio output channels. Set through the nchnls header variable in the csd file.
    pub fn output_channels(&self) -> u32 {
        unsafe { csound_sys::csoundGetNchnls(self.engine.csound) as u32 }
    }

    /// # Returns
    /// The number of audio input channels.
    /// Set through the **nchnls_i** header variable in the csd file.
    /// If this variable is not set, the value is taken from nchnls.
    pub fn input_channels(&self) -> u32 {
        unsafe { csound_sys::csoundGetNchnlsInput(self.engine.csound) as u32 }
    }
    /// # Returns
    /// The 0dBFS level of the spin/spout buffers.
    pub fn get_0dBFS(&self) -> f64 {
        unsafe { csound_sys::csoundGet0dBFS(self.engine.csound) as f64 }
    }

    /// # Returns
    /// The A4 frequency reference
    pub fn get_freq(&self) -> f64 {
        unsafe { csound_sys::csoundGetA4(self.engine.csound) as f64 }
    }

    /// #Returns
    /// The current performance time in samples
    pub fn get_current_sample_time(&self) -> usize {
        unsafe { csound_sys::csoundGetCurrentTimeSamples(self.engine.csound) as usize }
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
        unsafe { csound_sys::csoundGetDebug(self.engine.csound) as u32 }
    }

    /// Sets whether Csound prints debug messages from the *DebugMsg()* csouns's internal API function.
    /// # Arguments
    /// * `level` The debug level to assign, anything different to 0 means true.
    pub fn set_debug_level(&self, level: i32) {
        unsafe {
            csound_sys::csoundSetDebug(self.engine.csound, level as c_int);
        }
    }

    /* Engine general InputOutput functions implmentations ********************************************************* */

    /// Gets the csound's input source name if it has been defined
    /// otherwise, None is returned
    pub fn get_input_name(&self) -> Option<String> {
        unsafe {
            let ptr = csound_sys::csoundGetInputName(self.engine.csound);
            Trampoline::ptr_to_string(ptr)
        }
    }

    /// Gets output device name if the realtime output has been defined,
    /// Otherwise, None is returned
    pub fn get_output_name(&self) -> Option<String> {
        unsafe {
            let ptr = csound_sys::csoundGetOutputName(self.engine.csound);
            Trampoline::ptr_to_string(ptr)
        }
    }

    /// Set output destination, type and format
    /// # Arguments
    /// * `name` The destination/device name, for RT audio use the field [`CsAudioDevice::device_id`](struct.CsAudioDevice.html#field.device_id).
    ///  (see: [`Csound::get_audio_devices`](struct.Csound.html#method.get_audio_devices))
    /// * `out_type`  can be one of "wav","aiff", "au","raw", "paf", "svx", "nist", "voc", "ircam","w64","mat4", "mat5", "pvf","xi", "htk","sds","avr",
    /// "wavex","sd2", "flac", "caf","wve","ogg","mpc2k","rf64", or NULL (use default or realtime IO).
    /// * `format` can be one of "alaw", "schar", "uchar", "float", "double", "long", "short", "ulaw", "24bit", "vorbis", or NULL (use default or realtime IO).
    pub fn set_output(&self, name: &str, out_type: &str, format: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            let devType = CString::new(out_type)?;
            let devFormat = CString::new(format)?;

            csound_sys::csoundSetOutput(
                self.engine.csound,
                devName.as_ptr(),
                devType.as_ptr(),
                devFormat.as_ptr(),
            );
            Ok(())
        }
    }

    /// Get output type and format.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// let (output_type, output_format) = csound.get_output_format().unwrap();
    /// ```
    pub fn get_output_format(&self) -> Result<(String, String), Utf8Error> {
        let otype = vec![b'\0'; OUTPUT_TYPE_LENGTH];
        let format = vec![b'\0'; OUTPUT_FORMAT_LENGTH];
        unsafe {
            let otype = CString::from_vec_unchecked(otype).into_raw();
            let format = CString::from_vec_unchecked(format).into_raw();

            csound_sys::csoundGetOutputFormat(self.engine.csound, otype, format);

            let otype = CString::from_raw(otype);
            let otype = otype.to_str()?;
            let format = CString::from_raw(format);
            let format = format.to_str()?;

            Ok((otype.into(), format.into()))
        }
    }

    /// Sets input source
    /// # Arguments
    /// * `name` The source device name.
    pub fn set_input(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetInput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    /// Set MIDI file input name
    pub fn set_midi_file_input(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIFileInput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    /// Set MIDI file output name
    pub fn set_midi_file_output(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIFileOutput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    /// Set MIDI input device name/number
    pub fn set_midi_input(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIInput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    /// Set MIDI output device name
    pub fn set_midi_output(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIOutput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    /* Engine general Realtime Audio I/O functions implmentations ********************************************************* */

    /// Sets the current RT audio module
    pub fn set_rt_audio_module(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetRTAudioModule(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    /// # Returns
    /// The number of samples in Csound's input buffer.
    pub fn get_input_buffer_size(&self) -> usize {
        unsafe { csound_sys::csoundGetInputBufferSize(self.engine.csound) as usize }
    }

    /// # Returns
    /// The number of samples in Csound's input buffer.
    pub fn get_output_buffer_size(&self) -> usize {
        unsafe { csound_sys::csoundGetOutputBufferSize(self.engine.csound) as usize }
    }

    /// Gets the csound's input buffer.
    /// # Returns
    /// An Option containing either the [`BufferPtr`](struct.BufferPtr.html) or None if the
    /// csound's input buffer has not been initialized. The returned *BufferPtr* is Writable, it means that you can modify
    /// the csound's buffer content in order to write external audio data into csound and process it.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let input_buffer_ptr = csound.get_input_buffer();
    /// while !csound.perform_buffer() {
    ///     // fills your buffer with audio samples that you want to pass into csound
    ///     foo_fill_buffer(input_buffer_ptr.as_mut_slice());
    ///     // ...
    /// }
    /// ```
    pub fn get_input_buffer(&self) -> Option<BufferPtr<Writable>> {
        unsafe {
            let ptr = csound_sys::csoundGetInputBuffer(self.engine.csound) as *mut f64;
            let len = self.get_input_buffer_size();
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

    /// Gets the csound's output buffer.
    /// # Returns
    /// An Option containing either the [`BufferPtr`](struct.BufferPtr.html) or None if the
    /// csound's output buffer has not been initialized. The returned *BufferPtr* is only Readable.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let output_buffer_ptr = csound.get_output_buffer();
    /// let mut data = vec![0f64; input_buffer_ptr.get_size()];
    /// while !csound.perform_buffer() {
    ///     // process the data from csound
    ///     foo_process_buffer(output_buffer_ptr.as_slice());
    /// }
    /// ```
    pub fn get_output_buffer(&self) -> Option<BufferPtr<Readable>> {
        unsafe {
            let ptr = csound_sys::csoundGetOutputBuffer(self.engine.csound) as *mut f64;
            let len = self.get_output_buffer_size();
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

    /// Enables external software to write audio into Csound before calling perform_ksmps.
    /// # Returns
    /// An Option containing either the [`BufferPtr`](struct.BufferPtr.html) or None if the
    /// csound's spin buffer has not been initialized. The returned *BufferPtr* is Writable.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let spin = csound.get_spin();
    /// while !csound.perform_ksmps() {
    ///     // fills the spin buffer with audio samples that you want to pass into csound
    ///     foo_fill_buffer(spin.as_mut_slice());
    ///     // ...
    /// }
    /// ```
    pub fn get_spin(&self) -> Option<BufferPtr<Writable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpin(self.engine.csound) as *mut f64;
            let len = (self.get_ksmps() * self.input_channels()) as usize;
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
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let spout = csound.get_spout();
    /// while !csound.perform_ksmps() {
    ///     // Deref the spout pointer and read its content
    ///     foo_read_buffer(&*spout);
    ///     // ...
    /// }
    /// ```
    pub fn get_spout(&self) -> Option<BufferPtr<Readable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpout(self.engine.csound) as *mut f64;
            let len = (self.get_ksmps() * self.output_channels()) as usize;
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

    /// Method used when you want to copy audio samples from the csound's output buffer.
    /// # Arguments
    /// * `out` a reference to a mutable slice where the Csound's output buffer content
    /// will be copied.  This buffer have to has enough memory for at least
    /// [`Csound::get_output_buffer_size`](struct.Csound.html#method.get_output_buffer_size), samples.
    /// # Returns
    /// The number of samples copied into the slice on success, or an
    /// error message if the internal csound's buffer has not been initialized.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let output_buffer_length = csound.get_output_buffer_size();
    /// let mut output_buffer = vec![0f64; output_buffer_length];
    /// while !csound.perform_buffer() {
    ///     csound.read_output_buffer(&mut output_buffer).unwrap();
    ///     // ... do some stuff with the buffer
    /// }
    /// ```
    /// # Deprecated
    /// Use [`Csound::get_output_buffer`](struct.Csound.html#method.get_output_buffer) to get a [`BufferPtr`](struct.BufferPtr.html)
    /// object.
    pub fn read_output_buffer(&self, output: &mut [f64]) -> Result<usize, &'static str> {
        let size = self.get_output_buffer_size();
        let obuffer =
            unsafe { csound_sys::csoundGetOutputBuffer(self.engine.csound) as *const f64 };
        let mut len = output.len();
        if size < len {
            len = size;
        }
        if !obuffer.is_null() {
            unsafe {
                std::ptr::copy(obuffer, output.as_ptr() as *mut f64, len);
                return Ok(len);
            }
        }
        Err("The output buffer is not initialized, call the 'compile()' and 'start()' methods.")
    }

    /// Method used when you want to copy custom audio samples into the csound buffer to be processed.
    /// # Arguments
    /// * `input` a reference to a slice with samples which will be copied to
    /// the Csound's input buffer.
    /// # Returns
    /// The number of samples copied into the csound's input buffer or an
    /// error message if the internal csound's buffer has not been initialized.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let input_buffer_length = csound.get_input_buffer_size();
    /// let mut input_buffer = vec![0f64; output_buffer_length];
    /// while !csound.perform_buffer() {
    ///     // fills your buffer with audio samples you want to pass into csound
    ///     foo_fill_buffer(&mut input_buffer);
    ///     csound.write_input_buffer(&input_buffer);
    ///     // ...
    /// }
    /// ```
    /// # Deprecated
    /// Use [`Csound::get_input_buffer`](struct.Csound.html#method.get_input_buffer) to get a [`BufferPtr`](struct.BufferPtr.html)
    /// object.
    pub fn write_input_buffer(&self, input: &[f64]) -> Result<usize, &'static str> {
        let size = self.get_input_buffer_size();
        let ibuffer = unsafe { csound_sys::csoundGetInputBuffer(self.engine.csound) as *mut f64 };
        let mut len = input.len();
        if size < len {
            len = size;
        }
        if !ibuffer.is_null() {
            unsafe {
                std::ptr::copy(input.as_ptr(), ibuffer, len);
                return Ok(len);
            }
        }
        Err("The input buffer is not initialized, call the 'compile()' and 'start()' methods.")
    }

    /// Enables external software to read audio from Csound after calling [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps)
    /// # Returns
    /// The number of samples copied  or an
    /// error message if the internal csound's buffer has not been initialized.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let spout_length = csound.get_ksmps() * csound.output_channels();
    /// let mut spout_buffer = vec![0f64; spout_length as usize];
    /// while !csound.perform_ksmps() {
    ///     // fills your buffer with audio samples you want to pass into csound
    ///     foo_fill_buffer(&mut spout_buffer);
    ///     csound.read_spout_buffer(&spout_buffer);
    ///     // ...
    /// }
    /// ```
    /// # Deprecated
    /// Use [`Csound::get_spout`](struct.Csound.html#method.get_spout) to get a [`BufferPtr`](struct.BufferPtr.html)
    /// object.
    pub fn read_spout_buffer(&self, output: &mut [f64]) -> Result<usize, &'static str> {
        let size = self.get_ksmps() as usize * self.output_channels() as usize;
        let spout = unsafe { csound_sys::csoundGetSpout(self.engine.csound) as *const f64 };
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
        Err("The spout buffer is not initialized, call the 'compile()' and 'start()' methods.")
    }

    /// Enables external software to write audio into Csound before calling [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps)
    /// [`Csound::get_ksmps`](struct.Csound.html#method.get_ksmps) * [`Csound::input_channels`](struct.Csound.html#method.input_channels).
    /// # Returns
    /// The number of samples copied  or an
    /// error message if the internal csound's buffer has not been initialized.
    /// # Example
    /// ```
    /// let csound = Csound::new();
    /// csound.compile_csd("some_file_path");
    /// csound.start();
    /// let spin_length = csound.get_ksmps() * csound.input_channels();
    /// let mut spin_buffer = vec![0f64; spin_length as usize];
    /// while !csound.perform_ksmps() {
    ///     // fills your buffer with audio samples you want to pass into csound
    ///     foo_fill_buffer(&mut spin_buffer);
    ///     csound.write_spin_buffer(&spin_buffer);
    ///     // ...
    /// }
    /// ```
    /// # Deprecated
    /// Use [`Csound::get_spin`](struct.Csound.html#method.get_spin) to get a [`BufferPtr`](struct.BufferPtr.html)
    /// object.
    pub fn write_spin_buffer(&self, input: &[f64]) -> Result<usize, &'static str> {
        let size = self.get_ksmps() as usize * self.input_channels() as usize;
        let spin = unsafe { csound_sys::csoundGetSpin(self.engine.csound) as *mut f64 };
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
        Err("The spin buffer is not initialized, call the 'compile()' and 'start()' methods.")
    }

    /// Clears the spin buffer.
    pub fn clear_spin(&self) {
        unsafe {
            csound_sys::csoundClearSpin(self.engine.csound);
        }
    }

    /// Adds the indicated sample into the audio input working buffer (spin);
    ///  this only ever makes sense before calling [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps).
    ///  The frame and channel must be in bounds relative to ksmps and nchnls.
    /// *Note*:  the spin buffer needs to be cleared at every k-cycle by calling [`Csound::clear_spin`](struct.Csound.html#method.clear_spin).
    pub fn add_spin_sample(&self, frame: u32, channel: u32, sample: f64) {
        unsafe {
            csound_sys::csoundAddSpinSample(
                self.engine.csound,
                frame as i32,
                channel as i32,
                sample as c_double,
            );
        }
    }

    /// Sets the audio input working buffer (spin) to the indicated sample.
    /// this only ever makes sense before calling [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps).
    /// The frame and channel must be in bounds relative to ksmps and nchnls.
    pub fn set_spin_sample(&self, frame: u32, channel: u32, sample: f64) {
        unsafe {
            csound_sys::csoundSetSpinSample(
                self.engine.csound,
                frame as i32,
                channel as i32,
                sample as c_double,
            );
        }
    }

    /// Gets an audio sample from the spout buffer.
    /// only ever makes sense before calling [`Csound::perform_ksmps`](struct.Csound.html#method.perform_ksmps).
    /// The frame and channel must be in bounds relative to ksmps and nchnls.
    /// #Returns
    /// The indicated sample from the Csound audio output working buffer (spout).
    pub fn get_spout_sample(&self, frame: u32, channel: u32) -> f64 {
        unsafe {
            csound_sys::csoundGetSpoutSample(self.engine.csound, frame as i32, channel as i32)
                as f64
        }
    }

    /// Enable to host to handle the audio implementation.
    /// Calling this function with a non-zero 'state' value between [`Csound::create`](struct.Csound.html#method.create) and the start of performance will disable
    /// all default handling of sound I/O by the Csound library,
    /// allowing the host application to use the *spin*,*spout*,*input*, *output* buffers directly.
    /// # Arguments
    /// * `state` An no zero value will diseable all default handling of sound I/O in csound.
    /// * `bufSize` For applications using *spin* / *spout*, this argument should be set to 0 but if *bufSize* is greater than zero, the buffer size (-b) in frames will be set to the integer
    /// multiple of ksmps that is nearest to the value specified.
    pub fn set_host_implemented_audioIO(&self, state: u32, bufSize: u32) {
        unsafe {
            csound_sys::csoundSetHostImplementedAudioIO(
                self.engine.csound,
                state as c_int,
                bufSize as c_int,
            );
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
                csound_sys::csoundGetAudioDevList(self.engine.csound, ptr::null_mut(), 0);
            let num_of_odevices =
                csound_sys::csoundGetAudioDevList(self.engine.csound, ptr::null_mut(), 0);

            let mut in_vec = vec![csound_sys::CS_AUDIODEVICE::default(); num_of_idevices as usize];
            let mut out_vec = vec![csound_sys::CS_AUDIODEVICE::default(); num_of_odevices as usize];

            csound_sys::csoundGetAudioDevList(self.engine.csound, in_vec.as_mut_ptr(), 0);
            csound_sys::csoundGetAudioDevList(self.engine.csound, out_vec.as_mut_ptr(), 1);

            for dev in &in_vec {
                input_devices.push(CsAudioDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    rt_module: Trampoline::ptr_to_string(dev.rt_module.as_ptr()),
                    max_nchnls: dev.max_nchnls as u32,
                    isOutput: 0,
                });
            }
            for dev in &out_vec {
                output_devices.push(CsAudioDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    rt_module: Trampoline::ptr_to_string(dev.rt_module.as_ptr()),
                    max_nchnls: dev.max_nchnls as u32,
                    isOutput: 1,
                });
            }
        }
        (input_devices, output_devices)
    }

    /* Real time MIDI IO functions implmentations *************************************************************** */

    /// Sets the current MIDI IO module
    pub fn set_midi_module(&self, name: &str) {
        unsafe {
            let devName = CString::new(name);
            if devName.is_ok() {
                csound_sys::csoundSetMIDIModule(self.engine.csound, devName.unwrap().as_ptr());
            }
        }
    }

    /// call this function with state 1 if the host is going to implement MIDI through the callbacks
    pub fn set_host_implemented_midiIO(&self, state: u32) {
        unsafe {
            csound_sys::csoundSetHostImplementedMIDIIO(self.engine.csound, state as c_int);
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
                csound_sys::csoundGetMIDIDevList(self.engine.csound, ptr::null_mut(), 0);
            let num_of_odevices =
                csound_sys::csoundGetMIDIDevList(self.engine.csound, ptr::null_mut(), 0);

            let mut in_vec = vec![csound_sys::CS_MIDIDEVICE::default(); num_of_idevices as usize];
            let mut out_vec = vec![csound_sys::CS_MIDIDEVICE::default(); num_of_odevices as usize];

            csound_sys::csoundGetMIDIDevList(self.engine.csound, in_vec.as_mut_ptr(), 0);
            csound_sys::csoundGetMIDIDevList(self.engine.csound, out_vec.as_mut_ptr(), 1);

            for dev in &in_vec {
                input_devices.push(CsMidiDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    midi_module: Trampoline::ptr_to_string(dev.midi_module.as_ptr()),
                    interface_name: Trampoline::ptr_to_string(dev.interface_name.as_ptr()),
                    isOutput: 0,
                });
            }
            for dev in &out_vec {
                output_devices.push(CsMidiDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    midi_module: Trampoline::ptr_to_string(dev.midi_module.as_ptr()),
                    interface_name: Trampoline::ptr_to_string(dev.interface_name.as_ptr()),
                    isOutput: 1,
                });
            }
        }
        (input_devices, output_devices)
    }

    /* Score Handling functions implmentations ********************************************************* */

    /// Reads, preprocesses, and loads a score from an ASCII string.
    /// It can be called repeatedly with the new score events being added to the currently scheduled ones.
    pub fn read_score(&self, score: &str) -> Result<(), &'static str> {
        unsafe {
            match CString::new(score) {
                Ok(s) => {
                    if csound_sys::csoundReadScore(self.engine.csound, s.as_ptr())
                        == csound_sys::CSOUND_SUCCESS
                    {
                        Ok(())
                    } else {
                        Err("Can't read the score")
                    }
                }
                _ => Err("Invalid score"),
            }
        }
    }

    /// Asynchronous version of [`Csound::read_score`](struct.Csound.html#method.read_score)
    pub fn read_score_async(&self, score: &str) -> Result<(), &'static str> {
        unsafe {
            match CString::new(score) {
                Ok(s) => {
                    csound_sys::csoundReadScoreAsync(self.engine.csound, s.as_ptr());
                    Ok(())
                }
                _ => Err("Invalid score"),
            }
        }
    }

    /// # Returns
    /// The current score time in seconds since the beginning of the performance.
    pub fn get_score_time(&self) -> f64 {
        unsafe { csound_sys::csoundGetScoreTime(self.engine.csound) as f64 }
    }

    /// Sets whether Csound score events are performed or not.
    /// Independently of real-time MIDI events (see [`Csound::set_score_pending`](struct.Csound.html#method.set_score_pending)).
    pub fn is_score_pending(&self) -> i32 {
        unsafe { csound_sys::csoundIsScorePending(self.engine.csound) as i32 }
    }

    /// Sets whether Csound score events are performed or not (real-time events will continue to be performed).
    ///  Can be used by external software, such as a VST host, to turn off performance of score events (while continuing to perform real-time events),
    ///  for example to mute a Csound score while working on other tracks of a piece, or to play the Csound instruments live.
    pub fn set_score_pending(&self, pending: i32) {
        unsafe {
            csound_sys::csoundSetScorePending(self.engine.csound, pending as c_int);
        }
    }

    /// Gets the current score's time.
    /// # Returns
    /// The score time beginning at which score events will actually immediately be performed
    /// (see  [`Csound::set_score_offset_seconds`](struct.Csound.html#method.set_score_offset_seconds)).
    pub fn get_score_offset_seconds(&self) -> f64 {
        unsafe { csound_sys::csoundGetScoreOffsetSeconds(self.engine.csound) as f64 }
    }

    /// Csound score events prior to the specified time are not performed.
    /// And performance begins immediately at the specified time
    /// (real-time events will continue to be performed as they are received).
    /// Can be used by external software, such as a VST host, to begin score performance midway through a Csound score,
    ///  for example to repeat a loop in a sequencer or to synchronize other events with the Csound score.
    pub fn set_score_offset_seconds(&self, offset: f64) {
        unsafe {
            csound_sys::csoundSetScoreOffsetSeconds(self.engine.csound, offset as c_double);
        }
    }

    /// Rewinds a compiled Csound score to the time specified with [`Csound::set_score_offset_seconds`](struct.Csound.html#method.set_score_offset_seconds)
    pub fn rewindScore(&self) {
        unsafe {
            csound_sys::csoundRewindScore(self.engine.csound);
        }
    }
    // TODO SCORE SORT FUNCTIONS

    /* Engine general messages functions implmentations ********************************************************* */

    /// # Returns
    /// The Csound message level (from 0 to 231).
    pub fn get_message_level(&self) -> u32 {
        unsafe { csound_sys::csoundGetMessageLevel(self.engine.csound) as u32 }
    }

    /// Sets the Csound message level (from 0 to 231).
    pub fn set_message_level(&self, level: u32) {
        unsafe {
            csound_sys::csoundSetMessageLevel(self.engine.csound, level as c_int);
        }
    }

    /// Creates a buffer for storing messages printed by Csound. Should be called after creating a Csound instance and the buffer can be freed by
    /// calling [`Csound::destroy_message_buffer`](struct.Csound.html#method.destroy_message_buffer) or it will freed when the csound instance is dropped.
    /// You will generally want to call [`Csound::cleanup`](struct.Csound.html#method.cleanup) to make sure the last messages are flushed to the message buffer before destroying Csound.
    /// # Arguments
    /// * `toStdOut` If is non-zero, the messages are also printed to stdout and stderr (depending on the type of the message), in addition to being stored in the buffer.
    /// *Note*: Using the message buffer ties up the internal message callback,
    /// so [`Csound::message_string_callback`](struct.Csound.html#method.message_string_callback) should not be called after creating the message buffer.
    pub fn create_message_buffer(&self, stdout: i32) {
        unsafe {
            csound_sys::csoundCreateMessageBuffer(self.engine.csound, stdout as c_int);
            let mut msg_buff = self.engine.use_msg_buffer.borrow_mut();
            *msg_buff = true;
        }
    }

    /// Releases all memory used by the message buffer.
    /// If this buffer is created, the Drop method
    /// will call this function when the Csound instance were dropped.
    pub fn destroy_message_buffer(&self) {
        unsafe {
            csound_sys::csoundDestroyMessageBuffer(self.engine.csound);
            let mut msg_buff = self.engine.use_msg_buffer.borrow_mut();
            *msg_buff = false;
        }
    }

    /// # Returns
    /// The first message from the buffer.
    pub fn get_first_message(&self) -> Option<String> {
        unsafe {
            match CStr::from_ptr(csound_sys::csoundGetFirstMessage(self.engine.csound)).to_str() {
                Ok(m) => Some(m.to_owned()),
                _ => None,
            }
        }
    }

    /// # Returns
    /// The attribute parameter ([`MessageType`](enum.MessageType.html)) of the first message in the buffer.
    pub fn get_first_message_attr(&self) -> MessageType {
        unsafe {
            MessageType::from(csound_sys::csoundGetFirstMessageAttr(self.engine.csound) as u32)
        }
    }

    /// Removes the first message from the buffer.
    pub fn pop_first_message(&self) {
        unsafe {
            csound_sys::csoundPopFirstMessage(self.engine.csound);
        }
    }

    /// # Returns
    /// The number of pending messages in the buffer.
    pub fn get_message_count(&self) -> u32 {
        unsafe { csound_sys::csoundGetMessageCnt(self.engine.csound) as u32 }
    }

    /* Engine general Channels, Control and Events implementations ********************************************** */

    /// Requests a list of all control channels.
    /// # Returns
    /// A vector with all control channels info or None if there are not control channels. see: [`ChannelInfo`](struct.ChannelInfo.html)
    pub fn list_channels(&self) -> Option<Vec<ChannelInfo>> {
        let mut ptr = ptr::null_mut() as *mut csound_sys::controlChannelInfo_t;
        let ptr2: *mut *mut csound_sys::controlChannelInfo_t = &mut ptr as *mut *mut _;

        unsafe {
            let count = csound_sys::csoundListChannels(self.engine.csound, ptr2) as i32;
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
                csound_sys::csoundDeleteChannelList(self.engine.csound, *ptr2);
                return Some(list);
            }
            None
        }
    }

    /// Return a [`ControlChannelPtr`](struct.ControlChannelPtr.html) which represent a csound's channel ptr.
    /// creating the channel first if it does not exist yet.
    /// # Arguments
    /// * `name` The channel name.
    /// * `channel_type` must be the bitwise OR of exactly one of the following values:
    ///  - CSOUND_CONTROL_CHANNEL
    ///     control data (one MYFLT value)
    ///  - CSOUND_AUDIO_CHANNEL
    ///     audio data (get_ksmps() f64 values)
    ///  - CSOUND_STRING_CHANNEL
    ///     string data (f64 values with enough space to store
    ///     get_channel_data_size() characters, including the
    ///     NULL character at the end of the string)
    /// and at least one of these:
    ///  - CSOUND_INPUT_CHANNEL
    ///  - CSOUND_OUTPUT_CHANNEL
    /// If the channel already exists, it must match the data type
    /// (control, audio, or string), however, the input/output bits are
    /// OR'd with the new value. Note that audio and string channels
    /// can only be created after calling Compile(), because the
    /// storage size is not known until then.
    /// # Returns
    /// The ControlChannelPtr on success or a Status code,
    ///   "Not enough memory for allocating the channel" (CS_MEMORY)
    ///   "The specified name or type is invalid" (CS_ERROR)
    /// or, if a channel with the same name but incompatible type
    /// already exists, the type of the existing channel.
    /// * Note:* to find out the type of a channel without actually
    /// creating or changing it, set 'channel_type' argument  to CSOUND_UNKNOWN_CHANNEL, so that the error
    /// value will be either the type of the channel, or CSOUND_ERROR
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
    pub fn get_channel_ptr<'a>(
        &'a self,
        name: &str,
        channel_type: ControlChannelType,
    ) -> Result<ControlChannelPtr<'a>, Status> {
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let channel = ControlChannelType::from_bits(
            channel_type.bits() & ControlChannelType::CSOUND_CHANNEL_TYPE_MASK.bits(),
        )
        .unwrap();
        let len: usize = match channel {
            ControlChannelType::CSOUND_CONTROL_CHANNEL => std::mem::size_of::<f64>(),
            ControlChannelType::CSOUND_AUDIO_CHANNEL => self.get_ksmps() as usize,
            /*ControlChannelType::CSOUND_STRING_CHANNEL => {
                self.get_channel_data_size(name) / std::mem::size_of::<f64>()
            }*/
            _ => return Err(Status::CS_ERROR),
        };
        unsafe {
            let result = Status::from(csound_sys::csoundGetChannelPtr(
                self.engine.csound,
                ptr,
                cname.as_ptr(),
                channel_type.bits() as c_int,
            ));
            match result {
                Status::CS_SUCCESS => Ok(ControlChannelPtr {
                    ptr: *ptr,
                    channel_type: channel,
                    len,
                    phantom: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
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
        let attr = CString::new(attr).map_err(|_| Status::CS_ERROR)?;
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
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
                self.engine.csound,
                cname.as_ptr(),
                channel_hint,
            ) as i32)
            {
                Status::CS_SUCCESS => Ok(()),
                status => Err(status),
            }
        }
    }

    /// Returns special parameters (or None if there are not any) of a control channel.
    /// Previously set with csoundSetControlChannelHints() or the
    /// [chnparams](http://www.csounds.com/manualOLPC/chnparams.html) opcode.
    pub fn get_channel_hints(&self, name: &str) -> Result<ChannelHints, Status> {
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
        let mut hint = csound_sys::controlChannelHints_t::default();
        unsafe {
            match csound_sys::csoundGetControlChannelHints(
                self.engine.csound,
                cname.as_ptr() as *mut c_char,
                &mut hint as *mut _,
            ) {
                csound_sys::CSOUND_SUCCESS => {
                    //let hint = Box::from_raw(hint);

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
    pub fn get_control_channel(&self, name: &str) -> Result<f64, &'static str> {
        let cname = CString::new(name).map_err(|_| "invalid channel name")?;
        let mut err: c_int = 0;
        unsafe {
            let ret = csound_sys::csoundGetControlChannel(
                self.engine.csound,
                cname.as_ptr(),
                &mut err as *mut _,
            ) as f64;
            if (err) == csound_sys::CSOUND_SUCCESS {
                Ok(ret)
            } else {
                Err("channel not exist or is not a control channel")
            }
        }
    }

    /// Sets the value of control channel.
    /// # Arguments
    /// * `name`  The channel name.
    pub fn set_control_channel(&self, name: &str, value: f64) {
        let cname = CString::new(name).unwrap();
        unsafe {
            csound_sys::csoundSetControlChannel(self.engine.csound, cname.as_ptr(), value);
        }
    }

    /// Copies samples from an audio channel.
    /// # Arguments
    /// * `name` The channel name.
    /// * `out` The slice where the date contained in the internal audio channel buffer
    /// will be copied. Should contain enough memory for ksmps f64 samples.
    /// # Panic
    /// If the buffer passed to this function has not enough memory.
    pub fn read_audio_channel(&self, name: &str, output: &mut [f64]) {
        let size = self.get_ksmps() as usize;
        let bytes = output.len();
        let cname = CString::new(name).unwrap();
        assert!(
            size <= bytes,
            "The audio channel's capacity is {} so, it isn't possible to copy {} samples",
            size,
            bytes
        );
        unsafe {
            csound_sys::csoundGetAudioChannel(
                self.engine.csound,
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
    pub fn write_audio_channel(&self, name: &str, input: &[f64]) {
        let size = self.get_ksmps() as usize * self.input_channels() as usize;
        let bytes = input.len();
        let cname = CString::new(name).unwrap();
        assert!(
            size <= bytes,
            "The audio channel's capacity is {} so, it isn't possible to copy {} bytes",
            size,
            bytes
        );
        unsafe {
            csound_sys::csoundSetAudioChannel(
                self.engine.csound,
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
                self.engine.csound,
                cname.as_ptr(),
                ptr.as_ptr() as *mut _,
            );
        }
        data
    }

    /// Sets the string channel identified by *name* with *content*
    pub fn set_string_channel(&self, name: &str, content: &str) {
        let cname = CString::new(name).unwrap();
        let content = CString::new(content).unwrap();
        unsafe {
            csound_sys::csoundSetStringChannel(
                self.engine.csound,
                cname.as_ptr(),
                content.as_ptr() as *mut _,
            );
        }
    }

    /// returns the size of data stored in the channel identified by *name*
    pub fn get_channel_data_size(&self, name: &str) -> usize {
        let cname = CString::new(name).unwrap();
        unsafe { csound_sys::csoundGetChannelDatasize(self.engine.csound, cname.as_ptr()) as usize }
    }

    /// Receives a PVSDAT fout from the [*pvsout*](http://www.csounds.com/manual/html/pvsout.html) opcode.
    /// This method will return Ok on success,
    /// [`Status::CS_ERROR`](enum.Status.html#member.CS_ERROR) if the channel name is not valid or the channel doesn't
    /// exist or [`Status::CS_MEMORY`](enum.Status.html#member.CS_MEMORY) if the frame buffer lengths haven't the same size
    /// as the requested table
    /// # Arguments
    /// * `name` The channel identifier.
    /// * `pvs_data` Reference to tha struct which will be filled with the pvs data.
    /// # Example
    /// ```
    /// let mut pvs = PvsDataExt::new(512);
    /// cs.get_pvs_channel("1", &mut pvs);
    /// ```
    pub fn get_pvs_channel(&self, name: &str, pvs_data: &mut PvsDataExt) -> Result<(), Status> {
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
        let mut ptr = ptr::null_mut() as *mut f64;
        unsafe {
            if csound_sys::csoundGetChannelPtr(
                self.engine.csound,
                &mut ptr as *mut *mut _,
                cname.as_ptr(),
                (csound_sys::CSOUND_PVS_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int,
            ) == csound_sys::CSOUND_SUCCESS
            {
                if (*(ptr as *mut csound_sys::PVSDATEXT)).N == pvs_data.N as c_int {
                    let data = &mut csound_sys::PVSDATEXT::default();
                    data.frame = pvs_data.frame.as_mut_slice().as_ptr() as *mut f32;
                    let result = csound_sys::csoundGetPvsChannel(
                        self.engine.csound,
                        &mut *data,
                        cname.as_ptr(),
                    );
                    match result {
                        csound_sys::CSOUND_SUCCESS => {
                            pvs_data.N = data.N as u32;
                            pvs_data.sliding = data.sliding as u32;
                            pvs_data.NB = data.NB as i32;
                            pvs_data.overlap = data.overlap as u32;
                            pvs_data.winsize = data.winsize as u32;
                            pvs_data.wintype = data.wintype as u32;
                            pvs_data.format = data.format as u32;
                            pvs_data.framecount = data.framecount as u32;
                            Ok(())
                        }
                        err => Err(Status::from(err)),
                    }
                } else {
                    Err(Status::CS_MEMORY)
                }
            } else {
                Err(Status::CS_ERROR)
            }
        }
    }

    pub fn set_pvs_channel(&self, name: &str, pvs_data: &PvsDataExt) {
        unsafe {
            let cname = CString::new(name);
            if cname.is_ok() {
                let data = &mut csound_sys::PVSDATEXT {
                    N: pvs_data.N as _,
                    sliding: pvs_data.sliding as _,
                    NB: pvs_data.NB as _,
                    overlap: pvs_data.overlap as _,
                    winsize: pvs_data.winsize as _,
                    wintype: pvs_data.wintype as _,
                    format: pvs_data.format as _,
                    framecount: pvs_data.framecount as _,
                    frame: pvs_data.frame.as_slice().as_ptr() as *mut f32,
                };
                csound_sys::csoundSetPvsChannel(
                    self.engine.csound,
                    &*data,
                    cname.unwrap().as_ptr(),
                );
            }
        }
    }

    /// Send a new score event.
    /// # Arguments
    /// * `event_type` is the score event type ('a', 'i', 'q', 'f', or 'e').
    /// * `pfields` is a slice of f64 values with all the pfields for this event.
    /// # Example
    /// ```
    /// let cs = Csound::new();
    /// let pFields = [1.0, 1.0, 5.0];
    /// while cs.perform_ksmps() == false {
    ///     cs.send_score_event('i', &pFields);
    /// }
    /// ```
    pub fn send_score_event(&self, event_type: char, pfields: &[f64]) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEvent(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
            ) as i32)
        }
    }

    /// Like [`Csound::send_score_event`](struct.Csound.html#method.send_score_event).
    /// This function inserts a score event,
    /// but at absolute time with respect to the start of performance,
    /// or from an offset set with *time_offset*
    pub fn send_score_event_absolute(
        &self,
        event_type: char,
        pfields: &[f64],
        time_offset: f64,
    ) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEventAbsolute(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
                time_offset as c_double,
            ) as i32)
        }
    }

    /// Asynchronous version of [`Csound::send_score_event`](struct.Csound.html#method.send_score_event)
    pub fn send_score_event_async(&self, event_type: char, pfields: &[f64]) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEventAsync(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
            ) as i32)
        }
    }

    /// Asynchronous version of [`Csound::send_score_event_absolute`](struct.Csound.html#method.send_score_event_absolute)
    pub fn send_score_event_absolute_async(
        &self,
        event_type: char,
        pfields: &[f64],
        time_offset: f64,
    ) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEventAbsoluteAsync(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
                time_offset as c_double,
            ) as i32)
        }
    }

    /// Input a string (as if from a console), used for line events.
    /// # Example
    /// ```
    /// let cs = Csound::new();
    /// let pFields = [1.0, 1.0, 5.0];
    /// while cs.perform_ksmps() == false {
    ///     cs.send_input_message("i 2 0 0.75  1");
    /// }
    /// ```
    pub fn send_input_message(&self, message: &str) -> Result<(), NulError> {
        let cmessage = CString::new(message)?;
        unsafe {
            csound_sys::csoundInputMessage(self.engine.csound, cmessage.as_ptr() as *const c_char);
            Ok(())
        }
    }

    /// Asynchronous version of [`Csound::send_input_message`](struct.Csound.html#method.send_input_message)
    pub fn send_input_message_async(&self, message: &str) -> Result<(), NulError> {
        let cmessage = CString::new(message)?;
        unsafe {
            csound_sys::csoundInputMessageAsync(
                self.engine.csound,
                cmessage.as_ptr() as *const c_char,
            );
            Ok(())
        }
    }

    /// Kills off one or more running instances of an instrument.
    /// # Arguments
    /// * `instr` The numeric identifier of the instrument.
    /// * `name` The string identifier of the instrument or name. If it is None, the instrument
    /// numeric identifier is used.
    /// * `mode` is a sum of the following values: 0,1,2: kill all instances (1), oldest only (1), or newest (2)
    /// 4: only turnoff notes with exactly matching (fractional) instr number
    /// 8: only turnoff notes with indefinite duration (p3 < 0 or MIDI).
    /// * `allow_release` if true, the killed instances are allowed to release.
    pub fn kill_instrument(
        &self,
        instr: f64,
        name: Option<&str>,
        mode: u32,
        allow_release: bool,
    ) -> Status {
        let cname = CString::new(name.unwrap_or_else(|| "")).unwrap();
        unsafe {
            Status::from(csound_sys::csoundKillInstance(
                self.engine.csound,
                instr as c_double,
                cname.as_ptr() as *const c_char,
                mode as c_int,
                allow_release as c_int,
            ) as i32)
        }
    }

    /// Set the ASCII code of the most recent key pressed.
    /// # Arguments
    /// * `key` The ASCII identifier for the key pressed.
    pub fn key_press(&self, key: char) {
        unsafe {
            csound_sys::csoundKeyPress(self.engine.csound, key as c_char);
        }
    }

    /* Engine general Table function  implementations **************************************************************************************** */

    /// Returns the length of a function table (not including the guard point), or an error
    /// message if the table doens't exist.
    /// # Arguments
    /// * `table` The function table identifier.
    pub fn table_length(&self, table: u32) -> Result<usize, &'static str> {
        unsafe {
            let value = csound_sys::csoundTableLength(self.engine.csound, table as c_int) as i32;
            if value > 0 {
                Ok(value as usize)
            } else {
                Err("Table doesn't exist")
            }
        }
    }

    /// Returns the value of a slot in a function table.
    /// If the Table or index are not valid, an error message will be returned.
    /// # Arguments
    /// * `table` The function table identifier.
    /// * `index` The value at table[index] which will be read.
    pub fn table_get(&self, table: u32, index: u32) -> Result<f64, &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if index < size as u32 {
                Ok(
                    csound_sys::csoundTableGet(self.engine.csound, table as c_int, index as c_int)
                        as f64,
                )
            } else {
                Err("index out of range")
            }
        }
    }

    /// Sets the value of a slot in a function table.
    /// # Arguments
    /// * `table` The function table identifier.
    /// * `index` The slot at table[index] where value will be added.
    /// # Returns
    /// An error message if the index or table are no valid
    pub fn table_set(&self, table: u32, index: u32, value: f64) -> Result<(), &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if index < size as u32 {
                csound_sys::csoundTableSet(
                    self.engine.csound,
                    table as c_int,
                    index as c_int,
                    value,
                );
                Ok(())
            } else {
                Err("index out of range")
            }
        }
    }

    /// Copies the content of a function table into a slice.
    /// # Arguments
    /// * `table` The function table identifier.
    /// # Returns
    /// An error message if the table doesn't exist or the passed slice
    /// doesn't have enough memory to content the table values.
    pub fn table_copy_out(&self, table: u32, output: &mut [f64]) -> Result<(), &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if output.len() < size {
                Err("Not enough memory to copy the table")
            } else {
                csound_sys::csoundTableCopyOut(
                    self.engine.csound,
                    table as c_int,
                    output.as_ptr() as *mut c_double,
                );
                Ok(())
            }
        }
    }

    /// Asynchronous version of [`Csound:: table_copy_out`](struct.Csound.html#method.table_copy_out)
    pub fn table_copy_out_async(&self, table: u32, output: &mut [f64]) -> Result<(), &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if output.len() < size {
                Err("Not enough memory to copy the table")
            } else {
                csound_sys::csoundTableCopyOutAsync(
                    self.engine.csound,
                    table as c_int,
                    output.as_ptr() as *mut c_double,
                );
                Ok(())
            }
        }
    }

    /// Copy the contents of an array into a given function table.
    /// # Arguments
    /// * `table` The function table identifier.
    /// * `src` Slice with the values to be copied into the function table
    /// # Returns
    /// An error message if the table doesn't exist or doesn't have enough
    /// capacity.
    pub fn table_copy_in(&self, table: u32, src: &[f64]) -> Result<(), &'static str> {
        let size = self.table_length(table)?;
        if size < src.len() {
            Err("Table doesn't have enough capacity")
        } else {
            unsafe {
                csound_sys::csoundTableCopyIn(
                    self.engine.csound,
                    table as c_int,
                    src.as_ptr() as *const c_double,
                );
                Ok(())
            }
        }
    }

    /// Asynchronous version of [`Csound:: table_copy_in`](struct.Csound.html#method.table_copy_in)
    pub fn table_copy_in_async(&self, table: u32, src: &[f64]) -> Result<(), &'static str> {
        let size = self.table_length(table)?;
        if size < src.len() {
            Err("Table doesn't have enough capacity")
        } else {
            unsafe {
                csound_sys::csoundTableCopyInAsync(
                    self.engine.csound,
                    table as c_int,
                    src.as_ptr() as *const c_double,
                );
                Ok(())
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
    /// ```
    /// let cs = Csound::new();
    /// cs.compile_csd("some.csd");
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table_buff = vec![0f64; cs.table_length(1).unwrap() as usize];
    ///     // Gets the function table 1
    ///     let mut table = cs.get_table(1).unwrap();
    ///     // Copies the table content into table_buff
    ///     table.read( table_buff.as_mut_slice() ).unwrap();
    ///     // Do some stuffs
    ///     table.write(&table_buff.into_iter().map(|x| x*2.5).collect::<Vec<f64>>().as_mut_slice());
    ///     // Do some stuffs
    /// }
    /// ```
    /// see [`Table::read`](struct.Table.html#method.read) or [`Table::write`](struct.Table.html#method.write).
    pub fn get_table(&self, table: u32) -> Option<Table> {
        let mut ptr = ptr::null_mut() as *mut c_double;
        let length;
        unsafe {
            length = csound_sys::csoundGetTable(
                self.engine.csound,
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
                self.engine.csound,
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
                self.engine.csound,
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

    /// Checks if a given *gen* number is a named GEN
    /// # Arguments
    /// * `gen` The GEN number identifier.
    /// # Returns
    /// The GEN names's length
    pub fn is_named_gen(&self, gen: u32) -> usize {
        unsafe { csound_sys::csoundIsNamedGEN(self.engine.csound, gen as c_int) as usize }
    }

    /// Returns the GEN name if it exist ans is named, else, returns None
    /// # Arguments
    /// * `gen` The GEN number identifier.
    /// # Returns
    /// A option with the GEN name or None if the GEN is not a named one
    /// or not exist.
    pub fn get_gen_name(&self, gen: u32) -> Option<String> {
        unsafe {
            let len = self.is_named_gen(gen);
            if len > 0 {
                let name = vec![0u8; len];
                let name_raw = CString::from_vec_unchecked(name).into_raw();
                csound_sys::csoundGetNamedGEN(
                    self.engine.csound,
                    gen as c_int,
                    name_raw,
                    len as c_int,
                );
                let name = CString::from_raw(name_raw);
                match name.to_str() {
                    Ok(str) => Some(str.to_owned()),
                    Err(_) => None,
                }
            } else {
                None
            }
        }
    }

    /* Engine general Opcode function  implementations **************************************************************************************** */

    /// Gets an alphabetically sorted list of all opcodes.
    /// Should be called after externals are loaded by csoundCompile().
    /// The opcode information is contained in a [`Csound::OpcodeListEntry`](struct.Csound.html#struct.OpcodeListEntry)
    pub fn get_opcode_list_entry(&self) -> Option<Vec<OpcodeListEntry>> {
        let mut ptr = ptr::null_mut() as *mut csound_sys::opcodeListEntry;
        let length;
        unsafe {
            length = csound_sys::csoundNewOpcodeList(
                self.engine.csound,
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
                csound_sys::csoundDisposeOpcodeList(self.engine.csound, ptr);
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
    /// *Language::CSLANGUAGE_DEFAULT* can be used to disable translation of messages and
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
    pub fn get_rand31(seed: &mut u32) -> Result<u32, &'static str> {
        unsafe {
            match seed {
                1...2_147_483_646 => {
                    let ptr: *mut u32 = &mut *seed;
                    let res = csound_sys::csoundRand31(ptr as *mut c_int) as u32;
                    Ok(res)
                }
                _ => Err("invalid seed value"),
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
    /// let csound = Csound::new();
    /// let circular_buffer = csound.create_circular_buffer::<f64>(1024);
    /// ```
    pub fn create_circular_buffer<'a, T: 'a + Copy>(&'a self, len: u32) -> CircularBuffer<T> {
        unsafe {
            let ptr: *mut T = csound_sys::csoundCreateCircularBuffer(
                self.engine.csound,
                len as c_int,
                mem::size_of::<T>() as c_int,
            ) as *mut T;
            CircularBuffer {
                csound: self.engine.csound,
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
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .audio_dev_list_cb = Some(Box::new(f));
        }
        self.enable_callback(AUDIO_DEV_LIST);
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
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .play_open_cb = Some(Box::new(f));
        }
        self.enable_callback(PLAY_OPEN);
    }

    /// Sets a function to be called by Csound for opening real-time audio recording.
    /// This callback is used to inform the user about the current audio device Which
    /// Csound will use for opening realtime audio recording. You have to return Status::CS_SUCCESS
    pub fn rec_open_audio_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .play_open_cb = Some(Box::new(f));
        }
        self.enable_callback(REC_OPEN);
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
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .rt_play_cb = Some(Box::new(f));
        }
        self.enable_callback(REAL_TIME_PLAY);
    }

    /// Sets a function to be called by Csound for performing real-time audio recording.
    /// With this callback the user can fill a buffer with samples from a custom
    /// audio module, and pass it into csound.
    pub fn rt_audio_rec_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&mut [f64]) -> usize + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .rt_rec_cb = Some(Box::new(f));
        }
        self.enable_callback(REAL_TIME_REC);
    }

    /// Indicates to the user when csound has closed the rtaudio device.
    pub fn rt_close_callback<'c, F>(&self, f: F)
    where
        F: FnMut() + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .rt_close_cb = Some(Box::new(f));
        }
        self.enable_callback(RT_CLOSE_CB);
    }

    /// Sets  callback to be called once in every control period.
    /// This facility can be used to ensure a function is called synchronously
    /// before every csound control buffer processing.
    /// It is important to make sure no blocking operations are performed in the callback.
    pub fn sense_event_callback<'c, F>(&self, f: F)
    where
        F: FnMut() + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .sense_event_cb = Some(Box::new(f));
        }
        self.enable_callback(SENSE_EVENT);
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
    /// let mut cs = Csound::new();
    /// cs.message_string_callback(|att: MessageType, message: &str| print!("{}", message));
    /// ```
    pub fn message_string_callback<'c, F>(&self, f: F)
    where
        F: FnMut(MessageType, &str) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .message_cb = Some(Box::new(f));
        }
        self.enable_callback(MESSAGE_CB);
    }

    /*fn keyboard_callback<'c, F>(&self, f: F)
    where
        F: FnMut() -> char + 'c,
    {
        unsafe{(&mut *(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler)).callbacks.keyboard_cb = Some(Box::new(f));}
        self.enable_callback(KEYBOARD_CB);
    }*/

    /// Sets the function which will be called whenever the [*invalue*](http://www.csounds.com/manual/html/invalue.html) opcode is used.
    /// # Arguments
    /// * ´f´ Function which implement the FnMut trait. The invalue opcode will trigger this callback passing
    /// the channel name which requiere the data. This function/closure have to return the data which will be
    /// passed to that specific channel if not only return ChannelData::CS_UNKNOWN_CHANNEL. Only *String* and *control* Channels
    /// are supported.
    /// # Example
    /// ```
    /// let input_channel = |name: &str|->ChannelData {
    ///      if name == "myStringChannel"{
    ///          let myString = "my data".to_owned();
    ///          ChannelData::CS_STRING_CHANNEL(myString)
    ///      }
    ///      ChannelData::CS_UNKNOWN_CHANNEL
    /// };
    /// let mut cs = Csound::new();
    /// cs.input_channel_callback(input_channel);
    /// ```
    pub fn input_channel_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str) -> ChannelData + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .input_channel_cb = Some(Box::new(f));
        }
        self.enable_callback(CHANNEL_INPUT_CB);
    }

    /// Sets the function which will be called whenever the [*outvalue*](http://www.csounds.com/manual/html/outvalue.html) opcode is used.
    /// # Arguments
    /// * ´f´ Function which implement the FnMut trait. The outvalue opcode will trigger this callback passing
    /// the channel ##name and the channel's output data encoded in the ChannelData. Only *String* and *control* Channels
    /// are supported.
    /// # Example
    /// ```
    /// let output_channel = |name: &str, data:ChannelData|{
    ///      print!("channel name:{}  data: {:?}", name, data);
    /// };
    /// let mut cs = Csound::new();
    /// cs.output_channel_callback(output_channel);
    /// ```
    pub fn output_channel_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str, ChannelData) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .output_channel_cb = Some(Box::new(f));
        }
        self.enable_callback(CHANNEL_OUTPUT_CB);
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
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .file_open_cb = Some(Box::new(f));
        }
        self.enable_callback(FILE_OPEN_CB);
    }

    /// Sets a function to be called by Csound for opening real-time MIDI input.
    /// This callback is used to inform to the user about the current MIDI input device.
    /// # Arguments
    /// * `user_func` A function/closure which will receive a reference
    ///  to a str with the device name.
    pub fn midi_in_open_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_in_open_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_IN_OPEN_CB);
    }

    /// Sets a function to be called by Csound for opening real-time MIDI output.
    /// This callback is used to inform to the user about the current MIDI output device.
    /// # Arguments
    /// * `user_func` A function/closure which will receive a reference
    ///  to a str with the device name.
    pub fn midi_out_open_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&str) + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_out_open_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_OUT_OPEN_CB);
    }

    /// Sets a function to be called by Csound for reading from real time MIDI input.
    /// A reference to a buffer with audio samples is passed
    /// to the user function in the callback.  The callback have to return the number of elements written to the buffer.
    pub fn midi_read_callback<'c, F>(&self, f: F)
    where
        F: FnMut(&mut [u8]) -> usize + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_read_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_READ_CB);
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
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_write_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_WRITE_CB);
    }

    /// Indicates to the user when csound has closed the midi input device.
    pub fn midi_in_close_callback<'c, F>(&self, f: F)
    where
        F: FnMut() + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_in_close_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_IN_CLOSE);
    }

    /// Indicates to the user when csound has closed the midi output device.
    pub fn midi_out_close_callback<'c, F>(&self, f: F)
    where
        F: FnMut() + 'c,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_out_close_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_OUT_CLOSE);
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
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .yield_cb = Some(Box::new(f));
        }
        self.enable_callback(YIELD_CB);
    }

    fn enable_callback(&self, callback_type: u32) {
        match callback_type {
            SENSE_EVENT => unsafe {
                csound_sys::csoundRegisterSenseEventCallback(
                    self.engine.csound,
                    Some(Trampoline::senseEventCallback),
                    ::std::ptr::null_mut() as *mut c_void,
                );
            },
            MESSAGE_CB => unsafe {
                csound_sys::csoundSetMessageStringCallback(
                    self.engine.csound,
                    Trampoline::message_string_cb,
                )
            },

            AUDIO_DEV_LIST => unsafe {
                csound_sys::csoundSetAudioDeviceListCallback(
                    self.engine.csound,
                    Some(Trampoline::audioDeviceListCallback),
                );
            },
            PLAY_OPEN => unsafe {
                csound_sys::csoundSetPlayopenCallback(
                    self.engine.csound,
                    Some(Trampoline::playOpenCallback),
                );
            },
            REC_OPEN => unsafe {
                csound_sys::csoundSetRecopenCallback(
                    self.engine.csound,
                    Some(Trampoline::recOpenCallback),
                );
            },

            REAL_TIME_PLAY => unsafe {
                csound_sys::csoundSetRtplayCallback(
                    self.engine.csound,
                    Some(Trampoline::rtplayCallback),
                );
            },

            REAL_TIME_REC => unsafe {
                csound_sys::csoundSetRtrecordCallback(
                    self.engine.csound,
                    Some(Trampoline::rtrecordCallback),
                );
            },

            /*KEYBOARD_CB => unsafe {
                let host_data_ptr = &*self.engine as *const _ as *const _;
                csound_sys::csoundRegisterKeyboardCallback(
                    self.engine.csound,
                    Some(keyboard_callback::<H>),
                    host_data_ptr as *mut c_void,
                    csound_sys::CSOUND_CALLBACK_KBD_EVENT | csound_sys::CSOUND_CALLBACK_KBD_TEXT,
                );
                csound_sys::csoundKeyPress(self.engine.csound, '\n' as i8);
            },*/
            RT_CLOSE_CB => unsafe {
                csound_sys::csoundSetRtcloseCallback(
                    self.engine.csound,
                    Some(Trampoline::rtcloseCallback),
                );
            },

            CSCORE_CB => unsafe {
                csound_sys::csoundSetCscoreCallback(
                    self.engine.csound,
                    Some(Trampoline::scoreCallback),
                );
            },

            CHANNEL_INPUT_CB => unsafe {
                csound_sys::csoundSetInputChannelCallback(
                    self.engine.csound,
                    Some(Trampoline::inputChannelCallback),
                );
            },

            CHANNEL_OUTPUT_CB => unsafe {
                csound_sys::csoundSetOutputChannelCallback(
                    self.engine.csound,
                    Some(Trampoline::outputChannelCallback),
                );
            },

            FILE_OPEN_CB => unsafe {
                csound_sys::csoundSetFileOpenCallback(
                    self.engine.csound,
                    Some(Trampoline::fileOpenCallback),
                );
            },

            MIDI_IN_OPEN_CB => unsafe {
                csound_sys::csoundSetExternalMidiInOpenCallback(
                    self.engine.csound,
                    Some(Trampoline::midiInOpenCallback),
                );
            },

            MIDI_OUT_OPEN_CB => unsafe {
                csound_sys::csoundSetExternalMidiOutOpenCallback(
                    self.engine.csound,
                    Some(Trampoline::midiOutOpenCallback),
                );
            },

            MIDI_READ_CB => unsafe {
                csound_sys::csoundSetExternalMidiReadCallback(
                    self.engine.csound,
                    Some(Trampoline::midiReadCallback),
                );
            },

            MIDI_WRITE_CB => unsafe {
                csound_sys::csoundSetExternalMidiWriteCallback(
                    self.engine.csound,
                    Some(Trampoline::midiWriteCallback),
                );
            },

            MIDI_IN_CLOSE => unsafe {
                csound_sys::csoundSetExternalMidiInCloseCallback(
                    self.engine.csound,
                    Some(Trampoline::midiInCloseCallback),
                );
            },

            MIDI_OUT_CLOSE => unsafe {
                csound_sys::csoundSetExternalMidiOutCloseCallback(
                    self.engine.csound,
                    Some(Trampoline::midiOutCloseCallback),
                );
            },

            YIELD_CB => unsafe {
                csound_sys::csoundSetYieldCallback(
                    self.engine.csound,
                    Some(Trampoline::yieldCallback),
                );
            },

            _ => {}
        }
    }
} //End impl block

// Drop method to free the memory using during the csound performance and instantiation
impl Drop for Csound {
    fn drop(&mut self) {
        unsafe {
            csound_sys::csoundStop(self.engine.csound);
            csound_sys::csoundCleanup(self.engine.csound);
            let _ = Box::from_raw(
                csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler
            );
            // Checks if a message buffer exists and destroy it.
            let msg_buffer = self.engine.use_msg_buffer.borrow();
            if *msg_buffer == true {
                csound_sys::csoundDestroyMessageBuffer(self.engine.csound);
            }
            csound_sys::csoundDestroy(self.engine.csound);
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
    pub fn read(&self, out: &mut [T], items: u32) -> Result<usize, &'static str> {
        if items as usize <= out.len() {
            return Err("your buffer has not enough capacity");
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
    pub fn peek(&self, out: &mut [T], items: u32) -> Result<usize, &'static str> {
        if items as usize <= out.len() {
            return Err("your buffer has not enough capacity");
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
    pub fn write(&self, input: &[T], items: u32) -> Result<usize, &'static str> {
        if items as usize <= input.len() {
            return Err("your buffer has not enough capacity");
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
    /// ```
    /// let cs = Csound::new();
    /// cs.compile_csd("some.csd");
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table = cs.get_table(1).unwrap();
    ///     let mut table_buff = vec![0f64; table.length];
    ///     // copy Table::length elements from the table's internal buffer
    ///     table.copy_to_slice( table_buff.as_mut_slice() ).unwrap();
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
    /// ```
    /// let cs = Csound::new();
    /// cs.compile_csd("some.csd");
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table = cs.get_table(1).unwrap();
    ///     let mut table_buff = vec![0f64; table.length];
    ///     // copy Table::length elements from the table's internal buffer
    ///     table.read( table_buff.as_mut_slice() ).unwrap();
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

/// Rust representation for a raw csound channel pointer
///
/// Still in high development so changes might occur.
/// currently String channel is not supported.
#[derive(Debug)]
pub struct ControlChannelPtr<'a> {
    ptr: *mut f64,
    len: usize,
    channel_type: ControlChannelType,
    phantom: PhantomData<&'a f64>,
}

impl<'a> ControlChannelPtr<'a> {
    /// # Returns
    /// The channel length
    pub fn get_size(&self) -> usize {
        self.len
    }

    pub fn read<T: Copy>(&self, dest: &mut [T]) -> Result<usize, io::Error> {
        let mut len: usize = dest.len();
        if self.len < len {
            len = self.len;
        }
        if self.len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Missing data: requesting {} but only got {}.",
                    len, self.len
                ),
            ));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(self.ptr as *const T, dest.as_mut_ptr(), len);
        }
        Ok(len)
    }

    pub fn write<T: Copy>(&self, src: &[T]) -> Result<usize, io::Error> {
        let mut len: usize = src.len();
        if self.len < len {
            len = self.len;
        }
        if self.len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Not memory for data: writing {} but only got {}.",
                    len, self.len
                ),
            ));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.ptr as *mut T, len);
        }
        Ok(len)
    }
}
