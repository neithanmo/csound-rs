use enums::{ChannelData, FileTypes, MessageType, Status};
use rtaudio::{CsAudioDevice, RtAudioParams};

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub file_type: FileTypes,
    pub is_writing: bool,
    pub is_temp: bool,
}

#[doc(hidden)]
#[derive(Default)]
pub struct Callbacks<'a> {
    pub message_cb: Option<Box<FnMut(MessageType, &str) + 'a>>,
    pub audio_dev_list_cb: Option<Box<FnMut(CsAudioDevice) + 'a>>,
    pub play_open_cb: Option<Box<FnMut(&RtAudioParams) -> Status + 'a>>,
    pub rec_open_cb: Option<Box<FnMut(&RtAudioParams) -> Status + 'a>>,
    pub rt_play_cb: Option<Box<FnMut(&[f64]) + 'a>>,
    pub rt_rec_cb: Option<Box<FnMut(&mut [f64]) -> usize + 'a>>,
    pub sense_event_cb: Option<Box<FnMut() + 'a>>,
    pub keyboard_cb: Option<Box<FnMut() -> char + 'a>>, // TODO this callback doesn't work at the
    //csound side
    pub rt_close_cb: Option<Box<FnMut() + 'a>>,
    pub cscore_cb: Option<Box<FnMut() + 'a>>,
    pub input_channel_cb: Option<Box<FnMut(&str) -> ChannelData + 'a>>,
    pub output_channel_cb: Option<Box<FnMut(&str, ChannelData) + 'a>>,
    pub file_open_cb: Option<Box<FnMut(&FileInfo) + 'a>>,
    pub midi_in_open_cb: Option<Box<FnMut(&str) + 'a>>,
    pub midi_out_open_cb: Option<Box<FnMut(&str) + 'a>>,
    pub midi_read_cb: Option<Box<FnMut(&mut [u8]) -> usize + 'a>>,
    pub midi_write_cb: Option<Box<FnMut(&[u8]) -> usize + 'a>>,
    pub midi_in_close_cb: Option<Box<FnMut() + 'a>>,
    pub midi_out_close_cb: Option<Box<FnMut() + 'a>>,
    pub yield_cb: Option<Box<FnMut() -> bool + 'a>>,
}

pub const MESSAGE_CB: u32 = 1;
pub const SENSE_EVENT: u32 = 2;
pub const PLAY_OPEN: u32 = 3;
pub const REC_OPEN: u32 = 4;
pub const REAL_TIME_PLAY: u32 = 6;
pub const REAL_TIME_REC: u32 = 7;
pub const AUDIO_DEV_LIST: u32 = 9;
//pub const KEYBOARD_CB: u32 = 10;
pub const RT_CLOSE_CB: u32 = 11;
pub const CSCORE_CB: u32 = 12;
pub const CHANNEL_INPUT_CB: u32 = 13;
pub const CHANNEL_OUTPUT_CB: u32 = 14;
pub const FILE_OPEN_CB: u32 = 15;
pub const MIDI_IN_OPEN_CB: u32 = 16;
pub const MIDI_OUT_OPEN_CB: u32 = 17;
pub const MIDI_READ_CB: u32 = 18;
pub const MIDI_WRITE_CB: u32 = 19;
pub const MIDI_IN_CLOSE: u32 = 20;
pub const MIDI_OUT_CLOSE: u32 = 21;
pub const YIELD_CB: u32 = 22;

pub mod Trampoline {

    use std::panic::{self, AssertUnwindSafe};
    pub extern crate csound_sys as raw;
    use super::*;
    use csound::CallbackHandler;
    use libc::{c_char, c_int, c_uchar, /*c_uint,*/ c_void, memcpy};
    use rtaudio::{CsAudioDevice, RtAudioParams};
    use std::ffi::{CStr, CString};
    use std::slice;

    fn catch<T, F: FnOnce() -> T>(f: F) -> Option<T> {
        match panic::catch_unwind(AssertUnwindSafe(f)) {
            Ok(ret) => Some(ret),
            Err(_) => {
                std::process::exit(-1);
            }
        }
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
                    fun(MessageType::from_u32(attr as u32), s);
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
            let name = (CStr::from_ptr((*dev).devName)).to_owned();
            let rtParams = RtAudioParams {
                devName: name.into_string().unwrap(),
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
            let name = (CStr::from_ptr((*dev).devName)).to_owned();
            let rtParams = RtAudioParams {
                devName: name.into_string().unwrap(),
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
            let name = (CStr::from_ptr((*dev).device_name.as_ptr())).to_owned();
            let id = (CStr::from_ptr((*dev).device_id.as_ptr())).to_owned();
            let module = (CStr::from_ptr((*dev).rt_module.as_ptr())).to_owned();
            let audioDevice = CsAudioDevice {
                device_name: name.into_string().unwrap(),
                device_id: id.into_string().unwrap(),
                rt_module: module.into_string().unwrap(),
                max_nchnls: (*dev).max_nchnls as u32,
                isOutput: isOutput as u32,
            };
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .audio_dev_list_cb
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
            let path = (CStr::from_ptr(filePath)).to_owned();
            let path = path.into_string().unwrap_or_else(|_| "".to_string());

            let file_info = FileInfo {
                name: path,
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
    pub extern "C" fn scoreCallback(csound: *mut raw::CSOUND) {
        catch(|| unsafe {
            if let Some(fun) = (*(raw::csoundGetHostData(csound) as *mut CallbackHandler))
                .callbacks
                .cscore_cb
                .as_mut()
            {
                fun();
            }
        });
    }

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
                    if raw::csoundGetChannelDatasize(csound, channelName) as usize <= len
                        && c_str.is_ok()
                    {
                        memcpy(channelValuePtr, c_str.unwrap().as_ptr() as *mut c_void, len);
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
                    let mut string = CStr::from_ptr(channelValuePtr as *const c_char).to_str();
                    if string.is_err() {
                        return;
                    }
                    let string = string.unwrap().to_string();
                    let data = ChannelData::CS_STRING_CHANNEL(string);
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
