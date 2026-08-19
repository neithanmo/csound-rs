use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use libc::c_char;

use crate::{Csound, Error, Result};

/// Common identity and Csound-lifetime state for a named channel.
#[derive(Debug)]
pub(super) struct NamedChannel<'cs> {
    csound: NonNull<csound_sys::CSOUND>,
    name: CString,
    _csound: PhantomData<&'cs csound_sys::CSOUND>,
}

impl<'cs> NamedChannel<'cs> {
    /// Creates a channel identity tied to the borrow of its Csound instance.
    pub(super) fn new(csound: &'cs Csound, name: CString) -> Self {
        let csound = NonNull::new(csound.csound_ptr())
            .expect("a live Csound instance always has a non-null pointer");
        NamedChannel {
            csound,
            name,
            _csound: PhantomData,
        }
    }

    #[inline]
    pub(super) fn csound_ptr(&self) -> *mut csound_sys::CSOUND {
        self.csound.as_ptr()
    }

    #[inline]
    pub(super) fn name_ptr(&self) -> *const c_char {
        self.name.as_ptr()
    }

    #[inline]
    pub(super) fn name(&self) -> Result<&str> {
        self.name.to_str().map_err(Error::from)
    }

    /// Acquires the non-recursive Csound lock for this named channel.
    #[inline]
    pub(super) fn lock(&self) -> NamedChannelLock<'_, 'cs> {
        unsafe {
            csound_sys::csoundLockChannel(self.csound_ptr(), self.name_ptr());
        }
        NamedChannelLock { channel: self }
    }
}

/// Internal RAII token owning one acquisition of a named channel lock.
#[must_use = "the named channel is unlocked when this token is dropped"]
#[derive(Debug)]
pub(super) struct NamedChannelLock<'lock, 'cs> {
    channel: &'lock NamedChannel<'cs>,
}

impl NamedChannelLock<'_, '_> {
    #[inline]
    pub(super) fn csound_ptr(&self) -> *mut csound_sys::CSOUND {
        self.channel.csound_ptr()
    }

    #[inline]
    pub(super) fn name_ptr(&self) -> *const c_char {
        self.channel.name_ptr()
    }
}

impl Drop for NamedChannelLock<'_, '_> {
    fn drop(&mut self) {
        unsafe {
            csound_sys::csoundUnlockChannel(self.channel.csound_ptr(), self.channel.name_ptr());
        }
    }
}
