//! PVS channel support.
//!
//! This module provides a safe wrapper around Csound PVS channels using the
//! public C API. The native `PVSDAT` structure is opaque in the bindings, so
//! frame access is done via `csoundGetPvsData()` and a conservative length
//! of `fft_size + 2` floats.
//!
//! Note: Sliding PVS frames can be larger than `fft_size + 2`. This wrapper
//! intentionally exposes only the leading `fft_size + 2` samples, which is
//! compatible with `csoundInitPvsChannel()` and non-sliding PVS channels.

use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::slice;

use libc::{c_char, c_int, c_void};

use crate::Csound;
use crate::enums::Status;
use crate::error::{Error, Result};

use csound_sys::controlChannelType;
use csound_sys::ffi_bindgen::PVSDAT;
use csound_sys::ffi_bindgen::csoundGetPvsData;
use csound_sys::ffi_bindgen::csoundInitPvsChannel;
use csound_sys::ffi_bindgen::csoundPvsDataFFTSize;
use csound_sys::ffi_bindgen::csoundPvsDataFormat;
use csound_sys::ffi_bindgen::csoundPvsDataFramecount;
use csound_sys::ffi_bindgen::csoundPvsDataOverlap;
use csound_sys::ffi_bindgen::csoundPvsDataWindowSize;

const PVS_FRAME_GUARD: u32 = 2;

/// PVS window types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PvsWindowType {
    Hamming,
    Hann,
    Kaiser,
    Custom,
    Blackman,
    BlackmanExact,
    NuttallC3,
    BlackmanHarris3,
    BlackmanHarrisMin,
    Rect,
    Unknown(u32),
}

impl From<u32> for PvsWindowType {
    fn from(value: u32) -> Self {
        match value {
            0 => PvsWindowType::Hamming,
            1 => PvsWindowType::Hann,
            2 => PvsWindowType::Kaiser,
            3 => PvsWindowType::Custom,
            4 => PvsWindowType::Blackman,
            5 => PvsWindowType::BlackmanExact,
            6 => PvsWindowType::NuttallC3,
            7 => PvsWindowType::BlackmanHarris3,
            8 => PvsWindowType::BlackmanHarrisMin,
            9 => PvsWindowType::Rect,
            other => PvsWindowType::Unknown(other),
        }
    }
}

impl PvsWindowType {
    pub fn to_u32(self) -> u32 {
        match self {
            PvsWindowType::Hamming => 0,
            PvsWindowType::Hann => 1,
            PvsWindowType::Kaiser => 2,
            PvsWindowType::Custom => 3,
            PvsWindowType::Blackman => 4,
            PvsWindowType::BlackmanExact => 5,
            PvsWindowType::NuttallC3 => 6,
            PvsWindowType::BlackmanHarris3 => 7,
            PvsWindowType::BlackmanHarrisMin => 8,
            PvsWindowType::Rect => 9,
            PvsWindowType::Unknown(v) => v,
        }
    }
}

/// PVS analysis data formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PvsFormat {
    AmpFreq,
    AmpPhase,
    Complex,
    Tracks,
    Unknown(u32),
}

impl From<u32> for PvsFormat {
    fn from(value: u32) -> Self {
        match value {
            0 => PvsFormat::AmpFreq,
            1 => PvsFormat::AmpPhase,
            2 => PvsFormat::Complex,
            3 => PvsFormat::Tracks,
            other => PvsFormat::Unknown(other),
        }
    }
}

impl PvsFormat {
    pub fn to_u32(self) -> u32 {
        match self {
            PvsFormat::AmpFreq => 0,
            PvsFormat::AmpPhase => 1,
            PvsFormat::Complex => 2,
            PvsFormat::Tracks => 3,
            PvsFormat::Unknown(v) => v,
        }
    }
}

/// Parameters for initializing a PVS channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PvsChannelParams {
    pub fft_size: u32,
    pub overlap: u32,
    pub window_size: u32,
    pub window_type: PvsWindowType,
    pub format: PvsFormat,
}

impl PvsChannelParams {
    pub fn new(
        fft_size: u32,
        overlap: u32,
        window_size: u32,
        window_type: PvsWindowType,
        format: PvsFormat,
    ) -> Self {
        PvsChannelParams {
            fft_size,
            overlap,
            window_size,
            window_type,
            format,
        }
    }

    pub fn frame_len(&self) -> usize {
        frame_len_from_fft(self.fft_size)
    }
}

/// Basic metadata about a PVS channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PvsChannelInfo {
    pub fft_size: u32,
    pub overlap: u32,
    pub window_size: u32,
    pub format: PvsFormat,
    pub framecount: u32,
    pub frame_len: usize,
}

