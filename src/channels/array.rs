//! Array channel support.
//!
//! Array channels are new in Csound 7. They expose an orchestra array variable
//! (`k[]`, `i[]`, `a[]`, ...) to the host as an `ARRAYDAT`, which is opaque in
//! the bindings and reached only through the public accessors
//! `csoundArrayDataType()`, `csoundArrayDataDimensions()`,
//! `csoundArrayDataSizes()`, `csoundGetArrayData()` and `csoundSetArrayData()`.
//!
//! # Element layout
//!
//! Csound stores an array as `element_count` contiguous members, where
//! `element_count` is the product of the dimension sizes. Each member occupies
//! `arrayMemberSize` bytes, which the engine derives from the element type:
//!
//! - `"i"` / `"k"` — one `MYFLT` per element
//! - `"a"`         — `ksmps` `MYFLT`s per element (one audio vector)
//! - `"S"`         — a `STRINGDAT` per element (managed, see below)
//!
//! The engine applies `CS_FLOAT_ALIGN` to those sizes, but that rounds up to a
//! multiple of `sizeof(MYFLT)` and is therefore a no-op for the numeric types,
//! so this module can compute the exact buffer length from public data alone.
//!
//! # Managed element types
//!
//! `csoundSetArrayData()` rejects string, struct and other *managed* element
//! types because their values need CSOUND-aware type callbacks. This module
//! mirrors that: numeric access on a managed array returns
//! [`Error::InvalidArgument`] rather than reinterpreting `STRINGDAT` pointers
//! as samples.
//!
//! # Sizing and safety
//!
//! `csoundSetArrayData()` copies `arrayMemberSize * element_count` bytes *out
//! of the caller's pointer* and only validates the destination against its own
//! allocation. Passing a short buffer would therefore read past its end. To
//! make that unrepresentable, [`ArrayChannelLock::set_data`] requires a slice
//! whose length is exactly [`ArrayChannelLock::len`] and errors otherwise.

use std::ffi::{CStr, CString};
use std::ptr::NonNull;
use std::slice;

use libc::{c_int, c_void};

use crate::Csound;
use crate::Myflt;
use crate::enums::Status;
use crate::error::{Error, Result};

use csound_sys::controlChannelType;
use csound_sys::ffi_bindgen::ARRAYDAT;
use csound_sys::ffi_bindgen::{
    csoundArrayDataDimensions, csoundArrayDataSizes, csoundArrayDataType, csoundGetArrayData,
    csoundInitArrayChannel, csoundSetArrayData,
};

use super::named_channel::{NamedChannel, NamedChannelLock};

/// The element type of an array channel.
///
/// The variants mirror Csound's variable type names. [`ArrayType::Other`]
/// carries any further standard type name the engine reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayType {
    /// `"i"` — init-rate scalars, one `MYFLT` per element.
    Init,
    /// `"k"` — control-rate scalars, one `MYFLT` per element.
    Control,
    /// `"a"` — audio vectors, `ksmps` `MYFLT`s per element.
    Audio,
    /// `"S"` — strings. Managed; not numerically accessible.
    Str,
    /// Any other standard Csound type. Managed; not numerically accessible.
    Other(String),
}

impl ArrayType {
    /// Returns the Csound type name for this element type.
    pub fn as_str(&self) -> &str {
        match self {
            ArrayType::Init => "i",
            ArrayType::Control => "k",
            ArrayType::Audio => "a",
            ArrayType::Str => "S",
            ArrayType::Other(name) => name.as_str(),
        }
    }

    /// Returns true if elements are plain `MYFLT` data this crate can expose as
    /// a slice.
    ///
    /// False for `"S"` and other managed types, whose members are engine-owned
    /// structures rather than samples.
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            ArrayType::Init | ArrayType::Control | ArrayType::Audio
        )
    }

    /// Number of `MYFLT`s per element, given the engine's `ksmps`.
    ///
    /// Returns `None` for managed element types.
    fn myflts_per_element(&self, ksmps: u32) -> Option<usize> {
        match self {
            ArrayType::Init | ArrayType::Control => Some(1),
            ArrayType::Audio => Some(ksmps as usize),
            ArrayType::Str | ArrayType::Other(_) => None,
        }
    }
}

