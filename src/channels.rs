use libc::c_int;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::slice;

use csound::{Csound, Readable, Writable};
use enums::{AudioChannel, ControlChannel, Status, StrChannel};

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

pub trait GetChannel<'a, T> {
    fn get_input_channel(&'a self, name: &str, _: T)
        -> Result<ChannelPtr<'a, T, Writable>, Status>;
    fn get_output_channel(
        &'a self,
        name: &str,
        _: T,
    ) -> Result<ChannelPtr<'a, T, Readable>, Status>;
}

/// Tait with the write function which is implemented by
/// control, audio and string channels
pub trait InputChannelPtr<T: ?Sized> {
    fn write(&self, inp: T);
}

/// Trait with the read function which is implemented by
/// like control, audio and string channels
pub trait OutputChannelPtr<'a, T: ?Sized> {
    fn read(&'a self) -> &'a T;
}

/// Struct represents a csound channel object.
///
/// in a more accurate way than [`ControlChannelPtr`](struct.ControlChannelPtr.html)
/// use this struct instead.
/// Also, this struct implements traits to read/write audio, control and string channels.
#[derive(Debug)]
pub struct ChannelPtr<'a, C, T> {
    pub(crate) ptr: *mut f64,
    pub(crate) len: usize,
    pub(crate) phantom: PhantomData<&'a T>,
    pub(crate) phantomC: PhantomData<C>,
}

impl<'a> OutputChannelPtr<'a, f64> for ChannelPtr<'a, ControlChannel, Readable> {
    /// Reads data from a csound's control channel
    ///
    /// # Returns
    /// A reference to the control channel's value
    fn read(&'a self) -> &'a f64 {
        unsafe { &*self.ptr }
    }
}

impl<'a> InputChannelPtr<f64> for ChannelPtr<'a, ControlChannel, Writable> {
    /// Writes data to csound's control channel
    fn write(&self, inp: f64) {
        unsafe {
            *self.ptr = inp;
        }
    }
}

impl<'a> OutputChannelPtr<'a, [f64]> for ChannelPtr<'a, AudioChannel, Readable> {
    /// Reads data from a csound's Audio channel
    ///
    /// # Returns
    /// A reference to the control channel's slice of ksmps samples
    fn read(&'a self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.ptr as *const f64, self.len) }
    }
}

