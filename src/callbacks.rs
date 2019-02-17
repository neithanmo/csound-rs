extern crate csound_sys as raw;

use std::ffi::{CStr, CString};
//use std::panic;
use std::slice;

use csound::{Engine, Inner};
use enums::{ChannelData, FileTypes, MessageType};
use handler::Handler;
use libc::{c_char, c_int, c_uchar, c_uint, c_void, memcpy};
use rtaudio::{CS_AudioDevice, RT_AudioParams};

pub const MESSAGE_CB: u32 = 1;
pub const SENSE_EVENT: u32 = 2;
pub const PLAY_OPEN: u32 = 3;
pub const REC_OPEN: u32 = 4;
pub const REAL_TIME_PLAY: u32 = 6;
pub const REAL_TIME_REC: u32 = 7;
pub const AUDIO_DEV_LIST: u32 = 9;
pub const KEYBOARD_CB: u32 = 10;
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

/// Struct containing the relevant info of files opened by csound.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// pathname of the file; either full or relative to current dir
    pub name: String,
    /// Enum equivalent code for the file type code from the enum CSOUND_FILETYPES
    pub file_type: FileTypes,
    /// true if Csound is writing the file, false if reading
    pub is_writing: bool,
    /// true if  it is a temporary file that Csound will delete; false if not
    pub is_temp: bool,
}

pub trait CsoundCallbacks {
    fn enable_callback(&mut self, callback_type: u32);
}

impl<H> CsoundCallbacks for Engine<H>
where
    H: Handler,
{
    fn enable_callback(&mut self, callback_type: u32) {
        match callback_type {
            SENSE_EVENT => unsafe {
                raw::csoundRegisterSenseEventCallback(
                    self.inner.csound,
                    Some(senseEventCallback::<H>),
                    ::std::ptr::null_mut() as *mut c_void,
                );
            },
            MESSAGE_CB => unsafe {
                raw::csoundSetMessageStringCallback(self.inner.csound, message_string_cb::<H>)
            },

            AUDIO_DEV_LIST => unsafe {
                raw::csoundSetAudioDeviceListCallback(
                    self.inner.csound,
                    Some(audioDeviceListCallback::<H>),
                );
            },
            PLAY_OPEN => unsafe {
                raw::csoundSetPlayopenCallback(self.inner.csound, Some(playOpenCallback::<H>));
            },
            REC_OPEN => unsafe {
                raw::csoundSetRecopenCallback(self.inner.csound, Some(recOpenCallback::<H>));
            },

            REAL_TIME_PLAY => unsafe {
                raw::csoundSetRtplayCallback(self.inner.csound, Some(rtplayCallback::<H>));
            },

            REAL_TIME_REC => unsafe {
                raw::csoundSetRtrecordCallback(self.inner.csound, Some(rtrecordCallback::<H>));
            },

            KEYBOARD_CB => unsafe {
                let host_data_ptr = &*self.inner as *const _ as *const _;
                raw::csoundRegisterKeyboardCallback(
                    self.inner.csound,
                    Some(keyboard_callback::<H>),
                    host_data_ptr as *mut c_void,
                    raw::CSOUND_CALLBACK_KBD_EVENT | raw::CSOUND_CALLBACK_KBD_TEXT,
                );
                raw::csoundKeyPress(self.inner.csound, '\n' as i8);
            },

            RT_CLOSE_CB => unsafe {
                raw::csoundSetRtcloseCallback(self.inner.csound, Some(rtcloseCallback::<H>));
            },

            CSCORE_CB => unsafe {
                raw::csoundSetCscoreCallback(self.inner.csound, Some(scoreCallback::<H>));
            },

            CHANNEL_INPUT_CB => unsafe {
                raw::csoundSetInputChannelCallback(
                    self.inner.csound,
                    Some(inputChannelCallback::<H>),
                );
            },

            CHANNEL_OUTPUT_CB => unsafe {
                raw::csoundSetOutputChannelCallback(
                    self.inner.csound,
                    Some(outputChannelCallback::<H>),
                );
            },

            FILE_OPEN_CB => unsafe {
                raw::csoundSetFileOpenCallback(self.inner.csound, Some(fileOpenCallback::<H>));
            },

            MIDI_IN_OPEN_CB => unsafe {
                raw::csoundSetExternalMidiInOpenCallback(
                    self.inner.csound,
                    Some(midiInOpenCallback::<H>),
                );
            },

            MIDI_OUT_OPEN_CB => unsafe {
                raw::csoundSetExternalMidiOutOpenCallback(
                    self.inner.csound,
                    Some(midiOutOpenCallback::<H>),
                );
            },

            MIDI_READ_CB => unsafe {
                raw::csoundSetExternalMidiReadCallback(
                    self.inner.csound,
                    Some(midiReadCallback::<H>),
                );
            },

            MIDI_WRITE_CB => unsafe {
                raw::csoundSetExternalMidiWriteCallback(
                    self.inner.csound,
                    Some(midiWriteCallback::<H>),
                );
            },

            MIDI_IN_CLOSE => unsafe {
                raw::csoundSetExternalMidiInCloseCallback(
                    self.inner.csound,
                    Some(midiInCloseCallback::<H>),
                );
            },

            MIDI_OUT_CLOSE => unsafe {
                raw::csoundSetExternalMidiOutCloseCallback(
                    self.inner.csound,
                    Some(midiOutCloseCallback::<H>),
                );
            },

            _ => {}
        }
    }
}

