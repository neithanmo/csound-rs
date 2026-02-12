use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::slice;

use csound_sys::ffi_bindgen::STRINGDAT;
use libc::c_char;

use crate::Myflt;
use crate::enums::{AudioChannel, ControlChannel, ControlChannelType, StrChannel};
use crate::error::Result;

mod sealed {
    pub trait Sealed {}
}

// Channel Access types
// use to define input channels and output channels
// allong with their variance
type InputPhantom<'a, T> = &'a mut T;
type OutputPhantom<'a, T> = &'a T;

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
/// This hints (information) is metadata which describes the channel
/// and for what it is used for. These hints can be configured using the
/// [`chn`](https://csound.com/docs/manual/chn.html) opcode or through
/// [`Csound::set_channel_hints`](struct.Csound.html#method.set_channel_hints)
/// and [`Csound::get_channel_hints`](struct.Csound.html#method.get_channel_hints) functions.
#[derive(Debug, Clone)]
pub struct ChannelHints {
    /// The channel behavior hint (e.g., linear, exponential scaling).
    pub behav: ChannelBehavior,
    /// Default value for the channel.
    pub dflt: Myflt,
    /// Minimum value for the channel.
    pub min: Myflt,
    /// Maximum value for the channel.
    pub max: Myflt,
    /// Suggested x position for GUI display.
    pub x: i32,
    /// Suggested y position for GUI display.
    pub y: i32,
    /// Suggested width for GUI display.
    pub width: i32,
    /// Suggested height for GUI display.
    pub height: i32,
    /// Optional free-form attributes string for GUI controllers.
    ///
    /// From the Csound C API: "This member must be set explicitly to NULL if not used."
    ///
    /// This field corresponds to the `Sattributes` parameter in the `chn_k` opcode.
    /// It provides additional metadata that front-ends can use to customize
    /// channel/controller presentation or behavior.
    ///
    /// - `None`: No attributes were set (maps to NULL in C API)
    /// - `Some(String)`: Contains the attributes string
    pub attributes: Option<String>,
}

impl Default for ChannelHints {
    fn default() -> ChannelHints {
        ChannelHints {
            behav: ChannelBehavior::NoHints,
            dflt: 0.0 as Myflt,
            min: 0.0 as Myflt,
            max: 0.0 as Myflt,
            x: 0i32,
            y: 0i32,
            width: 0i32,
            height: 0i32,
            attributes: None,
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
///
/// The lifetime of this handle is tied to the originating [`Csound`]
/// instance that created it. Dropping the `Csound` invalidates the
/// underlying channel pointer.
pub struct InputChannel<'a, T: IsChannel> {
    inner: ChannelCore<T, InputPhantom<'a, T>>,
}

/// Csound output channel - allows reading data from csound.
///
/// This struct wraps a pointer to csound's internal channel buffer.
/// Use the explicit `get()`, `read()`, and slice accessor methods
/// rather than relying on implicit dereferencing.
///
/// The lifetime of this handle is tied to the originating [`Csound`]
/// instance that created it. Dropping the `Csound` invalidates the
/// underlying channel pointer.
pub struct OutputChannel<'a, T: IsChannel> {
    inner: ChannelCore<T, OutputPhantom<'a, T>>,
}

struct ChannelCore<T: IsChannel, P> {
    csound: NonNull<csound_sys::CSOUND>,
    name: CString,
    ptr: NonNull<T::Raw>,
    len: usize,
    phantom: PhantomData<P>,
}

impl<'a, T: IsChannel> std::fmt::Debug for InputChannel<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ptr = self.inner.ptr.as_ptr();
        f.debug_struct("InputChannel")
            .field("name", &self.inner.name)
            .field("ptr", &ptr)
            .field("len", &self.inner.len)
            .field("channel_type", &T::c_type())
            .finish()
    }
}

