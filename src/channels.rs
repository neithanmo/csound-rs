use std::ptr;
use std::marker::PhantomData;
use std::slice;
use libc::c_int;

use enums::{AudioChannel, ControlChannel, StrChannel, Status};
use csound::{Csound, Writable, Readable};

/// Indicates the channel behaivor.
#[derive(Debug, PartialEq, Clone)]
pub enum ChannelBehavior {
    CHANNEL_NO_HINTS = 0,
    CHANNEL_INT = 1,
    CHANNEL_LIN = 2,
    CHANNEL_EXP = 3,
}

impl ChannelBehavior {
    pub fn from_u32(value: u32) -> ChannelBehavior {
        match value {
            0 => ChannelBehavior::CHANNEL_NO_HINTS,
            1 => ChannelBehavior::CHANNEL_INT,
            2 => ChannelBehavior::CHANNEL_LIN,
            3 => ChannelBehavior::CHANNEL_EXP,
            _ => panic!("Unknown channel behavior type"),
        }
    }

    pub fn to_u32(&self) -> u32 {
        match self {
            ChannelBehavior::CHANNEL_NO_HINTS => 0,
            ChannelBehavior::CHANNEL_INT => 1,
            ChannelBehavior::CHANNEL_LIN => 2,
            ChannelBehavior::CHANNEL_EXP => 3,
        }
    }
}

/// Holds the channel HINTS information.
///
/// This hints(information) is metadata which describes the channel
/// and for what it is used for. This hints could be configured using the
/// [`chn`](https://csound.com/docs/manual/chn.html) opcode or through of [`Csound::set_channel_hints`](struct.Csound.html#method.set_channel_hints)
/// and [`Csound::get_channel_hints`](struct.Csound.html#method.get_channel_hints) functions.
///
#[derive(Debug, Clone)]
pub struct ChannelHints {
    pub behav: ChannelBehavior,
    pub dflt: f64,
    pub min: f64,
    pub max: f64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub attributes: String,
}

impl Default for ChannelHints {
    fn default() -> ChannelHints {
        ChannelHints {
            behav: ChannelBehavior::CHANNEL_NO_HINTS,
            dflt: 0f64,
            min: 0f64,
            max: 0f64,
            x: 0i32,
            y: 0i32,
            width: 0i32,
            height: 0i32,
            attributes: String::default(),
        }
    }
}

/// Holds all relevant information about a csound bus channel.
#[derive(Debug, Clone, Default)]
pub struct ChannelInfo {
    /// The channel name.
    pub name: String,
    /// The channel type.
    pub type_: i32,
    /// Channel extra metadata.
    pub hints: ChannelHints,
}

/// Holds pvs data info of a pvs channel.
///
/// To be used with [pvsin](http://www.csounds.com/manual/html/pvsin.html),
/// [`pvsout`](http://www.csounds.com/manual/html/pvsin.html) opcodes and with
/// [`Csound::get_pvs_channel`](struct.Csound.html#method.get_pvs_channel) and [`Csound::set_pvs_channel`](struct.Csound.html#method.set_pvs_channel)
/// methods.
///
#[derive(Debug, Clone)]
pub struct PvsDataExt {
    pub N: u32,
    pub sliding: u32,
    pub NB: i32,
    pub overlap: u32,
    pub winsize: u32,
    pub wintype: u32,
    pub format: u32,
    pub framecount: u32,
    pub frame: Vec<f32>,
}

impl PvsDataExt {
    /// Creates a new pvs data channel struct.
    ///
    /// # Arguments
    /// * `winsize` The number of elements in the pvs window and also it is the
    /// number of samples in the frame buffer.
    pub fn new(winsize: u32) -> PvsDataExt {
        PvsDataExt {
            N: winsize,
            sliding: 0,
            NB: 0,
            overlap: 0,
            winsize,
            wintype: 0,
            format: 0,
            framecount: 0,
            frame: vec![0.0; winsize as usize],
        }
    }
}

pub(crate) trait GetChannel<'a, T>{
    fn get_input_channel(&'a self, name: &str, _channel_type: T) -> Result<ChannelPtr<'a, T, Writable>, Status>  ;
    fn get_output_channel(&'a self, name: &str, _channel_type: T) -> Result<ChannelPtr<'a, T, Readable>, Status>  ;
}

pub(crate) trait InputChannelPtr<T: ?Sized>{
    fn write(&self, inp: T);
}

pub(crate) trait OutputChannelPtr<'a, T: ?Sized>{
    fn read(&'a self) -> &'a T;
}

#[derive(Debug)]
pub struct ChannelPtr<'a, C, T> {
    pub(crate) ptr: *mut f64,
    pub(crate) len: usize,
    pub(crate) phantom: PhantomData<&'a T>,
    pub(crate) phantomC: PhantomData<C>,
}


impl<'a> OutputChannelPtr<'a, f64> for ChannelPtr<'a, ControlChannel, Readable>{
    fn read(&'a self) -> &'a f64{
        unsafe{
            &*self.ptr
        }
    }

}