/// Snapshot of a PVS frame and its metadata.
#[derive(Debug, Clone)]
pub struct PvsFrame {
    pub fft_size: u32,
    pub overlap: u32,
    pub window_size: u32,
    pub format: PvsFormat,
    pub framecount: u32,
    pub frame: Vec<f32>,
}

impl PvsFrame {
    pub fn frame_len(&self) -> usize {
        self.frame.len()
    }
}

/// Handle to a PVS channel.
///
/// This wrapper assumes a non-sliding PVS frame layout and exposes a frame
/// length of `fft_size + 2` floats. This is compatible with
/// `csoundInitPvsChannel()` and safe for reading/writing the leading portion
/// of larger frames.
#[derive(Debug)]
pub struct PvsChannel<'a> {
    csound: NonNull<csound_sys::CSOUND>,
    name: CString,
    pvs: NonNull<PVSDAT>,
    phantom: PhantomData<&'a csound_sys::CSOUND>,
}

impl<'a> PvsChannel<'a> {
    pub(crate) unsafe fn from_raw(
        csound: *mut csound_sys::CSOUND,
        name: CString,
        pvs: *mut PVSDAT,
    ) -> Option<Self> {
        let csound = NonNull::new(csound)?;
        let pvs = NonNull::new(pvs)?;
        Some(PvsChannel {
            csound,
            name,
            pvs,
            phantom: PhantomData,
        })
    }

    /// Returns the channel name as UTF-8.
    ///
    /// Channels created inside Csound may contain non-UTF-8 names; in that case
    /// this returns [`Error::UtfError`].
    #[inline]
    pub fn name(&self) -> Result<&str> {
        self.name.to_str().map_err(Error::from)
    }

    /// Locks the channel and returns a guard for safe access.
    ///
    /// This call blocks (spin-waits) until the channel lock is available.
    ///
    /// # Panics / Deadlock
    /// Do not call `lock()` re-entrantly on the same channel from the same thread.
    /// The Csound channel lock is non-recursive and will deadlock.
    #[inline]
    pub fn lock(&self) -> PvsChannelLock<'_, 'a> {
        PvsChannelLock::new(self.csound.as_ptr(), self.name.as_ptr(), self.pvs.as_ptr())
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
    pub fn with_lock<R>(&self, f: impl for<'lock> FnOnce(PvsChannelLock<'lock, 'a>) -> R) -> R {
        f(self.lock())
    }

    /// Returns a snapshot of the channel's metadata (locks internally).
    #[inline]
    pub fn info(&self) -> PvsChannelInfo {
        self.with_lock(|lock| lock.info())
    }

    /// Reads the current frame into a newly allocated buffer (locks internally).
    #[inline]
    pub fn read_frame(&self) -> PvsFrame {
        self.with_lock(|lock| lock.read_frame())
    }

    /// Writes the provided frame into the channel (locks internally).
    ///
    /// Returns the number of elements copied.
    #[inline]
    pub fn write_frame(&self, frame: &[f32]) -> usize {
        self.with_lock(|mut lock| lock.write(frame))
    }
}

/// Guard that locks a PVS channel for safe access.
#[must_use = "PvsChannelLock unlocks on drop; keep it alive for the duration of channel access"]
#[derive(Debug)]
pub struct PvsChannelLock<'lock, 'chan> {
    csound: *mut csound_sys::CSOUND,
    name: *const c_char,
    pvs: *mut PVSDAT,
    frame_len: usize,
    _marker: PhantomData<&'lock PvsChannel<'chan>>,
}

impl<'lock, 'chan> From<&PvsChannelLock<'lock, 'chan>> for PvsChannelInfo {
    fn from(value: &PvsChannelLock<'lock, 'chan>) -> PvsChannelInfo {
        value.info()
    }
}

impl<'lock, 'chan> PvsChannelLock<'lock, 'chan> {
    fn new(csound: *mut csound_sys::CSOUND, name: *const c_char, pvs: *mut PVSDAT) -> Self {
        unsafe {
            csound_sys::csoundLockChannel(csound, name);
        }
        let frame_len = frame_len_from_fft(pvs_fft_size(pvs));
        PvsChannelLock {
            csound,
            name,
            pvs,
            frame_len,
            _marker: PhantomData,
        }
    }

    /// Returns a snapshot of the channel's metadata.
    #[inline]
    pub fn info(&self) -> PvsChannelInfo {
        let fft_size = pvs_fft_size(self.pvs);
        let overlap = pvs_overlap(self.pvs);
        let window_size = pvs_window_size(self.pvs);
        let format = pvs_format(self.pvs);
        let framecount = pvs_framecount(self.pvs);
        let frame_len = self.frame_len;
        PvsChannelInfo {
            fft_size,
            overlap,
            window_size,
            format,
            framecount,
            frame_len,
        }
    }