impl From<&str> for ArrayType {
    fn from(value: &str) -> Self {
        match value {
            "i" => ArrayType::Init,
            "k" => ArrayType::Control,
            "a" => ArrayType::Audio,
            "S" => ArrayType::Str,
            other => ArrayType::Other(other.to_owned()),
        }
    }
}

/// Metadata describing an array channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayChannelInfo {
    /// Element type reported by the engine.
    pub array_type: ArrayType,
    /// Number of dimensions.
    pub dimensions: usize,
    /// Size of each dimension.
    pub sizes: Vec<i32>,
    /// Product of the dimension sizes.
    pub element_count: usize,
    /// Total number of `MYFLT`s, or `None` for managed element types.
    pub len: Option<usize>,
}

/// Handle to an array channel.
///
/// The handle borrows the [`Csound`] instance it came from. Data access goes
/// through [`ArrayChannel::lock`] or [`ArrayChannel::with_lock`], which hold
/// Csound's channel lock for the duration of the access.
#[derive(Debug)]
pub struct ArrayChannel<'a> {
    channel: NamedChannel<'a>,
    adat: NonNull<ARRAYDAT>,
    /// `MYFLT`s per element, captured when the handle was created.
    ///
    /// `None` for managed element types. Captured rather than recomputed so a
    /// later `ksmps` change cannot silently alter the length used to size
    /// copies into engine memory.
    elem_myflts: Option<usize>,
}

impl<'a> ArrayChannel<'a> {
    fn from_raw(
        csound: &'a Csound,
        name: CString,
        adat: *mut ARRAYDAT,
        ksmps: u32,
    ) -> Option<Self> {
        let adat = NonNull::new(adat)?;
        let elem_myflts = array_type(adat.as_ptr()).myflts_per_element(ksmps);
        Some(ArrayChannel {
            channel: NamedChannel::new(csound, name),
            adat,
            elem_myflts,
        })
    }

    /// Returns the channel name as UTF-8.
    ///
    /// Channels created inside Csound may contain non-UTF-8 names; in that case
    /// this returns [`Error::UtfError`].
    #[inline]
    pub fn name(&self) -> Result<&str> {
        self.channel.name()
    }

    /// Locks the channel and returns a guard for safe access.
    ///
    /// This call blocks (spin-waits) until the channel lock is available.
    ///
    /// # Panics / Deadlock
    /// Do not call `lock()` re-entrantly on the same channel from the same
    /// thread. The Csound channel lock is non-recursive and will deadlock.
    #[inline]
    pub fn lock(&self) -> ArrayChannelLock<'_, 'a> {
        ArrayChannelLock::new(&self.channel, self.adat.as_ptr(), self.elem_myflts)
    }

    /// Locks the channel, runs the closure, and releases the lock.
    ///
    /// This holds the lock for exactly the duration of `f`, preventing
    /// references from escaping beyond the lock scope.
    ///
    /// # Panics / Deadlock
    /// Do not call `with_lock()` re-entrantly on the same channel from the same
    /// thread. The Csound channel lock is non-recursive and will deadlock.
    #[inline]
    pub fn with_lock<R>(&self, f: impl for<'lock> FnOnce(ArrayChannelLock<'lock, 'a>) -> R) -> R {
        f(self.lock())
    }

    /// Returns a snapshot of the channel's metadata (locks internally).
    #[inline]
    pub fn info(&self) -> ArrayChannelInfo {
        self.with_lock(|lock| lock.info())
    }

    /// Returns the element type of the array.
    #[inline]
    pub fn array_type(&self) -> ArrayType {
        self.with_lock(|lock| lock.array_type())
    }