extern "C" fn message_string_cb<H: Handler>(
    csound: *mut raw::CSOUND,
    attr: c_int,
    message: *const c_char,
) {
    unsafe {
        let info = CStr::from_ptr(message);
        match info.to_str() {
            Ok(s) => (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
                .handler
                .message_cb(MessageType::from_u32(attr as u32), s),
            Err(error) => println!("Error parsing the csound message {}", error),
        }
    }
}

/****** Event callbacks functions *******************************************************************/

extern "C" fn senseEventCallback<H: Handler>(csound: *mut raw::CSOUND, _userData: *mut c_void) {
    unsafe {
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .sense_event_cb();
    }
}

/****** real time audio callbacks functions *******************************************************************/

extern "C" fn playOpenCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    dev: *const raw::csRtAudioParams,
) -> i32 {
    unsafe {
        let name = (CStr::from_ptr((*dev).devName)).to_owned();
        let rtParams = RT_AudioParams {
            devName: name.into_string().unwrap(),
            devNum: (*dev).devNum as u32,
            bufSamp_SW: (*dev).bufSamp_SW as u32,
            bufSamp_HW: (*dev).bufSamp_HW as u32,
            nChannels: (*dev).nChannels as u32,
            sampleFormat: (*dev).sampleFormat as u32,
            sampleRate: (*dev).sampleRate as f32,
        };
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .play_open_cb(&rtParams)
            .to_i32()
    }
}

extern "C" fn recOpenCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    dev: *const raw::csRtAudioParams,
) -> i32 {
    unsafe {
        let name = (CStr::from_ptr((*dev).devName)).to_owned();
        let rtParams = RT_AudioParams {
            devName: name.into_string().unwrap(),
            devNum: (*dev).devNum as u32,
            bufSamp_SW: (*dev).bufSamp_SW as u32,
            bufSamp_HW: (*dev).bufSamp_HW as u32,
            nChannels: (*dev).nChannels as u32,
            sampleFormat: (*dev).sampleFormat as u32,
            sampleRate: (*dev).sampleRate as f32,
        };
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .rec_open_cb(&rtParams)
            .to_i32()
    }
}

extern "C" fn rtcloseCallback<H: Handler>(csound: *mut raw::CSOUND) {
    unsafe {
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .rt_close_cb();
    }
}

extern "C" fn rtplayCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    outBuf: *const f64,
    nbytes: c_int,
) {
    unsafe {
        let out = slice::from_raw_parts(outBuf, nbytes as usize);
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .rt_play_cb(&out);
    }
}

extern "C" fn rtrecordCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    outBuf: *mut f64,
    nbytes: c_int,
) -> c_int {
    unsafe {
        let mut buff = slice::from_raw_parts_mut(outBuf, nbytes as usize);
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .rt_rec_cb(&mut buff) as c_int
    }
}

extern "C" fn audioDeviceListCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    dev: *mut raw::CS_AUDIODEVICE,
    isOutput: c_int,
) -> i32 {
    unsafe {
        let name = (CStr::from_ptr((*dev).device_name.as_ptr())).to_owned();
        let id = (CStr::from_ptr((*dev).device_id.as_ptr())).to_owned();
        let module = (CStr::from_ptr((*dev).rt_module.as_ptr())).to_owned();
        let audioDevice = CS_AudioDevice {
            device_name: name.into_string().unwrap(),
            device_id: id.into_string().unwrap(),
            rt_module: module.into_string().unwrap(),
            max_nchnls: (*dev).max_nchnls as u32,
            isOutput: isOutput as u32,
        };
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .audio_dev_list_cb(audioDevice);
    }
    0
}

extern "C" fn keyboard_callback<H: Handler>(
    userData: *mut c_void,
    p: *mut c_void,
    _type_: c_uint,
) -> c_int {
    unsafe {
        match (*(userData as *mut Inner<H>)).handler.keyboard_cb() {
            '\0' => {}
            value => {
                *(p as *mut c_int) = value as c_int;
            }
        }
        0
    }
}

/********* General Input/Output callbacks ********************************************************************/
extern "C" fn fileOpenCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    filePath: *const c_char,
    fileType: c_int,
    operation: c_int,
    isTemp: c_int,
) {
    unsafe {
        let path = (CStr::from_ptr(filePath)).to_owned();
        let path = path.into_string().unwrap_or_else(|_| "".to_string());

        let file_info = FileInfo {
            name: path,
            file_type: FileTypes::from(fileType as u8),
            is_writing: operation != 0,
            is_temp: isTemp != 0,
        };
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .file_open_cb(&file_info);
    }
}