    /// Returns the FFT size (N) used by the channel.
    #[inline]
    pub fn fft_size(&self) -> u32 {
        pvs_fft_size(self.pvs)
    }

    /// Returns the overlap size used by the channel.
    #[inline]
    pub fn overlap(&self) -> u32 {
        pvs_overlap(self.pvs)
    }

    /// Returns the analysis window size used by the channel.
    #[inline]
    pub fn window_size(&self) -> u32 {
        pvs_window_size(self.pvs)
    }

    /// Returns the analysis data format used by the channel.
    #[inline]
    pub fn format(&self) -> PvsFormat {
        pvs_format(self.pvs)
    }

    /// Returns the current framecount.
    #[inline]
    pub fn framecount(&self) -> u32 {
        pvs_framecount(self.pvs)
    }

    /// Returns the exposed frame length (fft_size + 2).
    #[inline]
    pub fn frame_len(&self) -> usize {
        self.frame_len
    }

    /// Returns an immutable slice of the frame data.
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        let ptr = self.frame_ptr();
        let len = self.frame_len();
        if ptr.is_null() || len == 0 {
            return &[];
        }
        unsafe { slice::from_raw_parts(ptr, len) }
    }

    /// Returns a mutable slice of the frame data.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        let ptr = self.frame_ptr();
        let len = self.frame_len();
        if ptr.is_null() || len == 0 {
            return &mut [];
        }
        unsafe { slice::from_raw_parts_mut(ptr as *mut f32, len) }
    }

    /// Copies the current frame into `output`.
    ///
    /// Returns the number of elements copied.
    pub fn read(&self, output: &mut [f32]) -> usize {
        let src = self.as_slice();
        let len = output.len().min(src.len());
        if len == 0 {
            return 0;
        }
        output[..len].copy_from_slice(&src[..len]);
        len
    }

    /// Writes `input` into the current frame.
    ///
    /// Returns the number of elements copied.
    pub fn write(&mut self, input: &[f32]) -> usize {
        let dst = self.as_mut_slice();
        let len = input.len().min(dst.len());
        if len == 0 {
            return 0;
        }
        dst[..len].copy_from_slice(&input[..len]);
        len
    }

    /// Reads the current frame into an owned buffer.
    pub fn read_frame(&self) -> PvsFrame {
        let info = self.info();
        let mut frame = vec![0.0f32; info.frame_len];
        self.read(&mut frame);
        PvsFrame {
            fft_size: info.fft_size,
            overlap: info.overlap,
            window_size: info.window_size,
            format: info.format,
            framecount: info.framecount,
            frame,
        }
    }

    fn frame_ptr(&self) -> *const f32 {
        pvs_frame_ptr(self.pvs)
    }
}

impl Drop for PvsChannelLock<'_, '_> {
    fn drop(&mut self) {
        unsafe {
            csound_sys::csoundUnlockChannel(self.csound, self.name);
        }
    }
}

// SAFETY: PVS channel pointers are tied to the Csound instance lifetime
// and access is synchronized through csound's own mechanisms.
unsafe impl Send for PvsChannel<'_> {}
// SAFETY: Shared access is safe because channel reads/writes are synchronized
// via csound's channel lock.
unsafe impl Sync for PvsChannel<'_> {}

impl Csound {
    /// Creates or initializes a PVS channel and returns a handle.
    ///
    /// If the channel already exists and has been initialized, Csound treats this
    /// as a no-op and returns the existing channel. This method validates the
    /// observable parameters (FFT size, overlap, window size, format) and returns
    /// an error if they do not match the requested values.
    ///
    /// # Errors
    /// - [`Error::EmptyString`] if `name` is empty
    /// - [`Error::Nul`] if `name` contains an interior NUL byte
    /// - [`Error::InvalidArgument`] if parameters are out of range or mismatch
    /// - [`Error::NullPointer`] if the channel could not be initialized
    pub fn init_pvs_channel(&self, name: &str, params: PvsChannelParams) -> Result<PvsChannel<'_>> {
        if name.is_empty() {
            return Err(Error::EmptyString);
        }

        let fft_size = to_i32(params.fft_size, "fft_size out of range")?;
        let overlap = to_i32(params.overlap, "overlap out of range")?;
        let window_size = to_i32(params.window_size, "window_size out of range")?;
        let window_type = to_i32(params.window_type.to_u32(), "window_type out of range")?;
        let format = to_i32(params.format.to_u32(), "format out of range")?;
        if params.fft_size == 0 {
            return Err(Error::InvalidArgument("fft_size must be > 0"));
        }
        if params.window_size == 0 {
            return Err(Error::InvalidArgument("window_size must be > 0"));
        }

