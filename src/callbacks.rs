use libc::c_void;

use crate::enums::{ChannelData, FileTypes, MessageType, Status};
use crate::rtaudio::{CsAudioDevice, RtAudioParams};

use csound_sys as raw;

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
    pub sense_event_cb: Option<Box<dyn FnMut() + 'a>>,
    pub keyboard_cb: Option<Box<dyn FnMut() -> char + 'a>>, // TODO this callback doesn't work at the
    //csound side
    pub rt_close_cb: Option<Box<dyn FnMut() + 'a>>,
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
        self.message_cb = Some(Box::new(cb));
        raw::csoundSetMessageStringCallback(csound, Trampoline::message_string_cb)
    }

    pub(crate) unsafe fn set_devlist_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(CsAudioDevice) + 'a,
    {
        self.devlist_cb = Some(Box::new(cb));
        raw::csoundSetAudioDeviceListCallback(csound, Some(Trampoline::audioDeviceListCallback));
    }

    pub(crate) unsafe fn set_play_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'a,
    {
        self.play_open_cb = Some(Box::new(cb));
        raw::csoundSetPlayopenCallback(csound, Some(Trampoline::playOpenCallback));
    }

    pub(crate) unsafe fn set_rec_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'a,
    {
        self.play_open_cb = Some(Box::new(cb));
        raw::csoundSetRecopenCallback(csound, Some(Trampoline::recOpenCallback));
    }

    pub(crate) unsafe fn set_rt_play_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&[f64]) + 'a,
    {
        self.rt_play_cb = Some(Box::new(cb));
        csound_sys::csoundSetRtplayCallback(csound, Some(Trampoline::rtplayCallback));
    }

    pub(crate) unsafe fn set_rt_rec_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&mut [f64]) -> usize + 'a,
    {
        self.rt_rec_cb = Some(Box::new(cb));
        csound_sys::csoundSetRtrecordCallback(csound, Some(Trampoline::rtrecordCallback));
    }

    pub(crate) unsafe fn set_rt_close_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        self.rt_close_cb = Some(Box::new(cb));
        csound_sys::csoundSetRtcloseCallback(csound, Some(Trampoline::rtcloseCallback));
    }

    pub(crate) unsafe fn set_sense_event_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        self.sense_event_cb = Some(Box::new(cb));
        csound_sys::csoundRegisterSenseEventCallback(
            csound,
            Some(Trampoline::senseEventCallback),
            ::std::ptr::null_mut() as *mut c_void,
        );
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
        self.input_channel_cb = Some(Box::new(cb));
        csound_sys::csoundSetInputChannelCallback(csound, Some(Trampoline::inputChannelCallback));
    }

    pub(crate) unsafe fn set_output_channel_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&str, ChannelData) + 'a,
    {
        self.output_channel_cb = Some(Box::new(cb));
        csound_sys::csoundSetOutputChannelCallback(csound, Some(Trampoline::outputChannelCallback));
    }

    pub(crate) unsafe fn set_file_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&FileInfo) + 'a,
    {
        self.file_open_cb = Some(Box::new(cb));
        csound_sys::csoundSetFileOpenCallback(csound, Some(Trampoline::fileOpenCallback));
    }

    pub(crate) unsafe fn set_midi_in_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&str) + 'a,
    {
        self.midi_in_open_cb = Some(Box::new(cb));
        csound_sys::csoundSetExternalMidiInOpenCallback(
            csound,
            Some(Trampoline::midiInOpenCallback),
        );
    }

    pub(crate) unsafe fn set_midi_out_open_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&str) + 'a,
    {
        self.midi_out_open_cb = Some(Box::new(cb));
        csound_sys::csoundSetExternalMidiOutOpenCallback(
            csound,
            Some(Trampoline::midiOutOpenCallback),
        );
    }

    pub(crate) unsafe fn set_midi_read_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&mut [u8]) -> usize + 'a,
    {
        self.midi_read_cb = Some(Box::new(cb));
        csound_sys::csoundSetExternalMidiReadCallback(csound, Some(Trampoline::midiReadCallback));
    }

    pub(crate) unsafe fn set_midi_write_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut(&[u8]) -> usize + 'a,
    {
        self.midi_write_cb = Some(Box::new(cb));
        csound_sys::csoundSetExternalMidiWriteCallback(csound, Some(Trampoline::midiWriteCallback));
    }

    pub(crate) unsafe fn set_midi_in_close_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        self.midi_in_close_cb = Some(Box::new(cb));
        csound_sys::csoundSetExternalMidiInCloseCallback(
            csound,
            Some(Trampoline::midiInCloseCallback),
        );
    }

    pub(crate) unsafe fn set_midi_out_close_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() + 'a,
    {
        self.midi_out_close_cb = Some(Box::new(cb));
        csound_sys::csoundSetExternalMidiOutCloseCallback(
            csound,
            Some(Trampoline::midiOutCloseCallback),
        );
    }

    pub(crate) unsafe fn set_yield_cb<F>(&'a mut self, csound: *mut raw::CSOUND, cb: F)
    where
        F: FnMut() -> bool + 'a,
    {
        self.yield_cb = Some(Box::new(cb));
        csound_sys::csoundSetYieldCallback(csound, Some(Trampoline::yieldCallback));
    }
}

