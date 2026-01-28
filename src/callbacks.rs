#![allow(non_snake_case)]

use std::sync::atomic::{AtomicU32, Ordering};

use bitflags::bitflags;

use crate::enums::{ChannelData, FileTypes, MessageType, Status};
use crate::rtaudio::{CsAudioDevice, RtAudioParams};

use csound_sys as raw;
use raw::{CSOUND_STATUS, controlChannelType};

bitflags! {
    /// Bitflags tracking which callbacks have panicked.
    ///
    /// When a user-provided callback panics, its corresponding bit is set
    /// to prevent re-entering that specific callback. Other callbacks
    /// continue to function normally.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PanickedCallbacks: u32 {
        const MESSAGE         = 1 << 0;
        const DEVLIST         = 1 << 1;
        const PLAY_OPEN       = 1 << 2;
        const REC_OPEN        = 1 << 3;
        const RT_PLAY         = 1 << 4;
        const RT_REC          = 1 << 5;
        const KEYBOARD        = 1 << 6;
        const RT_CLOSE        = 1 << 7;
        const CSCORE          = 1 << 8;
        const INPUT_CHANNEL   = 1 << 9;
        const OUTPUT_CHANNEL  = 1 << 10;
        const FILE_OPEN       = 1 << 11;
        const MIDI_IN_OPEN    = 1 << 12;
        const MIDI_OUT_OPEN   = 1 << 13;
        const MIDI_READ       = 1 << 14;
        const MIDI_WRITE      = 1 << 15;
        const MIDI_IN_CLOSE   = 1 << 16;
        const MIDI_OUT_CLOSE  = 1 << 17;
        const YIELD           = 1 << 18;
    }
}

/// Atomic panic state for all callbacks.
///
/// Uses a single `AtomicU32` with bitflags for cache-friendly panic tracking.
/// Each callback checks/sets its own bit independently.
#[derive(Debug, Default)]
pub struct PanicState(AtomicU32);

impl PanicState {
    /// Creates a new panic state with no callbacks marked as panicked.
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    /// Checks if a specific callback has panicked.
    #[inline]
    pub fn has_panicked(&self, flag: PanickedCallbacks) -> bool {
        self.0.load(Ordering::Acquire) & flag.bits() != 0
    }

    /// Marks a callback as having panicked.
    #[inline]
    pub fn mark_panicked(&self, flag: PanickedCallbacks) {
        self.0.fetch_or(flag.bits(), Ordering::Release);
    }

    /// Returns the raw panic state bits.
    pub fn bits(&self) -> u32 {
        self.0.load(Ordering::Acquire)
    }

    /// Resets all panic state (useful when resetting Csound).
    pub fn reset(&self) {
        self.0.store(0, Ordering::Release);
    }
}

/// Struct containing the relevant info of files are opened by csound.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// pathname of the file; either full or relative to current dir
    pub name: Option<String>,
    /// Enum equivalent code for the file type code from the enum CSOUND_FILETYPES
    pub file_type: FileTypes,
    /// true if Csound is writing the file, false if reading
    pub is_writing: bool,
    /// true if  it is a temporary file that Csound will delete; false if not
    pub is_temp: bool,
}