impl<'a, T: IsChannel> std::fmt::Debug for OutputChannel<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ptr = self.inner.ptr.as_ptr();
        f.debug_struct("OutputChannel")
            .field("name", &self.inner.name)
            .field("ptr", &ptr)
            .field("len", &self.inner.len)
            .field("channel_type", &T::c_type())
            .finish()
    }
}

impl<T: IsChannel, P> ChannelCore<T, P> {
    /// Creates a new ChannelCore from a raw pointer.
    ///
    /// # Safety
    /// The pointer must be valid and point to memory owned by csound.
    unsafe fn new(
        csound: *mut csound_sys::CSOUND,
        name: CString,
        ptr: *mut T::Raw,
        len: usize,
    ) -> Option<Self> {
        let csound = NonNull::new(csound)?;
        NonNull::new(ptr).map(|ptr| ChannelCore {
            csound,
            name,
            ptr,
            len,
            phantom: PhantomData,
        })
    }

    /// Returns the length of the channel buffer.
    #[inline]
    fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the channel buffer is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Guard that locks a channel for safe pointer access.
#[must_use = "ChannelLock unlocks on drop; keep it alive for the duration of channel access"]
#[derive(Debug)]
pub struct ChannelLock<'a, C> {
    csound: *mut csound_sys::CSOUND,
    name: *const c_char,
    channel: &'a C,
}

impl<'a, C> ChannelLock<'a, C> {
    fn new(csound: *mut csound_sys::CSOUND, name: *const c_char, channel: &'a C) -> Self {
        unsafe {
            csound_sys::csoundLockChannel(csound, name);
        }
        ChannelLock {
            csound,
            name,
            channel,
        }
    }
}

impl<C> Drop for ChannelLock<'_, C> {
    fn drop(&mut self) {
        unsafe {
            csound_sys::csoundUnlockChannel(self.csound, self.name);
        }
    }
}

// SAFETY: Channel pointers are tied to the Csound instance lifetime
// and access is synchronized through csound's own mechanisms.
unsafe impl<T: IsChannel> Send for InputChannel<'_, T> {}
unsafe impl<T: IsChannel> Send for OutputChannel<'_, T> {}
// SAFETY: Shared access is safe because channel reads/writes are synchronized
// via csound's channel lock (or require unsafe caller synchronization).
unsafe impl<T: IsChannel> Sync for InputChannel<'_, T> {}
unsafe impl<T: IsChannel> Sync for OutputChannel<'_, T> {}

impl<'a, T: IsChannel> InputChannel<'a, T> {
    /// Creates a new InputChannel from a raw pointer.
    ///
    /// # Safety
    /// The pointer must be valid and point to memory owned by csound.
    pub(crate) unsafe fn from_raw(
        csound: *mut csound_sys::CSOUND,
        name: CString,
        ptr: *mut T::Raw,
        len: usize,
    ) -> Option<Self> {
        unsafe { ChannelCore::new(csound, name, ptr, len) }.map(|inner| InputChannel { inner })
    }

    /// Returns the length of the channel buffer.
    ///
    /// For string channels, the buffer size can change at runtime. Use
    /// `ChannelLock::len()` or `as_bytes().len()` to get the current string length,
    /// and `capacity_bytes()` to get the buffer capacity.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the channel buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Locks the channel and returns a guard for safe access.
    ///
    /// This call blocks (spin-waits) until the channel lock is available.
    ///
    /// # Panics / Deadlock
    /// Do not call `lock()` re-entrantly on the same channel from the same thread.
    /// The Csound channel lock is non-recursive and will deadlock.
    #[inline]
    pub fn lock(&self) -> ChannelLock<'_, Self> {
        ChannelLock::new(self.inner.csound.as_ptr(), self.inner.name.as_ptr(), self)
    }

    /// Locks the channel, runs the closure, and releases the lock.
    ///
    /// This call blocks (spin-waits) until the channel lock is available.
    /// This ensures the lock is held for exactly the duration of `f`,
    /// preventing references from escaping beyond the lock scope.
    ///
    /// # Panics / Deadlock
    /// Do not call `with_lock()` re-entrantly on the same channel from the same thread.
    /// The Csound channel lock is non-recursive and will deadlock.
    #[inline]
    pub fn with_lock<R>(&self, f: impl FnOnce(ChannelLock<'_, Self>) -> R) -> R {
        f(self.lock())
    }
}