pub mod Trampoline {

    use csound_sys as raw;
    use va_list::VaList;

    use super::*;
    use crate::csound::CallbackHandler;
    use crate::rtaudio::{CsAudioDevice, RtAudioParams};
    use libc::{c_char, c_int, c_uchar, c_void, memcpy};
    use std::ffi::{CStr, CString};
    use std::panic::{self, AssertUnwindSafe};
    use std::slice;

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

    pub fn convert_str_to_c<T>(string: T) -> Result<CString, &'static str>
    where
        T: AsRef<str>,
    {
        let string = string.as_ref();
        if string.is_empty() {
            return Err("Failed to convert empty string to C");
        }
        CString::new(string).map_err(|_| "Failed converting rust string to CString")
    }

    fn catch<T, F: FnOnce() -> T>(f: F) -> Option<T> {
        match panic::catch_unwind(AssertUnwindSafe(f)) {
            Ok(ret) => Some(ret),
            Err(_) => {
                std::process::exit(-1);
            }
        }
    }

    pub extern "C" fn default_message_callback(
        _csound: *mut raw::CSOUND,
        _attr: c_int,
        _format: *const c_char,
        _args: VaList,
    ) {
    }

    pub extern "C" fn message_string_cb(
        csound: *mut raw::CSOUND,
        attr: c_int,
        message: *const c_char,
    ) {
        catch(|| unsafe {
            let info = CStr::from_ptr(message);
            if let Ok(s) = info.to_str() {
                if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                    .callbacks
                    .message_cb
                    .as_mut()
                {
                    fun(MessageType::from(attr as u32), s);
                }
            }
        });
    }

    /****** Event callbacks functions *******************************************************************/

    pub extern "C" fn senseEventCallback(csound: *mut raw::CSOUND, _userData: *mut c_void) {
        catch(|| unsafe {
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .sense_event_cb
                .as_mut()
            {
                fun();
            }
        });
    }

    /****** real time audio callbacks functions *******************************************************************/

    pub extern "C" fn playOpenCallback(
        csound: *mut raw::CSOUND,
        dev: *const raw::csRtAudioParams,
    ) -> c_int {
        catch(|| unsafe {
            let rtParams = RtAudioParams {
                devName: ptr_to_string((*dev).devName),
                devNum: (*dev).devNum as u32,
                bufSamp_SW: (*dev).bufSamp_SW as u32,
                bufSamp_HW: (*dev).bufSamp_HW as u32,
                nChannels: (*dev).nChannels as u32,
                sampleFormat: (*dev).sampleFormat as u32,
                sampleRate: (*dev).sampleRate as f32,
            };
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .play_open_cb
                .as_mut()
            {
                return fun(&rtParams).to_i32() as c_int;
            }
            0
        })
        .unwrap()
    }

    pub extern "C" fn recOpenCallback(
        csound: *mut raw::CSOUND,
        dev: *const raw::csRtAudioParams,
    ) -> c_int {
        catch(|| unsafe {
            let rtParams = RtAudioParams {
                devName: ptr_to_string((*dev).devName),
                devNum: (*dev).devNum as u32,
                bufSamp_SW: (*dev).bufSamp_SW as u32,
                bufSamp_HW: (*dev).bufSamp_HW as u32,
                nChannels: (*dev).nChannels as u32,
                sampleFormat: (*dev).sampleFormat as u32,
                sampleRate: (*dev).sampleRate as f32,
            };
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .rec_open_cb
                .as_mut()
            {
                return fun(&rtParams).to_i32() as c_int;
            }
            -1
        })
        .unwrap()
    }