    /// Reads the whole array into a newly allocated buffer (locks internally).
    ///
    /// # Errors
    /// - [`Error::InvalidArgument`] if the element type is managed
    /// - [`Error::BufferNotInitialized`] if the array has no data
    #[inline]
    pub fn read_all(&self) -> Result<Vec<Myflt>> {
        self.with_lock(|lock| lock.read_all())
    }

    /// Copies `input` into the array via `csoundSetArrayData` (locks
    /// internally).
    ///
    /// `input.len()` must equal the array's total length.
    ///
    /// # Errors
    /// See [`ArrayChannelLock::set_data`].
    #[inline]
    pub fn set_data(&self, input: &[Myflt]) -> Result<()> {
        self.with_lock(|lock| lock.set_data(input))
    }
}

// SAFETY: Array channel pointers are tied to the Csound instance lifetime and
// data access is synchronized through Csound's channel lock.
unsafe impl Send for ArrayChannel<'_> {}
// SAFETY: Shared access is safe because reads/writes are synchronized via
// Csound's channel lock.
unsafe impl Sync for ArrayChannel<'_> {}

/// Guard that holds an array channel's lock for safe data access.
#[must_use = "ArrayChannelLock unlocks on drop; keep it alive for the duration of channel access"]
#[derive(Debug)]
pub struct ArrayChannelLock<'lock, 'chan> {
    _lock: NamedChannelLock<'lock, 'chan>,
    adat: *mut ARRAYDAT,
    elem_myflts: Option<usize>,
}

impl<'lock, 'chan> ArrayChannelLock<'lock, 'chan> {
    fn new(
        channel: &'lock NamedChannel<'chan>,
        adat: *mut ARRAYDAT,
        elem_myflts: Option<usize>,
    ) -> Self {
        ArrayChannelLock {
            _lock: channel.lock(),
            adat,
            elem_myflts,
        }
    }

    /// Returns the element type of the array.
    #[inline]
    pub fn array_type(&self) -> ArrayType {
        array_type(self.adat)
    }

    /// Returns the number of dimensions.
    #[inline]
    pub fn dimensions(&self) -> usize {
        let dims = unsafe { csoundArrayDataDimensions(self.adat) };
        if dims <= 0 { 0 } else { dims as usize }
    }

    /// Returns the size of each dimension.
    pub fn sizes(&self) -> Vec<i32> {
        let dims = self.dimensions();
        if dims == 0 {
            return Vec::new();
        }
        let ptr = unsafe { csoundArrayDataSizes(self.adat) };
        if ptr.is_null() {
            return Vec::new();
        }
        unsafe { slice::from_raw_parts(ptr, dims) }.to_vec()
    }

    /// Returns the number of elements: the product of the dimension sizes.
    ///
    /// Returns 0 if the shape is degenerate or would overflow `usize`.
    pub fn element_count(&self) -> usize {
        let sizes = self.sizes();
        if sizes.is_empty() {
            return 0;
        }
        let mut count: usize = 1;
        for size in sizes {
            if size < 0 {
                return 0;
            }
            match count.checked_mul(size as usize) {
                Some(next) => count = next,
                None => return 0,
            }
        }
        count
    }

    /// Returns the total number of `MYFLT`s addressable through
    /// [`Self::as_slice`], or `None` for managed element types.
    pub fn len(&self) -> Option<usize> {
        let per_element = self.elem_myflts?;
        self.element_count().checked_mul(per_element)
    }

    /// Returns true if the array exposes no `MYFLT` data.
    ///
    /// Managed element types are always considered empty.
    pub fn is_empty(&self) -> bool {
        self.len().unwrap_or(0) == 0
    }

    /// Returns a snapshot of the channel's metadata.
    pub fn info(&self) -> ArrayChannelInfo {
        ArrayChannelInfo {
            array_type: self.array_type(),
            dimensions: self.dimensions(),
            sizes: self.sizes(),
            element_count: self.element_count(),
            len: self.len(),
        }
    }