#[doc(hidden)]
#[derive(Default)]
pub struct Callbacks<'a> {
    pub message_cb: Option<Box<dyn FnMut(MessageType, &str) + 'a>>,
    pub devlist_cb: Option<Box<dyn FnMut(CsAudioDevice) + 'a>>,
    pub play_open_cb: Option<Box<dyn FnMut(&RtAudioParams) -> Status + 'a>>,
    pub rec_open_cb: Option<Box<dyn FnMut(&RtAudioParams) -> Status + 'a>>,
    pub rt_play_cb: Option<Box<dyn FnMut(&[f64]) + 'a>>,
    pub rt_rec_cb: Option<Box<dyn FnMut(&mut [f64]) -> usize + 'a>>,
    #[allow(dead_code)] // TODO: this callback doesn't work on csound side
    pub keyboard_cb: Option<Box<dyn FnMut() -> char + 'a>>,
    pub rt_close_cb: Option<Box<dyn FnMut() + 'a>>,
    #[allow(dead_code)] // TODO: cscore callback not yet implemented
    pub cscore_cb: Option<Box<dyn FnMut() + 'a>>,
    pub input_channel_cb: Option<Box<dyn FnMut(&str) -> ChannelData + 'a>>,
    pub output_channel_cb: Option<Box<dyn FnMut(&str, ChannelData) + 'a>>,
    pub file_open_cb: Option<Box<dyn FnMut(&FileInfo) + 'a>>,
    pub midi_in_open_cb: Option<Box<dyn FnMut(&str) + 'a>>,
    pub midi_out_open_cb: Option<Box<dyn FnMut(&str) + 'a>>,
    pub midi_read_cb: Option<Box<dyn FnMut(&mut [u8]) -> usize + 'a>>,
    pub midi_write_cb: Option<Box<dyn FnMut(&[u8]) -> usize + 'a>>,
    pub midi_in_close_cb: Option<Box<dyn FnMut() + 'a>>,
    pub midi_out_close_cb: Option<Box<dyn FnMut() + 'a>>,
    pub yield_cb: Option<Box<dyn FnMut() -> bool + 'a>>,
}

impl<'a> Callbacks<'a> {
    pub(crate) unsafe fn set_message_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(MessageType, &str) + 'a,
    {
        unsafe {
            self.message_cb = Some(Box::new(cb));
            raw::csoundSetMessageStringCallback(csound, Some(Trampoline::message_string_cb))
        }
    }

    pub(crate) unsafe fn set_devlist_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(CsAudioDevice) + 'a,
    {
        unsafe {
            self.devlist_cb = Some(Box::new(cb));
            raw::csoundSetAudioDeviceListCallback(
                csound,
                Some(Trampoline::audioDeviceListCallback),
            );
        }
    }

    pub(crate) unsafe fn set_play_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'a,
    {
        unsafe {
            self.play_open_cb = Some(Box::new(cb));
            raw::csoundSetPlayopenCallback(csound, Some(Trampoline::playOpenCallback));
        }
    }

    pub(crate) unsafe fn set_rec_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'a,
    {
        unsafe {
            self.play_open_cb = Some(Box::new(cb));
            raw::csoundSetRecopenCallback(csound, Some(Trampoline::recOpenCallback));
        }
    }