impl<'a> InputChannelPtr<f64> for ChannelPtr<'a, ControlChannel, Writable>{
    fn write(&self, inp: f64){
        unsafe{
            *self.ptr = inp;
            println!("input {}", *self.ptr);
        }
    }
}

impl<'a> OutputChannelPtr<'a, [f64]> for ChannelPtr<'a, AudioChannel, Readable>{

    fn read(&'a self) -> &[f64]{
        unsafe {
            slice::from_raw_parts(self.ptr as *const f64, self.len)
        }
    }
}

impl<'a> InputChannelPtr<&[f64]> for ChannelPtr<'a, AudioChannel, Writable>{
    fn write(&self, inp: &[f64]){
        let mut len = inp.len();
        let size = self.len;
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(inp.as_ptr(), self.ptr, len);
        }
    }
}

impl<'a> OutputChannelPtr<'a, [u8]> for ChannelPtr<'a, StrChannel, Readable>{
    fn read(&'a self) -> &'a [u8]{
        unsafe {
            slice::from_raw_parts(self.ptr as *const u8, self.len)
        }
    }
}

impl<'a> InputChannelPtr<&[u8]> for ChannelPtr<'a, StrChannel, Writable>{
    fn write(&self, inp: &[u8]){
        let mut len = inp.len();
        let size = self.len;
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(inp.as_ptr(), self.ptr as *mut u8, len);
        }
    }
}



impl<'a> GetChannel<'a, AudioChannel> for Csound {

    fn get_input_channel(&'a self, name: &str, _channel_type: AudioChannel) -> Result<ChannelPtr<'a, AudioChannel, Writable>, Status> {

        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let len = self.get_ksmps() as usize;
        let channel_bits = (csound_sys::CSOUND_AUDIO_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int;

        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, channel_bits));
            match result {
                Status::CS_SUCCESS => Ok(ChannelPtr {
                    ptr: *ptr,
                    len,
                    phantom: PhantomData,
                    phantomC: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
        }
    }

    fn get_output_channel(&'a self, name: &str, _channel_type: AudioChannel) -> Result<ChannelPtr<'a, AudioChannel, Readable>, Status> {

        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let len = self.get_ksmps() as usize;
        let channel_bits = (csound_sys::CSOUND_AUDIO_CHANNEL | csound_sys::CSOUND_OUTPUT_CHANNEL) as c_int;
        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, channel_bits));
            match result {
                Status::CS_SUCCESS => Ok(ChannelPtr {
                    ptr: *ptr,
                    len,
                    phantom: PhantomData,
                    phantomC: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
        }
    }
}

impl<'a> GetChannel<'a, ControlChannel> for Csound {

    fn get_input_channel(&'a self, name: &str, _channel_type: ControlChannel) -> Result<ChannelPtr<'a, ControlChannel, Writable>, Status> {

        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let len = 1;
        let channel_bits = (csound_sys::CSOUND_CONTROL_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int;

        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, channel_bits));
            match result {
                Status::CS_SUCCESS => Ok(ChannelPtr {
                    ptr: *ptr,
                    len,
                    phantom: PhantomData,
                    phantomC: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
        }
    }

    fn get_output_channel(&'a self, name: &str, _channel_type: ControlChannel) -> Result<ChannelPtr<'a, ControlChannel, Readable>, Status> {

        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let len = 1;
        let channel_bits = (csound_sys::CSOUND_CONTROL_CHANNEL | csound_sys::CSOUND_OUTPUT_CHANNEL) as c_int;
        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, channel_bits));
            match result {
                Status::CS_SUCCESS => Ok(ChannelPtr {
                    ptr: *ptr,
                    len,
                    phantom: PhantomData,
                    phantomC: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
        }
    }
}

impl<'a> GetChannel<'a, StrChannel> for Csound {

    fn get_input_channel(&'a self, name: &str, _channel_type: StrChannel) -> Result<ChannelPtr<'a, StrChannel, Writable>, Status> {

        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let len = self.get_channel_data_size(name) as usize;
        let channel_bits = (csound_sys::CSOUND_STRING_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int;

        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, channel_bits));
            match result {
                Status::CS_SUCCESS => Ok(ChannelPtr {
                    ptr: *ptr,
                    len,
                    phantom: PhantomData,
                    phantomC: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
        }
    }

    fn get_output_channel(&'a self, name: &str, _channel_type: StrChannel) -> Result<ChannelPtr<'a, StrChannel, Readable>, Status> {

        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let len = self.get_channel_data_size(name) as usize;
        let channel_bits = (csound_sys::CSOUND_STRING_CHANNEL | csound_sys::CSOUND_OUTPUT_CHANNEL) as c_int;
        unsafe {
            let result = Status::from(self.get_raw_channel_ptr(name, ptr, channel_bits));
            match result {
                Status::CS_SUCCESS => Ok(ChannelPtr {
                    ptr: *ptr,
                    len,
                    phantom: PhantomData,
                    phantomC: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
        }
    }
}