    /// Returns the array data as a slice of `MYFLT`.
    ///
    /// # Errors
    /// - [`Error::InvalidArgument`] if the element type is managed, since those
    ///   members are engine-owned structures rather than samples
    /// - [`Error::BufferNotInitialized`] if the array has no backing storage
    pub fn as_slice(&self) -> Result<&[Myflt]> {
        let len = self.numeric_len()?;
        let ptr = unsafe { csoundGetArrayData(self.adat) } as *const Myflt;
        if ptr.is_null() {
            return Err(Error::BufferNotInitialized);
        }
        if len == 0 {
            return Ok(&[]);
        }
        // SAFETY: `len` is derived from the engine's own dimension sizes and
        // element type, matching the allocation `tabinit` made.
        Ok(unsafe { slice::from_raw_parts(ptr, len) })
    }

    /// Returns the array data as a mutable slice of `MYFLT`.
    ///
    /// Writing through this slice mutates engine memory directly, under the
    /// channel lock held by this guard.
    ///
    /// # Errors
    /// Same as [`Self::as_slice`].
    pub fn as_mut_slice(&mut self) -> Result<&mut [Myflt]> {
        let len = self.numeric_len()?;
        let ptr = unsafe { csoundGetArrayData(self.adat) } as *mut Myflt;
        if ptr.is_null() {
            return Err(Error::BufferNotInitialized);
        }
        if len == 0 {
            return Ok(&mut []);
        }
        // SAFETY: as in `as_slice`; `&mut self` guarantees exclusive access for
        // the lifetime of the returned slice, and the channel lock is held.
        Ok(unsafe { slice::from_raw_parts_mut(ptr, len) })
    }

    /// Copies the array into `output`, returning the number of elements copied.
    ///
    /// Copies `min(output.len(), len())` elements.
    ///
    /// # Errors
    /// Same as [`Self::as_slice`].
    pub fn read(&self, output: &mut [Myflt]) -> Result<usize> {
        let src = self.as_slice()?;
        let len = output.len().min(src.len());
        if len == 0 {
            return Ok(0);
        }
        output[..len].copy_from_slice(&src[..len]);
        Ok(len)
    }

    /// Reads the whole array into an owned buffer.
    ///
    /// # Errors
    /// Same as [`Self::as_slice`].
    pub fn read_all(&self) -> Result<Vec<Myflt>> {
        Ok(self.as_slice()?.to_vec())
    }

    /// Writes `input` into the array, returning the number of elements written.
    ///
    /// Copies `min(input.len(), len())` elements directly into engine memory.
    /// For a checked, whole-array copy through Csound's own validation, use
    /// [`Self::set_data`].
    ///
    /// # Errors
    /// Same as [`Self::as_slice`].
    pub fn write(&mut self, input: &[Myflt]) -> Result<usize> {
        let dst = self.as_mut_slice()?;
        let len = input.len().min(dst.len());
        if len == 0 {
            return Ok(0);
        }
        dst[..len].copy_from_slice(&input[..len]);
        Ok(len)
    }

    /// Copies `input` into the array using `csoundSetArrayData`.
    ///
    /// `input.len()` must be exactly [`Self::len`]. Csound copies
    /// `arrayMemberSize * element_count` bytes out of the supplied pointer and
    /// validates only its own destination, so a shorter slice would be read
    /// past its end; the length check makes that unrepresentable.
    ///
    /// # Errors
    /// - [`Error::InvalidArgument`] if the element type is managed
    /// - [`Error::BufferNotInitialized`] if the array has no backing storage
    /// - [`Error::InsufficientCapacity`] if `input.len()` is not exactly
    ///   [`Self::len`]
    /// - [`Error::OperationFailed`] if Csound rejected the copy
    pub fn set_data(&self, input: &[Myflt]) -> Result<()> {
        let len = self.numeric_len()?;

        if unsafe { csoundGetArrayData(self.adat) }.is_null() {
            return Err(Error::BufferNotInitialized);
        }

        if input.len() != len {
            return Err(Error::InsufficientCapacity {
                expected: len,
                actual: input.len(),
            });
        }

        if len == 0 {
            return Ok(());
        }

        // SAFETY: `input` holds exactly the `arrayMemberSize * element_count`
        // bytes Csound will read, as established above.
        let status =
            unsafe { csoundSetArrayData(self.adat, input.as_ptr() as *const c_void) } as c_int;

        match Status::from(status) {
            Status::Success => Ok(()),
            Status::Memory => Err(Error::Memory),
            _ => Err(Error::OperationFailed),
        }
    }