    pub(crate) unsafe fn set_rt_play_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&[f64]) + 'a,
    {
        unsafe {
            self.rt_play_cb = Some(Box::new(cb));
            csound_sys::csoundSetRtplayCallback(csound, Some(Trampoline::rtplayCallback));
        }
    }

    pub(crate) unsafe fn set_rt_rec_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&mut [f64]) -> usize + 'a,
    {
        unsafe {
            self.rt_rec_cb = Some(Box::new(cb));
            csound_sys::csoundSetRtrecordCallback(csound, Some(Trampoline::rtrecordCallback));
        }
    }

    pub(crate) unsafe fn set_rt_close_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        unsafe {
            self.rt_close_cb = Some(Box::new(cb));
            csound_sys::csoundSetRtcloseCallback(csound, Some(Trampoline::rtcloseCallback));
        }
    }

    /*pub(crate) unsafe fn set_cscore_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        self.cscore_cb = Some(Box::new(cb));
        csound_sys::csoundSetCscoreCallback(
            csound,
            Some(Trampoline::scoreCallback),
        );
    }*/

    pub(crate) unsafe fn set_input_channel_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&str) -> ChannelData + 'a,
    {
        unsafe {
            self.input_channel_cb = Some(Box::new(cb));
            csound_sys::csoundSetInputChannelCallback(
                csound,
                Some(Trampoline::inputChannelCallback),
            );
        }
    }

    pub(crate) unsafe fn set_output_channel_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&str, ChannelData) + 'a,
    {
        unsafe {
            self.output_channel_cb = Some(Box::new(cb));
            csound_sys::csoundSetOutputChannelCallback(
                csound,
                Some(Trampoline::outputChannelCallback),
            );
        }
    }

    pub(crate) unsafe fn set_file_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&FileInfo) + 'a,
    {
        unsafe {
            self.file_open_cb = Some(Box::new(cb));
            csound_sys::csoundSetFileOpenCallback(csound, Some(Trampoline::fileOpenCallback));
        }
    }

    pub(crate) unsafe fn set_midi_in_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&str) + 'a,
    {
        unsafe {
            self.midi_in_open_cb = Some(Box::new(cb));
            csound_sys::csoundSetExternalMidiInOpenCallback(
                csound,
                Some(Trampoline::midiInOpenCallback),
            );
        }
    }

    pub(crate) unsafe fn set_midi_out_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&str) + 'a,
    {
        unsafe {
            self.midi_out_open_cb = Some(Box::new(cb));
            csound_sys::csoundSetExternalMidiOutOpenCallback(
                csound,
                Some(Trampoline::midiOutOpenCallback),
            );
        }
    }

    pub(crate) unsafe fn set_midi_read_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&mut [u8]) -> usize + 'a,
    {
        unsafe {
            self.midi_read_cb = Some(Box::new(cb));
            csound_sys::csoundSetExternalMidiReadCallback(
                csound,
                Some(Trampoline::midiReadCallback),
            );
        }
    }

    pub(crate) unsafe fn set_midi_write_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&[u8]) -> usize + 'a,
    {
        unsafe {
            self.midi_write_cb = Some(Box::new(cb));
            csound_sys::csoundSetExternalMidiWriteCallback(
                csound,
                Some(Trampoline::midiWriteCallback),
            );
        }
    }

    pub(crate) unsafe fn set_midi_in_close_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        unsafe {
            self.midi_in_close_cb = Some(Box::new(cb));
            csound_sys::csoundSetExternalMidiInCloseCallback(
                csound,
                Some(Trampoline::midiInCloseCallback),
            );
        }
    }

    pub(crate) unsafe fn set_midi_out_close_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        unsafe {
            self.midi_out_close_cb = Some(Box::new(cb));
            csound_sys::csoundSetExternalMidiOutCloseCallback(
                csound,
                Some(Trampoline::midiOutCloseCallback),
            );
        }
    }

    pub(crate) unsafe fn set_yield_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() -> bool + 'a,
    {
        unsafe {
            self.yield_cb = Some(Box::new(cb));
            csound_sys::csoundSetYieldCallback(csound, Some(Trampoline::yieldCallback));
        }
    }
}

pub mod Trampoline {
    use csound_sys as raw;

    use super::*;
    use crate::csound::CallbackHandler;
    use crate::rtaudio::{CsAudioDevice, RtAudioParams};
    use libc::{c_char, c_int, c_uchar, c_void, memcpy};
    use std::ffi::{CStr, CString};
    use std::slice;

    #[cfg(not(panic = "abort"))]
    use std::panic::{self, AssertUnwindSafe};

    pub fn ptr_to_string(ptr: *const c_char) -> Option<String> {
        if !ptr.is_null() {
            let result = match unsafe { CStr::from_ptr(ptr) }.to_str().ok() {
                Some(str_slice) => Some(str_slice.to_owned()),
                None => None,
            };
            return result;
        }
        None
    }

    /// Gets the callback handler from a csound instance.
    ///
    /// # Safety
    /// The csound pointer must be valid and have been created with a CallbackHandler as host data.
    #[inline]
    unsafe fn get_handler(csound: *mut raw::CSOUND) -> &'static mut CallbackHandler<'static> {
        unsafe { &mut *(raw::csoundGetHostData(csound) as *mut CallbackHandler) }
    }