impl<'a> InputChannelPtr<&[f64]> for ChannelPtr<'a, AudioChannel, Writable> {
    /// Writes audio data to an audio channel
    ///
    /// # Arguments
    /// A slice of ksmps audio samples to be copied into the channel's buffer
    /// If this slice is onger than the channel's buffer, only
    /// Channel's size elments would be copied from it
    fn write(&self, inp: &[f64]) {
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

impl<'a> OutputChannelPtr<'a, [u8]> for ChannelPtr<'a, StrChannel, Readable> {
    /// Reads data from a csound's Audio channel
    ///
    /// # Returns
    /// A reference to the string channel's slice with bytes which represents the content of a string channel
    fn read(&'a self) -> &'a [u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl<'a> InputChannelPtr<&[u8]> for ChannelPtr<'a, StrChannel, Writable> {
    /// Writes bytes to a string channel's buffer
    ///
    /// # Arguments
    /// A slice of bytes to be copied into the channel's buffer
    /// If this slice is longer than the channel's buffer, only
    /// Channel's size elements would be copied from it
    fn write(&self, inp: &[u8]) {
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

enum ChannelInternalType {
    AUDIO,
    CONTROL,
    STR,
}

// Internal macro used to generate AudioChannel, ControlChannel and StrChannel implementations
// for the GetChannel trait.
macro_rules! impl_get_channel_trait {

    ($($t:ty, $e:expr);*) => ($(

        impl<'a> GetChannel<'a, $t> for Csound {
            /// Return a [`ChannelPtr`](struct.ChannelPtr.html) which represent a csound's input channel ptr.
            /// creating the channel first if it does not exist yet.
            /// # Arguments
            /// * `name` The channel name.
            /// * `channel_type` must be any  of the following values:
            ///  - ControlChannel::ctype
            ///     control data (one MYFLT value)
            ///  - AudioChannel::ctype
            ///     audio data (get_ksmps() f64 values)
            ///  - StrChannel::ctype
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
            /// A  Writable ChannelPtr on success or a Status code,
            ///   "Not enough memory for allocating the channel" (CS_MEMORY)
            ///   "The specified name or type is invalid" (CS_ERROR)
            /// or, if a channel with the same name but incompatible type
            /// already exists, the type of the existing channel.
            /// * Note: to find out the type of a channel without actually
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
            ///
            /// # Example
            /// ```
            ///  // Creates a Csound instance
            /// let csound = Csound::new();
            /// csound.compile_csd(csd_filename).unwrap();
            /// csound.start();
            /// // Request a csound's input control channel
            /// let control_channel = csound.get_input_channel("myChannel", ControlChannel::ctype ).unwrap();
            /// // Writes some data to the channel
            /// control_channel.write(10.25);
            /// // Request a csound's input audio channel
            /// let audio_channel = csound.get_input_channel("myAudioChannel", AudioChannel::ctype).unwrap();
            /// // Request a csound's input string channel
            /// let string_channel = csound.get_input_channel("myStringChannel", StrChannel::ctype).unwrap();
            ///
            /// ```
            fn get_input_channel(&'a self, name: &str, _: $t) -> Result<ChannelPtr<'a, $t, Writable>, Status> {

                let mut ptr = ptr::null_mut() as *mut f64;
                let ptr = &mut ptr as *mut *mut _;
                let len;
                let bits;

                match $e {
                    ChannelInternalType::AUDIO => {
                        len = self.get_ksmps() as usize;
                        bits = (csound_sys::CSOUND_AUDIO_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int;
                    },
                    ChannelInternalType::CONTROL => {
                        len = 1;
                        bits = (csound_sys::CSOUND_CONTROL_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int;
                    },
                    ChannelInternalType::STR => {
                        len = self.get_channel_data_size(name) as usize;
                        bits = (csound_sys::CSOUND_STRING_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int;
                    },
                }

                unsafe {
                    let result = Status::from(self.get_raw_channel_ptr(name, ptr, bits));
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

            /// Return a [`ChannelPtr`](struct.ChannelPtr.html) which represent a csound's output channel ptr.
            /// creating the channel first if it does not exist yet.
            /// # Arguments
            /// * `name` The channel name.
            /// * `channel_type` must be any  of the following values:
            ///  - ControlChannel::ctype
            ///     control data (one MYFLT value)
            ///  - AudioChannel::ctype
            ///     audio data (get_ksmps() f64 values)
            ///  - StrChannel::ctype
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
            /// A  Readable ChannelPtr on success or a Status code,
            ///   "Not enough memory for allocating the channel" (CS_MEMORY)
            ///   "The specified name or type is invalid" (CS_ERROR)
            /// or, if a channel with the same name but incompatible type
            /// already exists, the type of the existing channel.
            /// * Note: to find out the type of a channel without actually
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
            /// # Example
            /// ```
            ///  // Creates a Csound instance
            /// let csound = Csound::new();
            /// csound.compile_csd(csd_filename).unwrap();
            /// csound.start();
            /// // Request a csound's output control channel
            /// let control_channel = csound.get_output_channel("myChannel", ControlChannel::ctype ).unwrap();
            /// // Writes some data to the channel
            /// println!("channel value {}", constrol_channel.read());
            /// // Request a csound's output audio channel
            /// let audio_channel = csound.get_output_channel("myAudioChannel", AudioChannel::ctype).unwrap();
            /// println!("audio channel samples {:?}", audio_channel.read() );
            /// // Request a csound's output string channel
            /// let string_channel = csound.get_output_channel("myStringChannel", StrChannel::ctype).unwrap();
            ///
            /// ```
            fn get_output_channel(&'a self, name: &str, _: $t) -> Result<ChannelPtr<'a, $t, Readable>, Status> {

                let mut ptr = ptr::null_mut() as *mut f64;
                let ptr = &mut ptr as *mut *mut _;

                let len;
                let bits;

                match $e {
                    ChannelInternalType::AUDIO => {
                        len = self.get_ksmps() as usize;
                        bits = (csound_sys::CSOUND_AUDIO_CHANNEL | csound_sys::CSOUND_OUTPUT_CHANNEL) as c_int;
                    },
                    ChannelInternalType::CONTROL => {
                        len = 1;
                        bits = (csound_sys::CSOUND_CONTROL_CHANNEL | csound_sys::CSOUND_OUTPUT_CHANNEL) as c_int;
                    },
                    ChannelInternalType::STR => {
                        len = self.get_channel_data_size(name) as usize;
                        bits = (csound_sys::CSOUND_STRING_CHANNEL | csound_sys::CSOUND_OUTPUT_CHANNEL) as c_int;
                    },
                }

                unsafe {
                    let result = Status::from(self.get_raw_channel_ptr(name, ptr, bits));
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
    )*)
}

impl_get_channel_trait!(AudioChannel, ChannelInternalType::AUDIO; ControlChannel, ChannelInternalType::CONTROL; StrChannel,ChannelInternalType::STR);

impl<'a> AsRef<f64> for ChannelPtr<'a, ControlChannel, Readable> {
    fn as_ref(&self) -> &f64 {
        unsafe { &*self.ptr }
    }
}

impl<'a> AsRef<f64> for ChannelPtr<'a, ControlChannel, Writable> {
    fn as_ref(&self) -> &f64 {
        unsafe { &*self.ptr }
    }
}

impl<'a> AsMut<f64> for ChannelPtr<'a, ControlChannel, Writable> {
    fn as_mut(&mut self) -> &mut f64 {
        unsafe { &mut *self.ptr }
    }
}

// Internal macro used to generate AudioChannel and StrChannel implementations
// for the AsRef trait.
macro_rules! impl_asref_for_channel_ptr {
    ($ct:ty, $t:ty) => {
        impl<'a> AsRef<[$t]> for ChannelPtr<'a, $ct, Readable> {
            fn as_ref(&self) -> &[$t] {
                unsafe { slice::from_raw_parts(self.ptr as *const $t, self.len) }
            }
        }

        impl<'a> AsRef<[$t]> for ChannelPtr<'a, $ct, Writable> {
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
        impl<'a> AsMut<[$t]> for ChannelPtr<'a, $ct, Writable> {
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
        impl<'a> Deref for ChannelPtr<'a, $ct, Readable> {
            type Target = $t;

            fn deref(&self) -> &Self::Target {
                self.as_ref()
            }
        }

        impl<'a> Deref for ChannelPtr<'a, $ct, Writable> {
            type Target = $t;

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
        impl<'a> DerefMut for ChannelPtr<'a, $ct, Writable> {
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