impl<'a, T: IsChannel> OutputChannel<'a, T> {
    /// Creates a new OutputChannel from a raw pointer.
    ///
    /// # Safety
    /// The pointer must be valid and point to memory owned by csound.
    pub(crate) unsafe fn from_raw(
        csound: *mut csound_sys::CSOUND,
        name: CString,
        ptr: *mut T::Raw,
        len: usize,
    ) -> Option<Self> {
        unsafe { ChannelCore::new(csound, name, ptr, len) }.map(|inner| OutputChannel { inner })
    }

    /// Returns the length of the channel buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the channel buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Locks the channel and returns a guard for safe access.
    ///
    /// This call blocks (spin-waits) until the channel lock is available.
    ///
    /// # Panics / Deadlock
    /// Do not call `lock()` re-entrantly on the same channel from the same thread.
    /// The Csound channel lock is non-recursive and will deadlock.
    #[inline]
    pub fn lock(&self) -> ChannelLock<'_, Self> {
        ChannelLock::new(self.inner.csound.as_ptr(), self.inner.name.as_ptr(), self)
    }

    /// Locks the channel, runs the closure, and releases the lock.
    ///
    /// This call blocks (spin-waits) until the channel lock is available.
    /// This ensures the lock is held for exactly the duration of `f`,
    /// preventing references from escaping beyond the lock scope.
    ///
    /// # Panics / Deadlock
    /// Do not call `with_lock()` re-entrantly on the same channel from the same thread.
    /// The Csound channel lock is non-recursive and will deadlock.
    #[inline]
    pub fn with_lock<R>(&self, f: impl FnOnce(ChannelLock<'_, Self>) -> R) -> R {
        f(self.lock())
    }
}

pub trait IsChannel: sealed::Sealed {
    type Raw;
    fn c_type() -> ControlChannelType;
}

impl sealed::Sealed for ControlChannel {}
impl IsChannel for ControlChannel {
    type Raw = Myflt;
    fn c_type() -> ControlChannelType {
        ControlChannelType::Control
    }
}

impl sealed::Sealed for AudioChannel {}
impl IsChannel for AudioChannel {
    type Raw = Myflt;
    fn c_type() -> ControlChannelType {
        ControlChannelType::Audio
    }
}

impl sealed::Sealed for StrChannel {}
impl IsChannel for StrChannel {
    type Raw = STRINGDAT;
    fn c_type() -> ControlChannelType {
        ControlChannelType::String
    }
}

// ============================================================================
// CONTROL CHANNEL implementations
// ============================================================================

impl<'a> InputChannel<'a, ControlChannel> {
    /// Gets the current value of the control channel.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn get(&self) -> Myflt {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { *self.inner.ptr.as_ptr() }
    }

    /// Sets the value of the control channel.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn set(&self, value: Myflt) {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe {
            *self.inner.ptr.as_ptr() = value;
        }
    }

    /// Writes a value to the control channel (alias for `set`).
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn write(&self, value: Myflt) {
        unsafe { self.set(value) };
    }
}

impl<'a> OutputChannel<'a, ControlChannel> {
    /// Gets the current value of the control channel.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn get(&self) -> Myflt {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { *self.inner.ptr.as_ptr() }
    }

    /// Reads the value from the control channel (alias for `get`).
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn read(&self) -> Myflt {
        unsafe { self.get() }
    }
}