    /// Panic-safe callback wrapper for callbacks returning a value.
    ///
    /// When `panic = "abort"` is set, this is a simple passthrough.
    /// Otherwise, it:
    /// 1. Checks if this callback has already panicked (early return with default)
    /// 2. Wraps the callback in `catch_unwind`
    /// 3. On panic: marks the callback as panicked, logs via tracing, returns default
    #[cfg(not(panic = "abort"))]
    fn catch_callback<T, F>(
        panic_state: &PanicState,
        flag: PanickedCallbacks,
        callback_name: &'static str,
        default: T,
        f: F,
    ) -> T
    where
        F: FnOnce() -> T,
    {
        // Check if this callback has already panicked
        if panic_state.has_panicked(flag) {
            tracing::warn!(
                callback = callback_name,
                "callback previously panicked, skipping invocation"
            );
            return default;
        }

        match panic::catch_unwind(AssertUnwindSafe(f)) {
            Ok(ret) => ret,
            Err(err) => {
                // Mark this callback as panicked
                panic_state.mark_panicked(flag);

                // Extract panic message if possible
                let panic_msg = if let Some(s) = err.downcast_ref::<&str>() {
                    *s
                } else if let Some(s) = err.downcast_ref::<String>() {
                    s.as_str()
                } else {
                    "unknown panic"
                };

                tracing::error!(
                    callback = callback_name,
                    panic_message = panic_msg,
                    "user callback panicked at FFI boundary"
                );

                default
            }
        }
    }

    /// Simplified callback wrapper when `panic = "abort"` is set.
    /// No catch_unwind needed since panics will abort anyway.
    #[cfg(panic = "abort")]
    #[inline]
    fn catch_callback<T, F>(
        _panic_state: &PanicState,
        _flag: PanickedCallbacks,
        _callback_name: &'static str,
        _default: T,
        f: F,
    ) -> T
    where
        F: FnOnce() -> T,
    {
        f()
    }