        let cname = CString::new(name)?;
        let pvs = unsafe {
            csoundInitPvsChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                fft_size,
                overlap,
                window_size,
                window_type,
                format,
            )
        };

        let channel = unsafe { PvsChannel::from_raw(self.csound_ptr(), cname, pvs) }
            .ok_or(Error::NullPointer("failed to initialize PVS channel"))?;

        let info = channel.info();
        if info.fft_size != params.fft_size
            || info.overlap != params.overlap
            || info.window_size != params.window_size
            || info.format != params.format
        {
            return Err(Error::InvalidArgument(
                "PVS channel exists with different parameters",
            ));
        }

        if channel.with_lock(|lock| lock.frame_ptr().is_null()) {
            return Err(Error::BufferNotInitialized(
                "PVS channel frame not initialized",
            ));
        }

        Ok(channel)
    }

    /// Returns a handle to a PVS channel, creating it if necessary.
    ///
    /// If the channel exists but its frame is uninitialized, this returns
    /// [`Error::BufferNotInitialized`]. Use [`Csound::init_pvs_channel`] or ensure
    /// the orchestra has created the channel before calling this.
    ///
    /// # Errors
    /// - [`Error::EmptyString`] if `name` is empty
    /// - [`Error::Nul`] if `name` contains an interior NUL byte
    /// - [`Error::Memory`] if channel allocation failed
    /// - [`Error::InvalidArgument`] if the name or type is invalid
    /// - [`Error::ChannelTypeMismatch`] if an incompatible channel exists
    /// - [`Error::BufferNotInitialized`] if the PVS frame is missing
    pub fn get_pvs_channel(&self, name: &str) -> Result<PvsChannel<'_>> {
        if name.is_empty() {
            return Err(Error::EmptyString);
        }

        let mut ptr: *mut c_void = std::ptr::null_mut();
        let ptr_ref = &mut ptr as *mut *mut c_void;
        let bits = (controlChannelType::CSOUND_PVS_CHANNEL
            | controlChannelType::CSOUND_INPUT_CHANNEL
            | controlChannelType::CSOUND_OUTPUT_CHANNEL) as c_int;

        let cname = CString::new(name)?;
        let status = self.get_raw_channel_ptr(&cname, ptr_ref, bits);
        match Status::from(status) {
            Status::Success => {
                let channel =
                    unsafe { PvsChannel::from_raw(self.csound_ptr(), cname, ptr as *mut PVSDAT) }
                        .ok_or(Error::NullPointer("failed to create PVS channel"))?;

                if channel.with_lock(|lock| lock.frame_ptr().is_null()) {
                    return Err(Error::BufferNotInitialized(
                        "PVS channel frame not initialized",
                    ));
                }

                Ok(channel)
            }
            Status::Memory => Err(Error::Memory),
            Status::Error => Err(Error::InvalidArgument("invalid channel name or type")),
            Status::Ok(existing_type) => Err(Error::ChannelTypeMismatch(existing_type)),
            _ => Err(Error::OperationFailed),
        }
    }
}

fn pvs_frame_ptr(pvs: *mut PVSDAT) -> *const f32 {
    unsafe { csoundGetPvsData(pvs) }
}

fn pvs_fft_size(pvs: *mut PVSDAT) -> u32 {
    let value = unsafe { csoundPvsDataFFTSize(pvs) };
    clamp_i32_to_u32(value)
}

fn pvs_overlap(pvs: *mut PVSDAT) -> u32 {
    let value = unsafe { csoundPvsDataOverlap(pvs) };
    clamp_i32_to_u32(value)
}

fn pvs_window_size(pvs: *mut PVSDAT) -> u32 {
    let value = unsafe { csoundPvsDataWindowSize(pvs) };
    clamp_i32_to_u32(value)
}

fn pvs_format(pvs: *mut PVSDAT) -> PvsFormat {
    let value = unsafe { csoundPvsDataFormat(pvs) };
    if value < 0 {
        PvsFormat::Unknown(value as u32)
    } else {
        PvsFormat::from(value as u32)
    }
}

fn pvs_framecount(pvs: *mut PVSDAT) -> u32 {
    unsafe { csoundPvsDataFramecount(pvs) }
}

fn frame_len_from_fft(fft_size: u32) -> usize {
    fft_size.saturating_add(PVS_FRAME_GUARD) as usize
}

fn clamp_i32_to_u32(value: i32) -> u32 {
    if value <= 0 { 0 } else { value as u32 }
}

fn to_i32(value: u32, msg: &'static str) -> Result<i32> {
    i32::try_from(value).map_err(|_| Error::InvalidArgument(msg))
}