    /// Returns the numeric length, rejecting managed element types.
    fn numeric_len(&self) -> Result<usize> {
        if self.elem_myflts.is_none() {
            return Err(Error::InvalidArgument(
                "array channel has a managed element type; numeric access is not supported",
            ));
        }
        self.len().ok_or(Error::InvalidArgument(
            "array channel length overflows usize",
        ))
    }
}

impl Csound {
    /// Creates and initializes an array channel, returning a handle.
    ///
    /// `array_type` is a Csound element type name (`"i"`, `"k"`, `"a"`, `"S"`,
    /// or another standard type). `sizes` gives the size of each dimension; its
    /// length is the number of dimensions.
    ///
    /// If the channel already exists *and has been initialized*, Csound treats
    /// this as a no-op and returns the existing array unchanged — including
    /// when the existing shape differs from the one requested. Use
    /// [`ArrayChannel::info`] to confirm the shape you got.
    ///
    /// For `"a"` arrays the element size is `ksmps`, which is only known once
    /// the orchestra has been compiled, so create those after
    /// [`Csound::compile_orc`] / [`Csound::compile_csd`].
    ///
    /// # Errors
    /// - [`Error::EmptyString`] if `name` or `array_type` is empty
    /// - [`Error::Nul`] if `name` or `array_type` contains an interior NUL byte
    /// - [`Error::InvalidArgument`] if `sizes` is empty, contains a negative
    ///   value, or has more entries than `i32` can express
    /// - [`Error::NullPointer`] if Csound could not initialize the channel
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use csound::Csound;
    /// let cs = Csound::new().unwrap();
    /// cs.compile_orc("instr 1\nendin\n", 0).unwrap();
    /// cs.start().unwrap();
    ///
    /// let chan = cs.init_array_channel("myarray", "k", &[4]).unwrap();
    /// chan.set_data(&[1.0, 2.0, 3.0, 4.0]).unwrap();
    /// assert_eq!(chan.read_all().unwrap(), vec![1.0, 2.0, 3.0, 4.0]);
    /// ```
    pub fn init_array_channel(
        &self,
        name: &str,
        array_type: &str,
        sizes: &[i32],
    ) -> Result<ArrayChannel<'_>> {
        if name.is_empty() {
            return Err(Error::EmptyString);
        }
        if array_type.is_empty() {
            return Err(Error::EmptyString);
        }
        if sizes.is_empty() {
            return Err(Error::InvalidArgument(
                "array channel needs at least one dimension",
            ));
        }
        if sizes.iter().any(|&size| size < 0) {
            return Err(Error::InvalidArgument(
                "array channel dimension sizes must be non-negative",
            ));
        }
        let dimensions = i32::try_from(sizes.len())
            .map_err(|_| Error::InvalidArgument("too many array dimensions"))?;

        let cname = CString::new(name)?;
        let ctype = CString::new(array_type)?;

        let adat = unsafe {
            csoundInitArrayChannel(
                self.csound_ptr(),
                cname.as_ptr(),
                ctype.as_ptr(),
                dimensions,
                sizes.as_ptr(),
            )
        };

        if adat.is_null() {
            return Err(Error::NullPointer("failed to initialize array channel"));
        }
        if array_data_ptr(adat).is_null() {
            return Err(Error::BufferNotInitialized);
        }