    pub extern "C" fn rtcloseCallback(csound: *mut raw::CSOUND) {
        catch(|| unsafe {
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .rt_close_cb
                .as_mut()
            {
                fun();
            }
        });
    }

    pub extern "C" fn rtplayCallback(csound: *mut raw::CSOUND, outBuf: *const f64, nbytes: c_int) {
        catch(|| unsafe {
            let out = slice::from_raw_parts(outBuf, nbytes as usize);
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .rt_play_cb
                .as_mut()
            {
                fun(&out);
            }
        });
    }

    pub extern "C" fn rtrecordCallback(
        csound: *mut raw::CSOUND,
        outBuf: *mut f64,
        nbytes: c_int,
    ) -> c_int {
        catch(|| unsafe {
            let mut buff = slice::from_raw_parts_mut(outBuf, nbytes as usize);
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .rt_rec_cb
                .as_mut()
            {
                return fun(&mut buff) as c_int;
            }
            -1
        })
        .unwrap()
    }

    pub extern "C" fn audioDeviceListCallback(
        csound: *mut raw::CSOUND,
        dev: *mut raw::CS_AUDIODEVICE,
        isOutput: c_int,
    ) -> c_int {
        catch(|| unsafe {
            let audioDevice = CsAudioDevice {
                device_name: ptr_to_string((*dev).device_name.as_ptr()),
                device_id: ptr_to_string((*dev).device_id.as_ptr()),
                rt_module: ptr_to_string((*dev).rt_module.as_ptr()),
                max_nchnls: (*dev).max_nchnls as u32,
                isOutput: isOutput as u32,
            };
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .devlist_cb
                .as_mut()
            {
                fun(audioDevice);
            }
            0
        })
        .unwrap()
    }

    /*pub extern "C" fn keyboard_callback(
        userData: *mut c_void,
        p: *mut c_void,
        _type_: c_uint,
    ) -> c_int {
        unsafe {
            match (*(userData as *mut CallbackHandler))
                .callbacks
                .keyboard_cb() {
                '\0' => {}
                value => {
                    *(p as *mut c_int) = value as c_int;
                }
            }
            0
        }
    }*/

    /********* General Input/Output callbacks ********************************************************************/
    pub extern "C" fn fileOpenCallback(
        csound: *mut raw::CSOUND,
        filePath: *const c_char,
        fileType: c_int,
        operation: c_int,
        isTemp: c_int,
    ) {
        catch(|| unsafe {
            let name = ptr_to_string(filePath);
            let file_info = FileInfo {
                name,
                file_type: FileTypes::from(fileType as u8),
                is_writing: operation != 0,
                is_temp: isTemp != 0,
            };
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .file_open_cb
                .as_mut()
            {
                fun(&file_info);
            }
        });
    }

    /* Score Handling callbacks ********************************************************* */

    // Sets an pub external callback for Cscore processing. Pass NULL to reset to the internal cscore() function (which does nothing).
    // This callback is retained after a csoundReset() call.
    /*pub extern "C" fn scoreCallback(csound: *mut raw::CSOUND) {
        catch(|| unsafe {
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .cscore_cb
                .as_mut()
            {
                fun();
            }
        });
    }*/

    /* Channels and events callbacks **************************************************** */

    pub extern "C" fn inputChannelCallback(
        csound: *mut raw::CSOUND,
        channelName: *const c_char,
        channelValuePtr: *mut c_void,
        _channelType: *const c_void,
    ) {
        catch(|| unsafe {
            let name = (CStr::from_ptr(channelName)).to_str();
            if name.is_err() {
                return;
            }
            let name = name.unwrap();
            let result = if let Some(fun) = (*(raw::csoundGetHostData(csound)
                as *mut CallbackHandler))
                .callbacks
                .input_channel_cb
                .as_mut()
            {
                fun(name)
            } else {
                return;
            };

            match result {
                ChannelData::CS_CONTROL_CHANNEL(data) => {
                    *(channelValuePtr as *mut f64) = data;
                }

                ChannelData::CS_STRING_CHANNEL(s) => {
                    let len = s.len();
                    let c_str = CString::new(s);
                    if raw::csoundGetChannelDatasize(csound, channelName) as usize <= len {
                        if let Ok(ptr) = c_str {
                            memcpy(channelValuePtr, ptr.as_ptr() as *mut c_void, len);
                        }
                    }
                }

                _ => {}
            }
        });
    }

