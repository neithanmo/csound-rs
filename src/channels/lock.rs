use std::marker::PhantomData;

use super::handle::{ChannelDir, ChannelSpec};
use super::named_channel::{NamedChannel, NamedChannelLock};

/// Guard that locks a channel for safe live-pointer access.
#[must_use = "ChannelLock unlocks on drop; keep it alive for the duration of channel access"]
#[derive(Debug)]
pub struct ChannelLock<'lock, 'chan, S: ChannelSpec, D: ChannelDir> {
    lock: NamedChannelLock<'lock, 'chan>,
    ptr: *mut S::Raw,
    len: usize,
    _dir: PhantomData<D>,
}

impl<'lock, 'chan, S: ChannelSpec, D: ChannelDir> ChannelLock<'lock, 'chan, S, D> {
    pub(super) fn new(channel: &'lock NamedChannel<'chan>, ptr: *mut S::Raw, len: usize) -> Self {
        ChannelLock {
            lock: channel.lock(),
            ptr,
            len,
            _dir: PhantomData,
        }
    }

    #[inline]
    pub(super) fn raw_ptr(&self) -> *mut S::Raw {
        self.ptr
    }

    #[inline]
    pub(super) fn buffer_len(&self) -> usize {
        self.len
    }

    #[inline]
    pub(super) fn csound_ptr(&self) -> *mut csound_sys::CSOUND {
        self.lock.csound_ptr()
    }

    #[inline]
    pub(super) fn name_ptr(&self) -> *const libc::c_char {
        self.lock.name_ptr()
    }
}