        ArrayChannel::from_raw(self, cname, adat, self.get_ksmps())
            .ok_or(Error::NullPointer("failed to initialize array channel"))
    }

    /// Returns a handle to an existing array channel, creating the channel
    /// entry if it does not exist.
    ///
    /// Unlike [`Csound::init_array_channel`] this does not allocate the array
    /// storage. A channel that exists but has never been initialized — by the
    /// orchestra or by `init_array_channel` — has no data, and numeric access
    /// through the returned handle will fail with
    /// [`Error::BufferNotInitialized`].
    ///
    /// # Errors
    /// - [`Error::EmptyString`] if `name` is empty
    /// - [`Error::Nul`] if `name` contains an interior NUL byte
    /// - [`Error::Memory`] if channel allocation failed
    /// - [`Error::InvalidArgument`] if the name or type is invalid
    /// - [`Error::ChannelTypeMismatch`] if an incompatible channel exists
    /// - [`Error::NullPointer`] if Csound returned a null array
    pub fn get_array_channel(&self, name: &str) -> Result<ArrayChannel<'_>> {
        if name.is_empty() {
            return Err(Error::EmptyString);
        }

        let mut ptr: *mut c_void = std::ptr::null_mut();
        let ptr_ref = &mut ptr as *mut *mut c_void;
        let bits = (controlChannelType::CSOUND_ARRAY_CHANNEL
            | controlChannelType::CSOUND_INPUT_CHANNEL
            | controlChannelType::CSOUND_OUTPUT_CHANNEL) as c_int;

        let cname = CString::new(name)?;
        let status = self.get_raw_channel_ptr(&cname, ptr_ref, bits);

        match Status::from(status) {
            Status::Success => {
                let adat = ptr as *mut ARRAYDAT;
                if adat.is_null() {
                    return Err(Error::NullPointer("failed to create array channel"));
                }
                // A channel entry can exist without storage; reading its type in
                // that state dereferences a NULL `arrayType` inside Csound.
                if array_data_ptr(adat).is_null() {
                    return Err(Error::BufferNotInitialized);
                }

                ArrayChannel::from_raw(self, cname, adat, self.get_ksmps())
                    .ok_or(Error::NullPointer("failed to create array channel"))
            }
            Status::Memory => Err(Error::Memory),
            Status::Error => Err(Error::InvalidArgument("invalid channel name or type")),
            Status::Ok(existing_type) => Err(Error::ChannelTypeMismatch(existing_type)),
            _ => Err(Error::OperationFailed),
        }
    }
}

/// Returns the array's data pointer, or null if it has no storage.
fn array_data_ptr(adat: *mut ARRAYDAT) -> *const c_void {
    unsafe { csoundGetArrayData(adat) }
}

/// Reads the element type name from an `ARRAYDAT`.
///
/// # Safety of the storage check
///
/// `csoundArrayDataType()` dereferences `adat->arrayType` without a null check,
/// and that field is NULL for a channel that exists but has never been
/// initialized — `csoundGetChannelPtr()` creates the entry without allocating
/// the array. Calling it in that state segfaults inside Csound, so the data
/// pointer is checked first: storage is only ever allocated after `arrayType`
/// has been set, making a non-null `data` a sound proxy for a valid type.
///
/// Falls back to [`ArrayType::Other`] with an empty name when the type cannot
/// be read, which keeps callers on the managed (non-numeric) path rather than
/// guessing a layout.
fn array_type(adat: *mut ARRAYDAT) -> ArrayType {
    if array_data_ptr(adat).is_null() {
        return ArrayType::Other(String::new());
    }
    let ptr = unsafe { csoundArrayDataType(adat) };
    if ptr.is_null() {
        return ArrayType::Other(String::new());
    }
    match unsafe { CStr::from_ptr(ptr) }.to_str() {
        Ok(name) => ArrayType::from(name),
        Err(_) => ArrayType::Other(String::new()),
    }
}
