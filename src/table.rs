use core::slice;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A Csound function table identifier.
///
/// Table IDs are user-defined integers that identify function tables in Csound.
/// They are specified in score `f` statements (e.g., `f 1 0 1024 10 1`) or
/// orchestra opcodes like `ftgen`.
///
/// A value of `0` in `ftgen` means "auto-assign a table number".
pub type TableId = u32;

/// Csound function table representation.
///
/// This struct provides direct access to a Csound function table's data.
/// The table data can be accessed as a slice via [`Deref`]/[`DerefMut`] or
/// the provided methods.
///
/// # Guard Point
///
/// The length does **not** include Csound's internal guard point. See
/// [`Csound::table_length`](crate::Csound::table_length) for details on guard points.
#[derive(Debug)]
pub struct Table<'a> {
    pub(crate) ptr: *mut f64,
    pub(crate) length: usize,
    pub(crate) phantom: PhantomData<&'a f64>,
}

impl<'a> Table<'a> {
    /// Returns the table length (excluding the guard point).
    ///
    /// # Returns
    /// The number of usable data points in the table.
    pub fn get_size(&self) -> usize {
        self.length
    }

    /// # Returns
    /// A slice representation with the table's internal data
    pub fn as_slice(&self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.ptr, self.length) }
    }

    /// # Returns
    /// A mutable slice representation with the table's internal data
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.length) }
    }

    /// method used to copy data from the table internal buffer
    /// into an user buffer. A error message is returned if the Table is not longer valid.
    /// # Arguments
    /// * `slice` A slice where out.len() elements from the table will be copied.
    /// # Returns
    /// The number of elements copied into the output slice.
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_csd("some.csd", 0, 0);
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table = cs.get_table(1).unwrap();
    ///     let mut table_buff = vec![0f64; table.len()];
    ///     // copy Table::length elements from the table's internal buffer
    ///     table.copy_to_slice( table_buff.as_mut_slice() );
    ///     // Do some stuffs
    /// }
    /// ```
    pub fn copy_to_slice(&self, slice: &mut [f64]) -> usize {
        let mut len = slice.len();
        let size = self.get_size();
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(self.ptr, slice.as_mut_ptr(), len);
            len
        }
    }

    /// method used to copy data into the table internal buffer
    /// from an user slice.
    /// # Arguments
    /// * `slice` A slice where input.len() elements will be copied.
    /// # Returns
    /// The number of elements copied into the table
    /// # Example
    /// ```ignore
    /// use csound::Csound;
    ///
    /// let cs = Csound::new().unwrap();
    /// cs.compile_csd("some.csd", 0, 0);
    /// cs.start().unwrap();
    /// while cs.perform_ksmps() == false {
    ///     let mut table = cs.get_table(1).unwrap();
    ///     let mut table_buff = vec![0f64; table.len()];
    ///     // copy Table::length elements from the table's internal buffer
    ///     // table.read( table_buff.as_mut_slice() ).unwrap();
    ///     // Do some stuffs
    ///     table.copy_from_slice(&table_buff.into_iter().map(|x| x*2.5).collect::<Vec<f64>>().as_mut_slice());
    ///     // Do some stuffs
    /// }
    /// ```
    pub fn copy_from_slice(&self, slice: &[f64]) -> usize {
        let mut len = slice.len();
        let size = self.get_size();
        if size < len {
            len = size;
        }
        unsafe {
            std::ptr::copy(slice.as_ptr(), self.ptr, len);
            len
        }
    }
}

impl<'a> AsRef<[f64]> for Table<'a> {
    fn as_ref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> AsMut<[f64]> for Table<'a> {
    fn as_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}

impl<'a> Deref for Table<'a> {
    type Target = [f64];
    fn deref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> DerefMut for Table<'a> {
    fn deref_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}
