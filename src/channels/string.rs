use std::ffi::{CStr, CString};
use std::slice;

use crate::enums::StrChannel;
use crate::error::Result;

use super::handle::{ChannelDir, ChannelHandle, InputDir, OutputDir};
use super::lock::ChannelLock;

impl<D: ChannelDir> ChannelHandle<'_, StrChannel, D> {
    /// Returns the current string buffer capacity in bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn capacity_bytes(&self) -> usize {
        let size =
            unsafe { csound_sys::csoundGetChannelDatasize(self.csound_ptr(), self.name_ptr()) };
        if size <= 0 { 0 } else { size as usize }
    }

    /// Returns the current string content as bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn as_slice(&self) -> &[u8] {
        let data = unsafe {
            csound_sys::ffi_bindgen::csoundGetStringData(self.csound_ptr(), self.raw_ptr())
        };
        if data.is_null() {
            return &[];
        }
        unsafe { CStr::from_ptr(data).to_bytes() }
    }
}

impl ChannelHandle<'_, StrChannel, InputDir> {
    /// Returns the live string allocation as a mutable byte slice.
    ///
    /// # Safety
    /// Caller must hold the channel lock and prevent aliasing with Csound's
    /// internal access. The returned slice includes the allocation capacity,
    /// not merely the current string content.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn as_mut_slice(&self) -> &mut [u8] {
        let data = unsafe {
            csound_sys::ffi_bindgen::csoundGetStringData(self.csound_ptr(), self.raw_ptr())
        };
        let size = unsafe { self.capacity_bytes() };
        if data.is_null() || size == 0 {
            return &mut [];
        }
        unsafe { slice::from_raw_parts_mut(data as *mut u8, size) }
    }

    /// Writes a Rust string directly to the live string channel.
    ///
    /// # Errors
    /// Returns [`crate::Error::Nul`] for an interior NUL byte.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    pub unsafe fn write_str(&self, value: &str) -> Result<()> {
        let cstr = CString::new(value)?;
        unsafe {
            csound_sys::ffi_bindgen::csoundSetStringData(
                self.csound_ptr(),
                self.raw_ptr(),
                cstr.as_ptr(),
            );
        }
        Ok(())
    }
}

impl ChannelHandle<'_, StrChannel, OutputDir> {
    /// Reads the string channel's raw bytes.
    ///
    /// # Safety
    /// Caller must ensure the channel is locked or otherwise synchronized.
    #[inline]
    pub unsafe fn read(&self) -> &[u8] {
        unsafe { self.as_slice() }
    }
}

impl<D: ChannelDir> ChannelLock<'_, '_, StrChannel, D> {
    /// Returns the current string length, excluding the trailing NUL.
    #[inline]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns true if the current string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the current string buffer capacity in bytes.
    #[inline]
    pub fn capacity_bytes(&self) -> usize {
        let size =
            unsafe { csound_sys::csoundGetChannelDatasize(self.csound_ptr(), self.name_ptr()) };
        if size <= 0 { 0 } else { size as usize }
    }

    /// Returns the current string content as UTF-8.
    #[inline]
    pub fn as_str(&self) -> Result<&str> {
        Ok(std::str::from_utf8(self.as_bytes())?)
    }

    /// Returns the current string content as raw bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            let data =
                csound_sys::ffi_bindgen::csoundGetStringData(self.csound_ptr(), self.raw_ptr());
            if data.is_null() {
                return &[];
            }
            CStr::from_ptr(data).to_bytes()
        }
    }
}

impl ChannelLock<'_, '_, StrChannel, InputDir> {
    /// Writes a Rust string while holding the channel lock.
    #[inline]
    pub fn write_str(&mut self, value: &str) -> Result<()> {
        let cstr = CString::new(value)?;
        unsafe {
            csound_sys::ffi_bindgen::csoundSetStringData(
                self.csound_ptr(),
                self.raw_ptr(),
                cstr.as_ptr(),
            );
        }
        Ok(())
    }
}

impl ChannelLock<'_, '_, StrChannel, OutputDir> {
    /// Reads the current string content as UTF-8.
    #[inline]
    pub fn read(&self) -> Result<&str> {
        self.as_str()
    }
}