/* Score Handling callbacks ********************************************************* */

// Sets an external callback for Cscore processing. Pass NULL to reset to the internal cscore() function (which does nothing).
// This callback is retained after a csoundReset() call.
extern "C" fn scoreCallback<H: Handler>(csound: *mut raw::CSOUND) {
    unsafe {
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .cscore_cb();
    }
}

/* Channels and events callbacks **************************************************** */

extern "C" fn inputChannelCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    channelName: *const c_char,
    channelValuePtr: *mut c_void,
    _channelType: *const c_void,
) {
    unsafe {
        let name = (CStr::from_ptr(channelName)).to_str();
        if name.is_err() {
            return;
        }
        let name = name.unwrap();

        match (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .input_channel_cb(name)
        {
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
    }
}

extern "C" fn outputChannelCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    channelName: *const c_char,
    channelValuePtr: *mut c_void,
    _channelType: *const c_void,
) {
    unsafe {
        let name = (CStr::from_ptr(channelName)).to_str();
        if name.is_err() {
            return;
        }
        let name = name.unwrap();
        let mut ptr = ::std::ptr::null_mut();
        let ptr: *mut *mut f64 = &mut ptr as *mut *mut _;
        let channel_type = raw::csoundGetChannelPtr(csound, ptr, channelName, 0) as u32;
        let channel_type = channel_type & raw::CSOUND_CHANNEL_TYPE_MASK as u32;
        match channel_type {
            raw::CSOUND_CONTROL_CHANNEL => {
                let value = *(channelValuePtr as *mut f64);
                let data = ChannelData::CS_CONTROL_CHANNEL(value);
                (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
                    .handler
                    .output_channel_cb(name, data);
            }

            raw::CSOUND_STRING_CHANNEL => {
                let mut string = CStr::from_ptr(channelValuePtr as *const c_char).to_str();
                if string.is_err() {
                    return;
                }
                let string = string.unwrap().to_string();
                let data = ChannelData::CS_STRING_CHANNEL(string);
                (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
                    .handler
                    .output_channel_cb(name, data);
            }

            _ => {}
        }
    }
}

/****** MIDI I/O callbacks functions *******************************************************************/

// Sets callback for opening real time MIDI input.
extern "C" fn midiInOpenCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    _userData: *mut *mut c_void,
    devName: *const c_char,
) -> c_int {
    unsafe {
        let name = match CStr::from_ptr(devName).to_str() {
            Ok(s) => s,
            _ => return raw::CSOUND_ERROR,
        };
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .midi_in_open_cb(&name);
        raw::CSOUND_SUCCESS
    }
}

// Sets callback for opening real time MIDI output.
extern "C" fn midiOutOpenCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    _userData: *mut *mut c_void,
    devName: *const c_char,
) -> c_int {
    unsafe {
        let name = match CStr::from_ptr(devName).to_str() {
            Ok(s) => s,
            _ => return raw::CSOUND_ERROR,
        };
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .midi_out_open_cb(&name);
        raw::CSOUND_SUCCESS
    }
}

// Sets callback for reading from real time MIDI input.
extern "C" fn midiReadCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    _userData: *mut c_void,
    buf: *mut c_uchar,
    nbytes: c_int,
) -> c_int {
    unsafe {
        let mut out = slice::from_raw_parts_mut(buf, nbytes as usize);
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .midi_read_cb(&mut out) as c_int
    }
}

// Sets callback for writing to real time MIDI output.
#[allow(dead_code)]
extern "C" fn midiWriteCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    _userData: *mut c_void,
    buf: *const u8,
    nbytes: c_int,
) -> c_int {
    unsafe {
        let buffer = slice::from_raw_parts(buf, nbytes as usize);
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .midi_write_cb(&buffer) as c_int
    }
}

//Sets callback for closing real time MIDI input.
extern "C" fn midiInCloseCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    _userData: *mut c_void,
) -> c_int {
    unsafe {
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .midi_in_close_cb();
        raw::CSOUND_SUCCESS
    }
}

// Sets callback for closing real time MIDI output.
extern "C" fn midiOutCloseCallback<H: Handler>(
    csound: *mut raw::CSOUND,
    _userData: *mut c_void,
) -> c_int {
    unsafe {
        (*(raw::csoundGetHostData(csound) as *mut Inner<H>))
            .handler
            .midi_out_close_cb();
        raw::CSOUND_SUCCESS
    }
}

//Sets callback for converting MIDI error codes to strings.
/*extern fn externalMidiErrorStringCallback<H: Handler> (midi_error_code : c_int) -> *const c_char {
    unsafe{
    }
}*/
