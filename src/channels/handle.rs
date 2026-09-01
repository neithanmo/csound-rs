use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use csound_sys::ffi_bindgen::STRINGDAT;

use crate::enums::{AudioChannel, ControlChannel, ControlChannelType, StrChannel};
use crate::{Csound, Myflt};

use super::lock::ChannelLock;
use super::named_channel::NamedChannel;

pub(super) mod sealed {
    pub trait Sealed {}
}

/// Describes the layout and type of a channel.
pub trait ChannelSpec: sealed::Sealed {
    type Raw;
    fn c_type() -> ControlChannelType;
}

/// Indicates input vs output channel behavior.
pub trait ChannelDir: sealed::Sealed {
    const FLAG: ControlChannelType;
    const DEBUG_NAME: &'static str;
    const NAME: &'static str;
}

/// Input channel direction.
#[derive(Debug, Clone, Copy)]
pub struct InputDir;

/// Output channel direction.
#[derive(Debug, Clone, Copy)]
pub struct OutputDir;

impl sealed::Sealed for InputDir {}
impl ChannelDir for InputDir {
    const FLAG: ControlChannelType = ControlChannelType::Input;
    const DEBUG_NAME: &'static str = "InputChannel";
    const NAME: &'static str = "input";
}

impl sealed::Sealed for OutputDir {}
impl ChannelDir for OutputDir {
    const FLAG: ControlChannelType = ControlChannelType::Output;
    const DEBUG_NAME: &'static str = "OutputChannel";
    const NAME: &'static str = "output";
}

/// Generic channel handle containing a live Csound channel pointer and metadata.
pub struct ChannelHandle<'chan, S: ChannelSpec, D: ChannelDir> {
    channel: NamedChannel<'chan>,
    ptr: NonNull<S::Raw>,
    len: usize,
    _spec: PhantomData<S>,
    _dir: PhantomData<D>,
}

/// Csound input channel—allows writing data to Csound.
///
/// This struct wraps a pointer to Csound's internal channel buffer. Use the
/// explicit access methods rather than relying on implicit dereferencing.
/// The lifetime is tied to the originating [`Csound`] instance.
pub type InputChannel<'a, T> = ChannelHandle<'a, T, InputDir>;

/// Csound output channel—allows reading data from Csound.
///
/// This struct wraps a pointer to Csound's internal channel buffer. Use the
/// explicit access methods rather than relying on implicit dereferencing.
/// The lifetime is tied to the originating [`Csound`] instance.
pub type OutputChannel<'a, T> = ChannelHandle<'a, T, OutputDir>;

impl<S: ChannelSpec, D: ChannelDir> std::fmt::Debug for ChannelHandle<'_, S, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(D::DEBUG_NAME)
            .field("name", &self.channel.name())
            .field("ptr", &self.ptr.as_ptr())
            .field("len", &self.len)
            .field("channel_type", &S::c_type())
            .finish()
    }
}

impl<'chan, S: ChannelSpec, D: ChannelDir> ChannelHandle<'chan, S, D> {
    /// Creates a channel handle from Csound-owned storage.
    pub(crate) fn from_raw(
        csound: &'chan Csound,
        name: CString,
        ptr: *mut S::Raw,
        len: usize,
    ) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| ChannelHandle {
            channel: NamedChannel::new(csound, name),
            ptr,
            len,
            _spec: PhantomData,
            _dir: PhantomData,
        })
    }

    /// Returns the length of the channel buffer.
    ///
    /// For string channels, the buffer size can change at runtime. Use the
    /// guard's `len()` or `as_bytes().len()` for the current string length and
    /// `capacity_bytes()` for its current allocation.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the channel buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Locks the channel and returns a guard for safe live-pointer access.
    ///
    /// This call blocks (spin-waits) until the channel lock is available.
    /// Do not call it re-entrantly for the same channel: Csound's lock is
    /// non-recursive and will deadlock.
    #[inline]
    pub fn lock(&self) -> ChannelLock<'_, 'chan, S, D> {
        ChannelLock::new(&self.channel, self.ptr.as_ptr(), self.len)
    }

    /// Locks the channel, runs `f`, and releases the lock.
    #[inline]
    pub fn with_lock<R>(
        &self,
        f: impl for<'lock> FnOnce(ChannelLock<'lock, 'chan, S, D>) -> R,
    ) -> R {
        f(self.lock())
    }

    #[inline]
    pub(super) fn raw_ptr(&self) -> *mut S::Raw {
        self.ptr.as_ptr()
    }

    #[inline]
    pub(super) fn csound_ptr(&self) -> *mut csound_sys::CSOUND {
        self.channel.csound_ptr()
    }

    #[inline]
    pub(super) fn name_ptr(&self) -> *const libc::c_char {
        self.channel.name_ptr()
    }
}

// SAFETY: Channel pointers are tied to the Csound instance lifetime and safe
// access is synchronized through Csound's named-channel lock. Raw direct
// access remains unsafe and requires caller-provided synchronization.
unsafe impl<S: ChannelSpec, D: ChannelDir> Send for ChannelHandle<'_, S, D> {}
unsafe impl<S: ChannelSpec, D: ChannelDir> Sync for ChannelHandle<'_, S, D> {}

impl sealed::Sealed for ControlChannel {}
impl ChannelSpec for ControlChannel {
    type Raw = Myflt;

    fn c_type() -> ControlChannelType {
        ControlChannelType::Control
    }
}

impl sealed::Sealed for AudioChannel {}
impl ChannelSpec for AudioChannel {
    type Raw = Myflt;

    fn c_type() -> ControlChannelType {
        ControlChannelType::Audio
    }
}

impl sealed::Sealed for StrChannel {}
impl ChannelSpec for StrChannel {
    type Raw = STRINGDAT;

    fn c_type() -> ControlChannelType {
        ControlChannelType::String
    }
}
