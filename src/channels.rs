use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::slice;

use crate::enums::{AudioChannel, ControlChannel, ControlChannelType, StrChannel};

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

/// Struct represents a csound input channel object.
#[derive(Debug)]
pub struct InputChannel<'a, T> {
    pub(crate) ptr: *mut f64,
    pub(crate) len: usize,
    pub(crate) phantom: PhantomData<&'a mut T>,
}

/// Struct represents a csound output channel object.
#[derive(Debug)]
pub struct OutputChannel<'a, T> {
    pub(crate) ptr: *mut f64,
    pub(crate) len: usize,
    pub(crate) phantom: PhantomData<&'a T>,
}

pub trait IsChannel {
    fn c_type() -> ControlChannelType;
}

impl IsChannel for ControlChannel {
    fn c_type() -> ControlChannelType {
        ControlChannelType::CSOUND_CONTROL_CHANNEL
    }
}
impl IsChannel for AudioChannel {
    fn c_type() -> ControlChannelType {
        ControlChannelType::CSOUND_AUDIO_CHANNEL
    }
}
impl IsChannel for StrChannel {
    fn c_type() -> ControlChannelType {
        ControlChannelType::CSOUND_STRING_CHANNEL
    }
}

// CONTROL CHANNEL
impl<'a> OutputChannel<'a, ControlChannel> {
    /// Reads data from a csound's control channel
    ///
    /// # Returns
    /// A reference to the control channel's value
    pub fn read(&'a self) -> f64 {
        unsafe { *self.ptr }
    }
}

impl<'a> InputChannel<'a, ControlChannel> {
    /// Writes data to csound's control channel
    pub fn write(&self, inp: f64) {
        unsafe {
            *self.ptr = inp;
        }
    }
}

// AUDIO CHANNEL
impl<'a> OutputChannel<'a, AudioChannel> {
    /// Reads data from a csound's Audio channel
    ///
    /// # Returns
    /// A reference to the control channel's slice of ksmps samples
    pub fn read(&'a self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.ptr as *const f64, self.len) }
    }
}

impl<'a> InputChannel<'a, AudioChannel> {
    /// Writes audio data to an audio channel
    ///
    /// # Arguments
    /// A slice of ksmps audio samples to be copied into the channel's buffer
    /// If this slice is longer than the channel's buffer, only
    /// Channel's size elements would be copied
    pub fn write(&self, inp: &[f64]) {
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

// STRING CHANNEL
impl<'a> OutputChannel<'a, StrChannel> {
    /// Reads data from a csound's Audio channel
    ///
    /// # Returns
    /// A reference to the string channel's slice with bytes which represents the content of a string channel
    pub fn read(&'a self) -> &'a [u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl<'a> InputChannel<'a, StrChannel> {
    /// Writes bytes to a string channel's buffer
    ///
    /// # Arguments
    /// A slice of bytes to be copied into the channel's buffer
    /// If this slice is longer than the channel's buffer, only
    /// Channel's size elements would be copied from it
    pub fn write(&self, inp: &[u8]) {
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

impl<'a> AsRef<f64> for OutputChannel<'a, ControlChannel> {
    fn as_ref(&self) -> &f64 {
        unsafe { &*self.ptr }
    }
}

impl<'a> AsRef<f64> for InputChannel<'a, ControlChannel> {
    fn as_ref(&self) -> &f64 {
        unsafe { &*self.ptr }
    }
}

impl<'a> AsMut<f64> for InputChannel<'a, ControlChannel> {
    fn as_mut(&mut self) -> &mut f64 {
        unsafe { &mut *self.ptr }
    }
}

// Internal macro used to generate AudioChannel and StrChannel implementations
// for the AsRef trait.
macro_rules! impl_asref_for_channel_ptr {
    ($ct:ty, $t:ty) => {
        impl<'a> AsRef<[$t]> for OutputChannel<'a, $ct> {
            fn as_ref(&self) -> &[$t] {
                unsafe { slice::from_raw_parts(self.ptr as *const $t, self.len) }
            }
        }
    };
}

// Internal macro used to generate AudioChannel and StrChannel implementations
// for the AsMut trait.
macro_rules! impl_asmut_for_channel_ptr {
    ($ct:ty, $t:ty) => {
        impl<'a> AsMut<[$t]> for InputChannel<'a, $ct> {
            fn as_mut(&mut self) -> &mut [$t] {
                unsafe { slice::from_raw_parts_mut(self.ptr as *mut $t, self.len) }
            }
        }
    };
}

impl_asref_for_channel_ptr!(AudioChannel, f64);
impl_asref_for_channel_ptr!(StrChannel, u8);

impl_asmut_for_channel_ptr!(AudioChannel, f64);
impl_asmut_for_channel_ptr!(StrChannel, u8);

// Internal macro used to generate ControlChannel, AudioChannel and StrChannel implementations
// for the Deref trait.
macro_rules! impl_deref_for_channel_ptr {
    ($ct:ty, $t:ty) => {
        impl<'a> Deref for OutputChannel<'a, $ct> {
            type Target = $t;

            fn deref(&self) -> &Self::Target {
                self.as_ref()
            }
        }

        impl<'a> Deref for InputChannel<'a, $ct> {
            type Target = $t;
            #[allow(unconditional_recursion)]
            fn deref(&self) -> &Self::Target {
                self.as_ref()
            }
        }
    };
}

// Internal macro used to generate ControlChannel, AudioChannel and StrChannel implementations
// for the DerefMut trait.
macro_rules! impl_deref_mut_for_channel_ptr {
    ($ct:ty, $t:ty) => {
        impl<'a> DerefMut for InputChannel<'a, $ct> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.as_mut()
            }
        }
    };
}

impl_deref_for_channel_ptr!(ControlChannel, f64);
impl_deref_for_channel_ptr!(AudioChannel, [f64]);
impl_deref_for_channel_ptr!(StrChannel, [u8]);

impl_deref_mut_for_channel_ptr!(ControlChannel, f64);
impl_deref_mut_for_channel_ptr!(AudioChannel, [f64]);
impl_deref_mut_for_channel_ptr!(StrChannel, [u8]);
