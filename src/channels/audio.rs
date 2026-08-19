//! Live-pointer audio channel access.
//!
//! `csoundGetAudioChannel()` and `csoundSetAudioChannel()` copy data while
//! locking internally. The handles in this module instead wrap the live pointer
//! returned by `csoundGetChannelPtr()`, so their safe accessors require the
//! named-channel guard.

use std::slice;

use crate::Myflt;
use crate::enums::AudioChannel;

use super::handle::{ChannelHandle, InputDir, OutputDir};
use super::lock::ChannelLock;

impl ChannelHandle<'_, AudioChannel, InputDir> {
    /// Returns the live audio buffer as a mutable slice.
    ///
    /// # Safety
    /// Caller must hold the channel lock and prevent aliasing with Csound's
    /// internal access.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn as_mut_slice(&self) -> &mut [Myflt] {
        unsafe { slice::from_raw_parts_mut(self.raw_ptr(), self.len()) }
    }

    /// Writes samples directly to the live channel buffer.
    ///
    /// Copies at most `len()` samples.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    pub unsafe fn write(&self, samples: &[Myflt]) -> usize {
        let copy_len = samples.len().min(self.len());
        if copy_len != 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(samples.as_ptr(), self.raw_ptr(), copy_len);
            }
        }
        copy_len
    }
}

impl ChannelHandle<'_, AudioChannel, OutputDir> {
    /// Returns the live audio buffer as an immutable slice.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    unsafe fn as_slice(&self) -> &[Myflt] {
        unsafe { slice::from_raw_parts(self.raw_ptr(), self.len()) }
    }

    /// Reads the live audio samples.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn read(&self) -> &[Myflt] {
        unsafe { self.as_slice() }
    }
}

impl ChannelLock<'_, '_, AudioChannel, InputDir> {
    /// Returns the audio channel's samples as a mutable slice.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Myflt] {
        unsafe { slice::from_raw_parts_mut(self.raw_ptr(), self.buffer_len()) }
    }

    /// Writes samples to the audio channel, copying at most its buffer length.
    pub fn write(&mut self, samples: &[Myflt]) -> usize {
        let copy_len = samples.len().min(self.buffer_len());
        if copy_len != 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(samples.as_ptr(), self.raw_ptr(), copy_len);
            }
        }
        copy_len
    }
}

impl ChannelLock<'_, '_, AudioChannel, OutputDir> {
    /// Returns the audio channel's samples as an immutable slice.
    #[inline]
    pub fn as_slice(&self) -> &[Myflt] {
        unsafe { slice::from_raw_parts(self.raw_ptr() as *const Myflt, self.buffer_len()) }
    }

    /// Reads the audio samples.
    #[inline]
    pub fn read(&self) -> &[Myflt] {
        self.as_slice()
    }
}