impl<'lock, 'chan> ChannelLock<'lock, InputChannel<'chan, ControlChannel>> {
    /// Gets the current value of the control channel.
    #[inline]
    pub fn get(&self) -> Myflt {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.get() }
    }

    /// Sets the value of the control channel.
    #[inline]
    pub fn set(&mut self, value: Myflt) {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.set(value) }
    }

    /// Writes a value to the control channel (alias for `set`).
    #[inline]
    pub fn write(&mut self, value: Myflt) {
        self.set(value);
    }
}

impl<'lock, 'chan> ChannelLock<'lock, OutputChannel<'chan, ControlChannel>> {
    /// Gets the current value of the control channel.
    #[inline]
    pub fn get(&self) -> Myflt {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.get() }
    }

    /// Reads the value from the control channel (alias for `get`).
    #[inline]
    pub fn read(&self) -> Myflt {
        self.get()
    }
}

// ============================================================================
// AUDIO CHANNEL implementations
// ============================================================================

impl<'a> InputChannel<'a, AudioChannel> {
    /// Returns a mutable slice of the audio channel's samples.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked and no aliasing occurs with csound's internal access.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn as_mut_slice(&self) -> &mut [Myflt] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts_mut(self.inner.ptr.as_ptr(), self.inner.len) }
    }

    /// Writes audio samples to the channel.
    ///
    /// If the input slice is longer than the channel buffer,
    /// only `len()` samples will be copied.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    pub unsafe fn write(&self, samples: &[Myflt]) -> usize {
        let copy_len = samples.len().min(self.inner.len);
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe {
            std::ptr::copy_nonoverlapping(samples.as_ptr(), self.inner.ptr.as_ptr(), copy_len);
        };
        copy_len
    }
}

impl<'a> OutputChannel<'a, AudioChannel> {
    /// Returns an immutable slice of the audio channel's samples.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn as_slice(&self) -> &[Myflt] {
        // SAFETY: pointer is guaranteed non-null and valid for the channel lifetime
        unsafe { slice::from_raw_parts(self.inner.ptr.as_ptr(), self.inner.len) }
    }

    /// Reads the audio samples from the channel (alias for `as_slice`).
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn read(&self) -> &[Myflt] {
        unsafe { self.as_slice() }
    }
}

impl<'lock, 'chan> ChannelLock<'lock, InputChannel<'chan, AudioChannel>> {
    /// Returns a mutable slice of the audio channel's samples.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Myflt] {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.as_mut_slice() }
    }

    /// Writes audio samples to the channel.
    #[inline]
    pub fn write(&mut self, samples: &[Myflt]) -> usize {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.write(samples) }
    }
}

impl<'lock, 'chan> ChannelLock<'lock, OutputChannel<'chan, AudioChannel>> {
    /// Returns an immutable slice of the audio channel's samples.
    #[inline]
    pub fn as_slice(&self) -> &[Myflt] {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.as_slice() }
    }

    /// Reads the audio samples from the channel (alias for `as_slice`).
    #[inline]
    pub fn read(&self) -> &[Myflt] {
        self.as_slice()
    }
}

// ============================================================================
// STRING CHANNEL implementations
// ============================================================================

