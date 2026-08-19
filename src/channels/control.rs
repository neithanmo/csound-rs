use crate::Myflt;
use crate::enums::ControlChannel;

use super::handle::{ChannelHandle, InputDir, OutputDir};
use super::lock::ChannelLock;

impl ChannelHandle<'_, ControlChannel, InputDir> {
    /// Writes a value to the live control channel.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn write(&self, value: Myflt) {
        unsafe {
            *self.raw_ptr() = value;
        }
    }
}

impl ChannelHandle<'_, ControlChannel, OutputDir> {
    /// Reads a value from the live control channel.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn read(&self) -> Myflt {
        unsafe { *self.raw_ptr() }
    }
}

impl ChannelLock<'_, '_, ControlChannel, InputDir> {
    /// Writes a value under the channel lock.
    #[inline]
    pub fn write(&mut self, value: Myflt) {
        unsafe {
            *self.raw_ptr() = value;
        }
    }
}

impl ChannelLock<'_, '_, ControlChannel, OutputDir> {
    /// Reads a value under the channel lock.
    #[inline]
    pub fn read(&self) -> Myflt {
        unsafe { *self.raw_ptr() }
    }
}