    pub extern "C" fn outputChannelCallback(
        csound: *mut raw::CSOUND,
        channelName: *const c_char,
        channelValuePtr: *mut c_void,
        _channelType: *const c_void,
    ) {
        catch(|| unsafe {
            let name = (CStr::from_ptr(channelName)).to_str();
            if name.is_err() {
                return;
            }
            let name = name.unwrap();
            let mut ptr = ::std::ptr::null_mut();
            let ptr: *mut *mut f64 = &mut ptr as *mut *mut _;
            let channel_type = raw::csoundGetChannelPtr(csound, ptr, channelName, 0) as u32;
            let channel_type = channel_type & raw::CSOUND_CHANNEL_TYPE_MASK as u32;

            let fun = if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .output_channel_cb
                .as_mut()
            {
                fun
            } else {
                return;
            };

            match channel_type {
                raw::CSOUND_CONTROL_CHANNEL => {
                    let value = *(channelValuePtr as *mut f64);
                    let data = ChannelData::CS_CONTROL_CHANNEL(value);
                    fun(name, data);
                }

                raw::CSOUND_STRING_CHANNEL => {
                    let data = ChannelData::CS_STRING_CHANNEL(
                        ptr_to_string(channelValuePtr as *const c_char)
                            .unwrap_or_else(|| "".to_owned()),
                    );
                    fun(name, data);
                }

                _ => {}
            }
        });
    }

    /****** MIDI I/O callbacks functions *******************************************************************/

    // Sets callback for opening real time MIDI input.
    pub extern "C" fn midiInOpenCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut *mut c_void,
        devName: *const c_char,
    ) -> c_int {
        catch(|| unsafe {
            let name = match CStr::from_ptr(devName).to_str() {
                Ok(s) => s,
                _ => return raw::CSOUND_ERROR,
            };
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .midi_in_open_cb
                .as_mut()
            {
                fun(&name);
            }
            raw::CSOUND_SUCCESS
        })
        .unwrap()
    }

    // Sets callback for opening real time MIDI output.
    pub extern "C" fn midiOutOpenCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut *mut c_void,
        devName: *const c_char,
    ) -> c_int {
        catch(|| unsafe {
            let name = match CStr::from_ptr(devName).to_str() {
                Ok(s) => s,
                _ => return raw::CSOUND_ERROR,
            };
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .midi_out_open_cb
                .as_mut()
            {
                fun(&name);
            }
            raw::CSOUND_SUCCESS
        })
        .unwrap()
    }

    // Sets callback for reading from real time MIDI input.
    pub extern "C" fn midiReadCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
        buf: *mut c_uchar,
        nbytes: c_int,
    ) -> c_int {
        catch(|| unsafe {
            let mut out = slice::from_raw_parts_mut(buf, nbytes as usize);
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .midi_read_cb
                .as_mut()
            {
                return fun(&mut out) as c_int;
            }
            -1
        })
        .unwrap()
    }

    // Sets callback for writing to real time MIDI output.
    #[allow(dead_code)]
    pub extern "C" fn midiWriteCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
        buf: *const u8,
        nbytes: c_int,
    ) -> c_int {
        catch(|| unsafe {
            let buffer = slice::from_raw_parts(buf, nbytes as usize);
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .midi_write_cb
                .as_mut()
            {
                return fun(&buffer) as c_int;
            }
            -1
        })
        .unwrap()
    }

    //Sets callback for closing real time MIDI input.
    pub extern "C" fn midiInCloseCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
    ) -> c_int {
        catch(|| unsafe {
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .midi_in_close_cb
                .as_mut()
            {
                fun();
            }
            raw::CSOUND_SUCCESS
        })
        .unwrap()
    }

    // Sets callback for closing real time MIDI output.
    pub extern "C" fn midiOutCloseCallback(
        csound: *mut raw::CSOUND,
        _userData: *mut c_void,
    ) -> c_int {
        catch(|| unsafe {
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .midi_out_close_cb
                .as_mut()
            {
                fun();
            }
            raw::CSOUND_SUCCESS
        })
        .unwrap()
    }

    pub extern "C" fn yieldCallback(csound: *mut raw::CSOUND) -> c_int {
        catch(|| unsafe {
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .yield_cb
                .as_mut()
            {
                return fun() as c_int;
            }
            0
        })
        .unwrap()
    }
}

//Sets callback for converting MIDI error codes to strings.
/*pub extern fn pub externalMidiErrorStringCallback (midi_error_code : c_int) -> *const c_char {
    unsafe{
    }
}*/