impl<'a> InputChannel<'a, StrChannel> {
    /// Returns the current string buffer capacity in bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn capacity_bytes(&self) -> usize {
        let size = unsafe {
            csound_sys::csoundGetChannelDatasize(
                self.inner.csound.as_ptr(),
                self.inner.name.as_ptr(),
            )
        };
        if size <= 0 { 0 } else { size as usize }
    }

    /// Returns an immutable slice of the string channel's bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn as_slice(&self) -> &[u8] {
        let data = unsafe {
            csound_sys::ffi_bindgen::csoundGetStringData(
                self.inner.csound.as_ptr(),
                self.inner.ptr.as_ptr(),
            )
        };
        if data.is_null() {
            return &[];
        }
        // SAFETY: pointer is a valid NUL-terminated string
        unsafe { CStr::from_ptr(data).to_bytes() }
    }

    /// Returns a mutable slice of the string channel's bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked and no aliasing occurs with csound's internal access.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn as_mut_slice(&self) -> &mut [u8] {
        let data = unsafe {
            csound_sys::ffi_bindgen::csoundGetStringData(
                self.inner.csound.as_ptr(),
                self.inner.ptr.as_ptr(),
            )
        };
        let size = unsafe { self.capacity_bytes() };
        if data.is_null() || size == 0 {
            return &mut [];
        }
        // SAFETY: caller guarantees exclusive access and size bytes are valid
        unsafe { slice::from_raw_parts_mut(data as *mut u8, size) }
    }

    /// Writes a Rust string to the channel, ensuring NUL termination and zeroing the remainder.
    ///
    /// # Errors
    /// - [`Error::Nul`] if the string contains an interior NUL byte
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    pub unsafe fn write_str(&self, value: &str) -> Result<()> {
        let cstr = CString::new(value)?;
        unsafe {
            csound_sys::ffi_bindgen::csoundSetStringData(
                self.inner.csound.as_ptr(),
                self.inner.ptr.as_ptr(),
                cstr.as_ptr(),
            );
        }
        Ok(())
    }
}

impl<'a> OutputChannel<'a, StrChannel> {
    /// Returns the current string buffer capacity in bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn capacity_bytes(&self) -> usize {
        let size = unsafe {
            csound_sys::csoundGetChannelDatasize(
                self.inner.csound.as_ptr(),
                self.inner.name.as_ptr(),
            )
        };
        if size <= 0 { 0 } else { size as usize }
    }

    /// Returns an immutable slice of the string channel's bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn as_slice(&self) -> &[u8] {
        let data = unsafe {
            csound_sys::ffi_bindgen::csoundGetStringData(
                self.inner.csound.as_ptr(),
                self.inner.ptr.as_ptr(),
            )
        };
        if data.is_null() {
            return &[];
        }
        // SAFETY: pointer is a valid NUL-terminated string
        unsafe { CStr::from_ptr(data).to_bytes() }
    }

    /// Reads the string channel's raw bytes (alias for `as_slice`).
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn read(&self) -> &[u8] {
        unsafe { self.as_slice() }
    }
}

impl<'lock, 'chan> ChannelLock<'lock, InputChannel<'chan, StrChannel>> {
    /// Returns the current string length in bytes (excluding the trailing NUL).
    #[inline]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns true if the string channel is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the current string buffer capacity in bytes.
    #[inline]
    pub fn capacity_bytes(&self) -> usize {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.capacity_bytes() }
    }

    /// Returns the string channel's content as a `&str`.
    #[inline]
    pub fn as_str(&self) -> Result<&str> {
        // SAFETY: channel is locked by this guard
        let bytes = unsafe { self.channel.as_slice() };
        Ok(std::str::from_utf8(bytes)?)
    }

    /// Returns the string channel's content as raw bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.as_slice() }
    }

    /// Writes a Rust string to the channel, ensuring NUL termination and zeroing the remainder.
    #[inline]
    pub fn write_str(&mut self, value: &str) -> Result<()> {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.write_str(value) }
    }
}

impl<'lock, 'chan> ChannelLock<'lock, OutputChannel<'chan, StrChannel>> {
    /// Returns the current string length in bytes (excluding the trailing NUL).
    #[inline]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns true if the string channel is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the current string buffer capacity in bytes.
    #[inline]
    pub fn capacity_bytes(&self) -> usize {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.capacity_bytes() }
    }

    /// Returns the string channel's content as a `&str`.
    #[inline]
    pub fn as_str(&self) -> Result<&str> {
        // SAFETY: channel is locked by this guard
        let bytes = unsafe { self.channel.as_slice() };
        Ok(std::str::from_utf8(bytes)?)
    }

    /// Returns the string channel's content as raw bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: channel is locked by this guard
        unsafe { self.channel.as_slice() }
    }

    /// Reads the string channel's content as a `&str`.
    #[inline]
    pub fn read(&self) -> Result<&str> {
        self.as_str()
    }
}