    /// Panic-safe callback wrapper for void-returning callbacks.
    #[cfg(not(panic = "abort"))]
    fn catch_callback_void<F>(
        panic_state: &PanicState,
        flag: PanickedCallbacks,
        callback_name: &'static str,
        f: F,
    ) where
        F: FnOnce(),
    {
        // Check if this callback has already panicked
        if panic_state.has_panicked(flag) {
            tracing::warn!(
                callback = callback_name,
                "callback previously panicked, skipping invocation"
            );
            return;
        }

        if let Err(err) = panic::catch_unwind(AssertUnwindSafe(f)) {
            // Mark this callback as panicked
            panic_state.mark_panicked(flag);

            // Extract panic message if possible
            let panic_msg = if let Some(s) = err.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.as_str()
            } else {
                "unknown panic"
            };

            tracing::error!(
                callback = callback_name,
                panic_message = panic_msg,
                "user callback panicked at FFI boundary"
            );
        }
    }

    /// Simplified void callback wrapper when `panic = "abort"` is set.
    #[cfg(panic = "abort")]
    #[inline]
    fn catch_callback_void<F>(
        _panic_state: &PanicState,
        _flag: PanickedCallbacks,
        _callback_name: &'static str,
        f: F,
    ) where
        F: FnOnce(),
    {
        f()
    }

    pub extern "C" fn message_string_cb(
        csound: *mut raw::CSOUND,
        attr: c_int,
        message: *const c_char,
    ) {
        let handler = unsafe { get_handler(csound) };
        catch_callback_void(
            &handler.panic_state,
            PanickedCallbacks::MESSAGE,
            "message_string_cb",
            || unsafe {
                let info = CStr::from_ptr(message);
                if let Ok(s) = info.to_str() {
                    if let Some(fun) = handler.callbacks.message_cb.as_mut() {
                        fun(MessageType::from(attr as u32), s);
                    }
                }
            },
        );
    }

    /****** real time audio callbacks functions *******************************************************************/

    pub extern "C" fn playOpenCallback(
        csound: *mut raw::CSOUND,
        dev: *const raw::csRtAudioParams,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::PLAY_OPEN,
            "playOpenCallback",
            CSOUND_STATUS::CSOUND_ERROR,
            || unsafe {
                let rt_params = RtAudioParams {
                    dev_name: ptr_to_string((*dev).devName),
                    dev_num: (*dev).devNum as u32,
                    buf_samp_sw: (*dev).bufSamp_SW as u32,
                    buf_samp_hw: (*dev).bufSamp_HW as u32,
                    n_channels: (*dev).nChannels as u32,
                    sample_format: (*dev).sampleFormat as u32,
                    sample_rate: (*dev).sampleRate as f32,
                };
                if let Some(fun) = handler.callbacks.play_open_cb.as_mut() {
                    return fun(&rt_params).to_i32() as c_int;
                }
                0
            },
        )
    }

    pub extern "C" fn recOpenCallback(
        csound: *mut raw::CSOUND,
        dev: *const raw::csRtAudioParams,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::REC_OPEN,
            "recOpenCallback",
            CSOUND_STATUS::CSOUND_ERROR,
            || unsafe {
                let rt_params = RtAudioParams {
                    dev_name: ptr_to_string((*dev).devName),
                    dev_num: (*dev).devNum as u32,
                    buf_samp_sw: (*dev).bufSamp_SW as u32,
                    buf_samp_hw: (*dev).bufSamp_HW as u32,
                    n_channels: (*dev).nChannels as u32,
                    sample_format: (*dev).sampleFormat as u32,
                    sample_rate: (*dev).sampleRate as f32,
                };
                if let Some(fun) = handler.callbacks.rec_open_cb.as_mut() {
                    return fun(&rt_params).to_i32() as c_int;
                }
                -1
            },
        )
    }

    pub extern "C" fn rtcloseCallback(csound: *mut raw::CSOUND) {
        let handler = unsafe { get_handler(csound) };
        catch_callback_void(
            &handler.panic_state,
            PanickedCallbacks::RT_CLOSE,
            "rtcloseCallback",
            || {
                if let Some(fun) = handler.callbacks.rt_close_cb.as_mut() {
                    fun();
                }
            },
        );
    }

    pub extern "C" fn rtplayCallback(csound: *mut raw::CSOUND, outBuf: *const f64, nbytes: c_int) {
        let handler = unsafe { get_handler(csound) };
        catch_callback_void(
            &handler.panic_state,
            PanickedCallbacks::RT_PLAY,
            "rtplayCallback",
            || unsafe {
                let out = slice::from_raw_parts(outBuf, nbytes as usize);
                if let Some(fun) = handler.callbacks.rt_play_cb.as_mut() {
                    fun(out);
                }
            },
        );
    }

    pub extern "C" fn rtrecordCallback(
        csound: *mut raw::CSOUND,
        outBuf: *mut f64,
        nbytes: c_int,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::RT_REC,
            "rtrecordCallback",
            -1,
            || unsafe {
                let buff = slice::from_raw_parts_mut(outBuf, nbytes as usize);
                if let Some(fun) = handler.callbacks.rt_rec_cb.as_mut() {
                    return fun(buff) as c_int;
                }
                -1
            },
        )
    }

    pub extern "C" fn audioDeviceListCallback(
        csound: *mut raw::CSOUND,
        dev: *mut raw::CS_AUDIODEVICE,
        is_output: c_int,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::DEVLIST,
            "audioDeviceListCallback",
            0,
            || unsafe {
                let audio_device = CsAudioDevice {
                    device_name: ptr_to_string((*dev).device_name.as_ptr()),
                    device_id: ptr_to_string((*dev).device_id.as_ptr()),
                    rt_module: ptr_to_string((*dev).rt_module.as_ptr()),
                    max_nchnls: (*dev).max_nchnls as u32,
                    is_output: is_output as u32,
                };
                if let Some(fun) = handler.callbacks.devlist_cb.as_mut() {
                    fun(audio_device);
                }
                0
            },
        )
    }

    /********* General Input/Output callbacks ********************************************************************/

    pub extern "C" fn fileOpenCallback(
        csound: *mut raw::CSOUND,
        filePath: *const c_char,
        fileType: c_int,
        operation: c_int,
        isTemp: c_int,
    ) {
        let handler = unsafe { get_handler(csound) };
        catch_callback_void(
            &handler.panic_state,
            PanickedCallbacks::FILE_OPEN,
            "fileOpenCallback",
            || {
                let name = ptr_to_string(filePath);
                let file_info = FileInfo {
                    name,
                    file_type: FileTypes::from(fileType as u8),
                    is_writing: operation != 0,
                    is_temp: isTemp != 0,
                };
                if let Some(fun) = handler.callbacks.file_open_cb.as_mut() {
                    fun(&file_info);
                }
            },
        );
    }

    /* Channels and events callbacks **************************************************** */

    pub extern "C" fn inputChannelCallback(
        csound: *mut raw::CSOUND,
        channelName: *const c_char,
        channelValuePtr: *mut c_void,
        _channelType: *const c_void,
    ) {
        let handler = unsafe { get_handler(csound) };
        catch_callback_void(
            &handler.panic_state,
            PanickedCallbacks::INPUT_CHANNEL,
            "inputChannelCallback",
            || unsafe {
                let name = match CStr::from_ptr(channelName).to_str() {
                    Ok(s) => s,
                    Err(_) => return,
                };

                let result = if let Some(fun) = handler.callbacks.input_channel_cb.as_mut() {
                    fun(name)
                } else {
                    return;
                };

                match result {
                    ChannelData::Control(data) => {
                        *(channelValuePtr as *mut f64) = data;
                    }
                    ChannelData::String(s) => {
                        let len = s.len();
                        if let Ok(c_str) = CString::new(s) {
                            if raw::csoundGetChannelDatasize(csound, channelName) as usize <= len {
                                memcpy(channelValuePtr, c_str.as_ptr() as *mut c_void, len);
                            }
                        }
                    }
                    _ => {}
                }
            },
        );
    }

    pub extern "C" fn outputChannelCallback(
        csound: *mut raw::CSOUND,
        channelName: *const c_char,
        channelValuePtr: *mut c_void,
        _channelType: *const c_void,
    ) {
        let handler = unsafe { get_handler(csound) };
        catch_callback_void(
            &handler.panic_state,
            PanickedCallbacks::OUTPUT_CHANNEL,
            "outputChannelCallback",
            || unsafe {
                let name = match CStr::from_ptr(channelName).to_str() {
                    Ok(s) => s,
                    Err(_) => return,
                };

                let mut ptr = ::std::ptr::null_mut();
                let ptr: *mut *mut c_void = &mut ptr as *mut *mut _;
                let channel_type = raw::csoundGetChannelPtr(csound, ptr, channelName, 0);
                let channel_type =
                    channel_type & controlChannelType::CSOUND_CHANNEL_TYPE_MASK as i32;

                let fun = if let Some(fun) = handler.callbacks.output_channel_cb.as_mut() {
                    fun
                } else {
                    return;
                };

                match channel_type as u32 {
                    controlChannelType::CSOUND_CONTROL_CHANNEL => {
                        let value = *(channelValuePtr as *mut f64);
                        let data = ChannelData::Control(value);
                        fun(name, data);
                    }
                    controlChannelType::CSOUND_STRING_CHANNEL => {
                        let data = ChannelData::String(
                            ptr_to_string(channelValuePtr as *const c_char)
                                .unwrap_or_else(|| "".to_owned()),
                        );
                        fun(name, data);
                    }
                    _ => {}
                }
            },
        );
    }

    /****** MIDI I/O callbacks functions *******************************************************************/

    pub extern "C" fn midiInOpenCallback(
        csound: *mut raw::CSOUND,
        _user_data: *mut *mut c_void,
        dev_name: *const c_char,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::MIDI_IN_OPEN,
            "midiInOpenCallback",
            CSOUND_STATUS::CSOUND_ERROR,
            || unsafe {
                let name = match CStr::from_ptr(dev_name).to_str() {
                    Ok(s) => s,
                    Err(_) => return CSOUND_STATUS::CSOUND_ERROR,
                };
                if let Some(fun) = handler.callbacks.midi_in_open_cb.as_mut() {
                    fun(name);
                }
                CSOUND_STATUS::CSOUND_SUCCESS
            },
        )
    }

    pub extern "C" fn midiOutOpenCallback(
        csound: *mut raw::CSOUND,
        _user_data: *mut *mut c_void,
        dev_name: *const c_char,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::MIDI_OUT_OPEN,
            "midiOutOpenCallback",
            CSOUND_STATUS::CSOUND_ERROR,
            || unsafe {
                let name = match CStr::from_ptr(dev_name).to_str() {
                    Ok(s) => s,
                    Err(_) => return CSOUND_STATUS::CSOUND_ERROR,
                };
                if let Some(fun) = handler.callbacks.midi_out_open_cb.as_mut() {
                    fun(name);
                }
                CSOUND_STATUS::CSOUND_SUCCESS
            },
        )
    }

    pub extern "C" fn midiReadCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
        buf: *mut c_uchar,
        nbytes: c_int,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::MIDI_READ,
            "midiReadCallback",
            -1,
            || unsafe {
                let out = slice::from_raw_parts_mut(buf, nbytes as usize);
                if let Some(fun) = handler.callbacks.midi_read_cb.as_mut() {
                    return fun(out) as c_int;
                }
                -1
            },
        )
    }

    #[allow(dead_code)]
    pub extern "C" fn midiWriteCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
        buf: *const u8,
        nbytes: c_int,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::MIDI_WRITE,
            "midiWriteCallback",
            -1,
            || unsafe {
                let buffer = slice::from_raw_parts(buf, nbytes as usize);
                if let Some(fun) = handler.callbacks.midi_write_cb.as_mut() {
                    return fun(buffer) as c_int;
                }
                -1
            },
        )
    }

    pub extern "C" fn midiInCloseCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::MIDI_IN_CLOSE,
            "midiInCloseCallback",
            CSOUND_STATUS::CSOUND_SUCCESS, // Close should succeed even on panic
            || {
                if let Some(fun) = handler.callbacks.midi_in_close_cb.as_mut() {
                    fun();
                }
                CSOUND_STATUS::CSOUND_SUCCESS
            },
        )
    }

    pub extern "C" fn midiOutCloseCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
    ) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::MIDI_OUT_CLOSE,
            "midiOutCloseCallback",
            CSOUND_STATUS::CSOUND_SUCCESS, // Close should succeed even on panic
            || {
                if let Some(fun) = handler.callbacks.midi_out_close_cb.as_mut() {
                    fun();
                }
                CSOUND_STATUS::CSOUND_SUCCESS
            },
        )
    }

    pub extern "C" fn yieldCallback(csound: *mut raw::CSOUND) -> c_int {
        let handler = unsafe { get_handler(csound) };
        catch_callback(
            &handler.panic_state,
            PanickedCallbacks::YIELD,
            "yieldCallback",
            1, // Signal stop on panic
            || {
                if let Some(fun) = handler.callbacks.yield_cb.as_mut() {
                    return fun() as c_int;
                }
                0
            },
        )
    }
}

//Sets callback for converting MIDI error codes to strings.
/*pub extern fn pub externalMidiErrorStringCallback (midi_error_code : c_int) -> *const c_char {
    unsafe{
    }
}*/
