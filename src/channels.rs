use std::marker::PhantomData;
use std::ptr::NonNull;
use std::slice;

use crate::enums::{AudioChannel, ControlChannel, ControlChannelType, StrChannel};

/// Indicates the channel behavior.
// Unknown(u32) preserves unrecognized values from the C API, keeping
// forward-compatibility as csound adds new behavior types.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ChannelBehavior {
    /// No hints provided.
    NoHints,
    /// Integer values.
    Integer,
    /// Linear interpolation.
    Linear,
    /// Exponential interpolation.
    Exponential,
    /// Unrecognized behavior value from the C API.
    Unknown(u32),
}

impl From<u32> for ChannelBehavior {
    fn from(value: u32) -> Self {
        match value {
            0 => ChannelBehavior::NoHints,
            1 => ChannelBehavior::Integer,
            2 => ChannelBehavior::Linear,
            3 => ChannelBehavior::Exponential,
            other => ChannelBehavior::Unknown(other),
        }
    }
}

impl ChannelBehavior {
    pub fn to_u32(self) -> u32 {
        match self {
            ChannelBehavior::NoHints => 0,
            ChannelBehavior::Integer => 1,
            ChannelBehavior::Linear => 2,
            ChannelBehavior::Exponential => 3,
            ChannelBehavior::Unknown(v) => v,
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
            behav: ChannelBehavior::NoHints,
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
    pub n: u32,
    pub sliding: u32,
    pub nb: i32,
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
    ///   number of samples in the frame buffer.
    pub fn new(winsize: u32) -> PvsDataExt {
        PvsDataExt {
            n: winsize,
            sliding: 0,
            nb: 0,
            overlap: 0,
            winsize,
            wintype: 0,
            format: 0,
            framecount: 0,
            frame: vec![0.0; winsize as usize],
        }
    }
}

/// Csound input channel - allows writing data to csound.
///
/// This struct wraps a pointer to csound's internal channel buffer.
/// Use the explicit `get()`, `set()`, `write()`, and slice accessor methods
/// rather than relying on implicit dereferencing.
#[derive(Debug)]
pub struct InputChannel<'a, T> {
    ptr: NonNull<f64>,
    len: usize,
    phantom: PhantomData<&'a mut T>,
}

/// Csound output channel - allows reading data from csound.
///
/// This struct wraps a pointer to csound's internal channel buffer.
/// Use the explicit `get()`, `read()`, and slice accessor methods
/// rather than relying on implicit dereferencing.
#[derive(Debug)]
pub struct OutputChannel<'a, T> {
    ptr: NonNull<f64>,
    len: usize,
    phantom: PhantomData<&'a T>,
}

// SAFETY: Channel pointers are tied to the Csound instance lifetime
// and access is synchronized through csound's own mechanisms.
unsafe impl<T> Send for InputChannel<'_, T> {}
unsafe impl<T> Send for OutputChannel<'_, T> {}

impl<'a, T> InputChannel<'a, T> {
    /// Creates a new InputChannel from a raw pointer.
    ///
    /// # Safety
    /// The pointer must be valid and point to memory owned by csound.
    pub(crate) unsafe fn from_raw(ptr: *mut f64, len: usize) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| InputChannel {
            ptr,
            len,
            phantom: PhantomData,
        })
    }

    /// Returns the length of the channel buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the channel buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<'a, T> OutputChannel<'a, T> {
    /// Creates a new OutputChannel from a raw pointer.
    ///
    /// # Safety
    /// The pointer must be valid and point to memory owned by csound.
    pub(crate) unsafe fn from_raw(ptr: *mut f64, len: usize) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| OutputChannel {
            ptr,
            len,
            phantom: PhantomData,
        })
    }

    /// Returns the length of the channel buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the channel buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

pub trait IsChannel {
    fn c_type() -> ControlChannelType;
}

impl IsChannel for ControlChannel {
    fn c_type() -> ControlChannelType {
        ControlChannelType::Control
    }
}

impl IsChannel for AudioChannel {
    fn c_type() -> ControlChannelType {
        ControlChannelType::Audio
    }
}

impl IsChannel for StrChannel {
    fn c_type() -> ControlChannelType {
        ControlChannelType::String
    }
}

// ============================================================================
// CONTROL CHANNEL implementations
// ============================================================================

impl<'a> InputChannel<'a, ControlChannel> {
    /// Gets the current value of the control channel.
    #[inline]
    pub fn get(&self) -> f64 {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { *self.ptr.as_ptr() }
    }

    /// Sets the value of the control channel.
    #[inline]
    pub fn set(&self, value: f64) {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe {
            *self.ptr.as_ptr() = value;
        }
    }

    /// Writes a value to the control channel (alias for `set`).
    #[inline]
    pub fn write(&self, value: f64) {
        self.set(value);
    }
}

impl<'a> OutputChannel<'a, ControlChannel> {
    /// Gets the current value of the control channel.
    #[inline]
    pub fn get(&self) -> f64 {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { *self.ptr.as_ptr() }
    }

    /// Reads the value from the control channel (alias for `get`).
    #[inline]
    pub fn read(&self) -> f64 {
        self.get()
    }
}

// ============================================================================
// AUDIO CHANNEL implementations
// ============================================================================

impl<'a> InputChannel<'a, AudioChannel> {
    /// Returns an immutable slice of the audio channel's samples.
    #[inline]
    pub fn as_slice(&self) -> &[f64] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Returns a mutable slice of the audio channel's samples.
    ///
    /// # Safety
    /// Caller must ensure no aliasing occurs with csound's internal access.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    /// Writes audio samples to the channel.
    ///
    /// If the input slice is longer than the channel buffer,
    /// only `len()` samples will be copied.
    pub fn write(&self, samples: &[f64]) {
        let copy_len = samples.len().min(self.len);
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe {
            std::ptr::copy_nonoverlapping(samples.as_ptr(), self.ptr.as_ptr(), copy_len);
        }
    }
}

impl<'a> OutputChannel<'a, AudioChannel> {
    /// Returns an immutable slice of the audio channel's samples.
    #[inline]
    pub fn as_slice(&self) -> &[f64] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Reads the audio samples from the channel (alias for `as_slice`).
    #[inline]
    pub fn read(&self) -> &[f64] {
        self.as_slice()
    }
}

// ============================================================================
// STRING CHANNEL implementations
// ============================================================================

impl<'a> InputChannel<'a, StrChannel> {
    /// Returns an immutable slice of the string channel's bytes.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts(self.ptr.as_ptr() as *const u8, self.len) }
    }

    /// Returns a mutable slice of the string channel's bytes.
    ///
    /// # Safety
    /// Caller must ensure no aliasing occurs with csound's internal access.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr() as *mut u8, self.len) }
    }

    /// Writes bytes to the string channel.
    ///
    /// If the input slice is longer than the channel buffer,
    /// only `len()` bytes will be copied.
    pub fn write(&self, bytes: &[u8]) {
        let copy_len = bytes.len().min(self.len);
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr.as_ptr() as *mut u8, copy_len);
        }
    }
}

impl<'a> OutputChannel<'a, StrChannel> {
    /// Returns an immutable slice of the string channel's bytes.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts(self.ptr.as_ptr() as *const u8, self.len) }
    }

    /// Reads the string channel's bytes (alias for `as_slice`).
    #[inline]
    pub fn read(&self) -> &[u8] {
        self.as_slice()
    }
}
