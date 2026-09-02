//! Csound debugger support.
//!
//! Wraps `csdebug.h`: breakpoints, execution control, and inspection of live
//! instrument instances, their variables, active UDO frames and orchestra
//! globals.
//!
//! # Shape of the API
//!
//! [`Csound::debugger`] initialises the debugger and returns a [`Debugger`]
//! guard that cleans up on drop. Everything else hangs off that guard, so the
//! debugger cannot be used without having been initialised:
//!
//! ```no_run
//! # use csound::Csound;
//! let cs = Csound::new().unwrap();
//! cs.compile_orc("instr 1\nendin\n", 0).unwrap();
//!
//! let mut dbg = cs.debugger().unwrap();      // before start()
//! dbg.set_instrument_breakpoint(1.0, 0);
//! dbg.on_breakpoint(|bkpt| {
//!     println!("stopped in instr {:?}", bkpt.instrument().map(|i| i.p1()));
//! });
//! ```
//!
//! # Ownership of the C lists
//!
//! The C API returns linked lists that the caller must free with a matching
//! `csoundDebugFree*` call. Each is wrapped in a guard that frees on drop, so
//! the lists cannot leak and cannot outlive the debugger.
//!
//! The lists reachable from a breakpoint callback are the exception: those are
//! owned by the engine and must *not* be freed. They are handed out as borrows
//! that skip the free, which is why [`BreakpointInfo`] is only reachable inside
//! the callback.
//!
//! # Thread safety
//!
//! `csoundDebuggerInit` is not thread safe and must run before performance
//! starts. Reading instances and variables is only valid while the engine is
//! stopped at a breakpoint or from inside the k-cycle callback. Setting and
//! removing breakpoints is thread safe: those go through a lock-free queue that
//! the performance loop drains.

use std::ffi::CStr;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use libc::{c_int, c_void};

use crate::Csound;
use crate::Myflt;
use crate::error::{CsoundErrorCode, Error, IntegerTarget, Result};

use csound_sys::ffi_bindgen::{
    CSOUND_STATUS, debug_array_info_t, debug_bkpt_info_t, debug_fsig_info_t, debug_instr_t,
    debug_udo_frame_t, debug_variable_t,
};
use csound_sys::ffi_bindgen::{
    csoundClearBreakpoints, csoundDebugContinue, csoundDebugFreeInstrInstances,
    csoundDebugFreeUdoFrames, csoundDebugFreeVariables, csoundDebugGetGlobalVariables,
    csoundDebugGetInstrInstances, csoundDebugGetUdoFrames, csoundDebugGetVariables,
    csoundDebugNext, csoundDebugSerializeArray, csoundDebugSerializeFsig, csoundDebugStop,
    csoundDebuggerClean, csoundDebuggerInit, csoundGetStringData, csoundRemoveBreakpoint,
    csoundRemoveDebugCallback, csoundRemoveInstrumentBreakpoint, csoundSetBreakpoint,
    csoundSetBreakpointCallback, csoundSetDebugCallback, csoundSetInstrumentBreakpoint,
};

type BreakpointCallback<'a> = Box<dyn for<'b> FnMut(&BreakpointInfo<'b>) + 'a>;
type KCycleCallback<'a> = Box<dyn FnMut() + 'a>;

/// An initialised Csound debugger.
///
/// Created by [`Csound::debugger`]. Dropping it removes any callbacks that were
/// installed and calls `csoundDebuggerClean`, so the engine never retains a
/// pointer to a closure this guard owned.
pub struct Debugger<'a> {
    csound: *mut csound_sys::CSOUND,
    /// Boxed so the address handed to Csound as `userdata` stays stable.
    breakpoint_cb: Option<Box<BreakpointCallback<'a>>>,
    kcycle_cb: Option<Box<KCycleCallback<'a>>>,
    phantom: PhantomData<&'a Csound>,
}

impl<'a> Debugger<'a> {
    // -- breakpoints ------------------------------------------------------

    /// Sets a breakpoint on a source line.
    ///
    /// `instr` of 0 makes `line` refer to a line in the score; otherwise it
    /// refers to a line within that instrument. `skip` is the number of control
    /// blocks to skip before breaking again.
    pub fn set_line_breakpoint(&mut self, line: i32, instr: i32, skip: i32) {
        unsafe { csoundSetBreakpoint(self.csound, line, instr, skip) }
    }

    /// Removes a line breakpoint previously set by [`Self::set_line_breakpoint`].
    pub fn remove_line_breakpoint(&mut self, line: i32, instr: i32) {
        unsafe { csoundRemoveBreakpoint(self.csound, line, instr) }
    }

    /// Sets a breakpoint on an instrument.
    ///
    /// A fractional instrument number targets a particular instance. `skip` is
    /// the number of control blocks to skip before breaking again; 0 and 1 both
    /// mean "break every time".
    ///
    /// Thread safe: the breakpoint is queued and picked up by the performance
    /// loop.
    pub fn set_instrument_breakpoint(&mut self, instr: Myflt, skip: i32) {
        unsafe { csoundSetInstrumentBreakpoint(self.csound, instr, skip) }
    }

    /// Removes an instrument breakpoint. Thread safe, as above.
    pub fn remove_instrument_breakpoint(&mut self, instr: Myflt) {
        unsafe { csoundRemoveInstrumentBreakpoint(self.csound, instr) }
    }

    /// Removes every breakpoint. Thread safe, as above.
    pub fn clear_breakpoints(&mut self) {
        unsafe { csoundClearBreakpoints(self.csound) }
    }

    // -- execution control ------------------------------------------------

    /// Continues execution, stopping again at the next instrument instance.
    pub fn next(&mut self) {
        unsafe { csoundDebugNext(self.csound) }
    }

    /// Continues execution from a breakpoint.
    pub fn continue_(&mut self) {
        unsafe { csoundDebugContinue(self.csound) }
    }

    /// Stops rendering and enters the debugger at the soonest opportunity, as
    /// though a breakpoint had been reached.
    pub fn stop(&mut self) {
        unsafe { csoundDebugStop(self.csound) }
    }

    // -- callbacks --------------------------------------------------------

    /// Sets the callback invoked when a breakpoint is reached.
    ///
    /// The lists reachable from the [`BreakpointInfo`] are owned by the engine
    /// and borrowed for the duration of the call, so they cannot escape it.
    ///
    /// A panic inside the callback is caught and logged rather than unwound
    /// across the FFI boundary, which would be undefined behaviour.
    pub fn on_breakpoint<F>(&mut self, callback: F)
    where
        F: for<'b> FnMut(&BreakpointInfo<'b>) + 'a,
    {
        let boxed: Box<BreakpointCallback<'a>> = Box::new(Box::new(callback));
        let userdata = &*boxed as *const BreakpointCallback<'a> as *mut c_void;
        // Point Csound at the new closure *before* storing it, because the
        // store drops any previous one. Doing it the other way round leaves
        // Csound holding a freed pointer for the window in between.
        unsafe {
            csoundSetBreakpointCallback(self.csound, Some(breakpoint_trampoline), userdata);
        }
        self.breakpoint_cb = Some(boxed);
    }

    /// Sets a callback invoked after every k-cycle while the debugger is active.
    ///
    /// Unlike [`Self::on_breakpoint`] this does not stop the engine: performance
    /// continues as soon as the callback returns, so it must be quick. It fires
    /// from inside the performance loop, after all instruments have run and
    /// before audio is sent.
    ///
    /// A panic inside the callback is caught and logged rather than unwound
    /// across the FFI boundary.
    pub fn on_k_cycle<F>(&mut self, callback: F)
    where
        F: FnMut() + 'a,
    {
        let boxed: Box<KCycleCallback<'a>> = Box::new(Box::new(callback));
        let userdata = &*boxed as *const KCycleCallback<'a> as *mut c_void;
        // Install before storing; see `on_breakpoint`.
        unsafe {
            csoundSetDebugCallback(self.csound, Some(kcycle_trampoline), userdata);
        }
        self.kcycle_cb = Some(boxed);
    }

    /// Removes a callback previously installed by [`Self::on_k_cycle`].
    pub fn remove_k_cycle_callback(&mut self) {
        // Detach in the engine first, then free the closure.
        unsafe { csoundRemoveDebugCallback(self.csound) }
        self.kcycle_cb = None;
    }

    // -- inspection -------------------------------------------------------

    /// Returns the list of active instrument instances.
    ///
    /// Only valid while the engine is stopped at a breakpoint or from inside
    /// the k-cycle callback; this is not thread safe against a running
    /// performance.
    pub fn instr_instances(&self) -> InstrInstances<'_> {
        let head = unsafe { csoundDebugGetInstrInstances(self.csound) };
        InstrInstances {
            csound: self.csound,
            head,
            phantom: PhantomData,
        }
    }

    /// Returns the orchestra-wide global variables (`gk*`, `ga*`, `gi*`, `gS*`,
    /// `gf*`, global arrays, and Csound's own globals such as `sr` and `ksmps`).
    ///
    /// Returns an empty list before compilation, when the global pool does not
    /// yet exist.
    pub fn global_variables(&self) -> Variables<'_> {
        let head = unsafe { csoundDebugGetGlobalVariables(self.csound) };
        Variables {
            csound: self.csound,
            head,
            owned: true,
            phantom: PhantomData,
        }
    }
}

impl Drop for Debugger<'_> {
    fn drop(&mut self) {
        unsafe {
            // Clear the callbacks before the boxed closures are freed, so the
            // engine cannot call into memory this guard is about to release.
            csoundRemoveDebugCallback(self.csound);
            csoundSetBreakpointCallback(self.csound, None, ptr::null_mut());
            csoundDebuggerClean(self.csound);
        }
    }
}

// -- trampolines ----------------------------------------------------------

/// Runs `f`, swallowing any panic so it cannot unwind into C.
fn guard<F: FnOnce()>(what: &'static str, f: F) {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        tracing::error!(
            callback = what,
            "debugger callback panicked; panic contained"
        );
    }
}

extern "C" fn breakpoint_trampoline(
    csound: *mut csound_sys::CSOUND,
    info: *mut debug_bkpt_info_t,
    userdata: *mut c_void,
) {
    if userdata.is_null() {
        return;
    }
    guard("on_breakpoint", || {
        // SAFETY: `userdata` is the address of the boxed callback owned by the
        // Debugger, which clears this callback before dropping it.
        let callback = unsafe { &mut *(userdata as *mut BreakpointCallback<'_>) };
        let bkpt = BreakpointInfo {
            csound,
            info,
            phantom: PhantomData,
        };
        callback(&bkpt);
    });
}

extern "C" fn kcycle_trampoline(_csound: *mut csound_sys::CSOUND, userdata: *mut c_void) {
    if userdata.is_null() {
        return;
    }
    guard("on_k_cycle", || {
        // SAFETY: as in `breakpoint_trampoline`.
        let callback = unsafe { &mut *(userdata as *mut KCycleCallback<'_>) };
        callback();
    });
}

// -- breakpoint info ------------------------------------------------------

/// State handed to a breakpoint callback.
///
/// Everything reachable here is owned by the engine and borrowed for the
/// duration of the callback, so nothing is freed when this is dropped.
pub struct BreakpointInfo<'a> {
    csound: *mut csound_sys::CSOUND,
    info: *mut debug_bkpt_info_t,
    phantom: PhantomData<&'a ()>,
}

impl BreakpointInfo<'_> {
    /// The instrument instance the engine stopped in, if any.
    pub fn instrument(&self) -> Option<InstrInstance<'_>> {
        let info = self.as_ref()?;
        let ptr = info.breakpointInstr;
        (!ptr.is_null()).then_some(InstrInstance {
            csound: self.csound,
            ptr,
            phantom: PhantomData,
        })
    }

    /// Variables of the instrument that was stopped in.
    ///
    /// Borrowed from the engine; not freed on drop.
    pub fn variables(&self) -> Variables<'_> {
        let head = self.as_ref().map_or(ptr::null_mut(), |i| i.instrVarList);
        Variables {
            csound: self.csound,
            head,
            owned: false,
            phantom: PhantomData,
        }
    }

    /// The name of the opcode the engine stopped on, when it stopped on a line
    /// breakpoint.
    pub fn opcode_name(&self) -> Option<String> {
        let info = self.as_ref()?;
        let opcode = info.currentOpcode;
        if opcode.is_null() {
            return None;
        }
        // SAFETY: non-null opcode from the engine; `opname` is a fixed NUL
        // terminated buffer.
        let name = unsafe { CStr::from_ptr((*opcode).opname.as_ptr()) };
        name.to_str().ok().map(str::to_owned)
    }

    /// Resumes execution from inside the callback.
    ///
    /// Csound stops the performance loop when a breakpoint is hit, so the
    /// engine stays stopped until something continues it. Resuming from within
    /// the callback is the usual single-threaded pattern; the alternative is to
    /// call [`Debugger::continue_`] from another thread or from the loop
    /// driving [`Csound::perform_ksmps`].
    ///
    /// These are on the callback's info rather than on [`Debugger`] because the
    /// debugger owns the closure being run, so it cannot also be borrowed
    /// mutably here.
    pub fn continue_(&self) {
        unsafe { csoundDebugContinue(self.csound) }
    }

    /// Continues from inside the callback, stopping at the next instrument
    /// instance.
    pub fn next(&self) {
        unsafe { csoundDebugNext(self.csound) }
    }

    /// Re-enters the debugger at the soonest opportunity.
    pub fn stop(&self) {
        unsafe { csoundDebugStop(self.csound) }
    }

    /// Removes an instrument breakpoint from inside the callback.
    ///
    /// Typical use is a one-shot breakpoint that removes itself when hit.
    pub fn remove_instrument_breakpoint(&self, instr: Myflt) {
        unsafe { csoundRemoveInstrumentBreakpoint(self.csound, instr) }
    }

    fn as_ref(&self) -> Option<&debug_bkpt_info_t> {
        // SAFETY: valid for the duration of the callback.
        (!self.info.is_null()).then(|| unsafe { &*self.info })
    }
}

// -- instrument instances -------------------------------------------------

/// A list of active instrument instances, freed on drop.
pub struct InstrInstances<'a> {
    csound: *mut csound_sys::CSOUND,
    head: *mut debug_instr_t,
    phantom: PhantomData<&'a Debugger<'a>>,
}

impl InstrInstances<'_> {
    /// Iterates the instances.
    pub fn iter(&self) -> impl Iterator<Item = InstrInstance<'_>> {
        LinkedList {
            current: self.head,
            next: |p: *mut debug_instr_t| unsafe { (*p).next },
        }
        .map(|ptr| InstrInstance {
            csound: self.csound,
            ptr,
            phantom: PhantomData,
        })
    }

    /// Returns true when there are no active instances.
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Number of active instances.
    pub fn len(&self) -> usize {
        self.iter().count()
    }
}

impl Drop for InstrInstances<'_> {
    fn drop(&mut self) {
        if !self.head.is_null() {
            unsafe { csoundDebugFreeInstrInstances(self.csound, self.head) }
        }
    }
}

/// One active instrument instance.
pub struct InstrInstance<'a> {
    csound: *mut csound_sys::CSOUND,
    ptr: *mut debug_instr_t,
    phantom: PhantomData<&'a ()>,
}

impl InstrInstance<'_> {
    fn as_ref(&self) -> &debug_instr_t {
        // SAFETY: non-null for the lifetime of the owning list.
        unsafe { &*self.ptr }
    }

    /// p1 — the instrument number, including any fractional instance part.
    pub fn p1(&self) -> Myflt {
        self.as_ref().p1
    }

    /// p2 — the instance's start time.
    pub fn p2(&self) -> Myflt {
        self.as_ref().p2
    }

    /// p3 — the instance's duration.
    pub fn p3(&self) -> Myflt {
        self.as_ref().p3
    }

    /// The control-cycle count at which this instance was observed.
    pub fn kcounter(&self) -> u64 {
        self.as_ref().kcounter
    }

    /// The source line currently being executed.
    pub fn line(&self) -> i32 {
        self.as_ref().line
    }

    /// The instance's local variables, freed on drop.
    pub fn variables(&self) -> Variables<'_> {
        let head = unsafe { csoundDebugGetVariables(self.csound, self.ptr) };
        Variables {
            csound: self.csound,
            head,
            owned: true,
            phantom: PhantomData,
        }
    }

    /// Active UDO frames beneath this instance, freed on drop.
    ///
    /// Includes nested and recursive frames, at increasing `depth`.
    pub fn udo_frames(&self) -> UdoFrames<'_> {
        let mut truncated: i32 = 0;
        let head =
            unsafe { csoundDebugGetUdoFrames(self.csound, self.ptr, &mut truncated as *mut i32) };
        UdoFrames {
            csound: self.csound,
            head,
            truncated: truncated != 0,
            phantom: PhantomData,
        }
    }
}

// -- UDO frames -----------------------------------------------------------

/// Active UDO invocation frames for one instrument instance, freed on drop.
pub struct UdoFrames<'a> {
    csound: *mut csound_sys::CSOUND,
    head: *mut debug_udo_frame_t,
    truncated: bool,
    phantom: PhantomData<&'a ()>,
}

impl UdoFrames<'_> {
    /// True when the walk could not enumerate every active frame, in which case
    /// the list is incomplete.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Returns true when no UDO frames are active.
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Number of active frames.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Iterates the frames.
    pub fn iter(&self) -> impl Iterator<Item = UdoFrame<'_>> {
        LinkedList {
            current: self.head,
            next: |p: *mut debug_udo_frame_t| unsafe { (*p).next },
        }
        .map(|ptr| UdoFrame {
            csound: self.csound,
            ptr,
            phantom: PhantomData,
        })
    }
}

impl Drop for UdoFrames<'_> {
    fn drop(&mut self) {
        if !self.head.is_null() {
            unsafe { csoundDebugFreeUdoFrames(self.csound, self.head) }
        }
    }
}

/// One active UDO invocation frame.
pub struct UdoFrame<'a> {
    csound: *mut csound_sys::CSOUND,
    ptr: *mut debug_udo_frame_t,
    phantom: PhantomData<&'a ()>,
}

impl UdoFrame<'_> {
    fn as_ref(&self) -> &debug_udo_frame_t {
        // SAFETY: non-null for the lifetime of the owning list.
        unsafe { &*self.ptr }
    }

    /// The UDO's name.
    pub fn name(&self) -> Option<&str> {
        cstr_opt(self.as_ref().udoName)
    }

    /// Source line of the call site.
    pub fn call_line(&self) -> i32 {
        self.as_ref().callLine
    }

    /// Nesting depth; deeper values are nested or recursive calls.
    pub fn depth(&self) -> i32 {
        self.as_ref().depth
    }

    /// Ordering among sibling calls on the same parent, 0 being the most recent.
    pub fn frame_index(&self) -> i32 {
        self.as_ref().frameIndex
    }

    /// Variables belonging to this UDO body.
    ///
    /// Borrowed from the frame list, so it is not freed separately.
    pub fn variables(&self) -> Variables<'_> {
        Variables {
            csound: self.csound,
            head: self.as_ref().varList,
            owned: false,
            phantom: PhantomData,
        }
    }
}

// -- variables ------------------------------------------------------------

/// A list of debugger variables.
///
/// Frees the underlying list on drop when it owns it. Lists reached through a
/// breakpoint callback or a UDO frame are owned by the engine and are not
/// freed here.
pub struct Variables<'a> {
    csound: *mut csound_sys::CSOUND,
    head: *mut debug_variable_t,
    owned: bool,
    phantom: PhantomData<&'a ()>,
}

impl Variables<'_> {
    /// Returns true when the list is empty.
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Number of variables.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Iterates the variables.
    pub fn iter(&self) -> impl Iterator<Item = DebugVariable<'_>> {
        LinkedList {
            current: self.head,
            next: |p: *mut debug_variable_t| unsafe { (*p).next },
        }
        .map(|ptr| DebugVariable {
            csound: self.csound,
            ptr,
            phantom: PhantomData,
        })
    }

    /// Finds a variable by name.
    pub fn get(&self, name: &str) -> Option<DebugVariable<'_>> {
        self.iter().find(|v| v.name() == Some(name))
    }
}

impl Drop for Variables<'_> {
    fn drop(&mut self) {
        if self.owned && !self.head.is_null() {
            unsafe { csoundDebugFreeVariables(self.csound, self.head) }
        }
    }
}

/// One variable visible to the debugger.
pub struct DebugVariable<'a> {
    csound: *mut csound_sys::CSOUND,
    ptr: *mut debug_variable_t,
    phantom: PhantomData<&'a ()>,
}

impl DebugVariable<'_> {
    fn as_ref(&self) -> &debug_variable_t {
        // SAFETY: non-null for the lifetime of the owning list.
        unsafe { &*self.ptr }
    }

    /// The variable's name as written in the orchestra.
    pub fn name(&self) -> Option<&str> {
        cstr_opt(self.as_ref().name)
    }

    /// Csound's type name: `"i"`, `"k"`, `"a"`, `"S"`, `"f"`, `"["`, ...
    pub fn type_name(&self) -> Option<&str> {
        cstr_opt(self.as_ref().typeName)
    }

    /// True when the variable has no storage to read.
    pub fn is_null(&self) -> bool {
        self.as_ref().data.is_null()
    }

    /// Reads a scalar `i`- or `k`-rate value.
    ///
    /// # Errors
    /// [`Error::TypeMismatch`] if the variable is not scalar, or
    /// [`Error::NullPointer`] if it has no storage.
    pub fn scalar(&self) -> Result<Myflt> {
        match self.type_name() {
            Some("i") | Some("k") => {}
            _ => {
                return Err(Error::TypeMismatch {
                    context: "debug variable",
                    expected: "i- or k-rate scalar",
                    actual: self.type_name().unwrap_or("unknown").to_owned(),
                });
            }
        }
        let data = self.data_ptr()?;
        // SAFETY: checked non-null, and an i/k variable is one MYFLT.
        Ok(unsafe { *(data as *const Myflt) })
    }

    /// Reads an `a`-rate audio vector.
    ///
    /// `ksmps` is the number of samples to read. Pass the engine's
    /// [`Csound::get_ksmps`] for a top-level instrument; a UDO running at a
    /// local ksmps needs that local value instead, since the wrapper cannot
    /// infer it.
    ///
    /// # Errors
    /// [`Error::TypeMismatch`] if the variable is not audio rate, or
    /// [`Error::NullPointer`] if it has no storage.
    pub fn audio(&self, ksmps: u32) -> Result<Vec<Myflt>> {
        if self.type_name() != Some("a") {
            return Err(Error::TypeMismatch {
                context: "debug variable",
                expected: "a-rate audio",
                actual: self.type_name().unwrap_or("unknown").to_owned(),
            });
        }
        let data = self.data_ptr()?;
        if ksmps == 0 {
            return Ok(Vec::new());
        }
        // SAFETY: checked non-null; the caller states the vector length, which
        // matches the producer's ksmps.
        Ok(unsafe { std::slice::from_raw_parts(data as *const Myflt, ksmps as usize) }.to_vec())
    }

    /// Reads an `S`-rate string value.
    ///
    /// # Errors
    /// [`Error::TypeMismatch`] if the variable is not a string,
    /// [`Error::NullPointer`] if it has no storage, or [`Error::UtfError`] if
    /// the contents are not UTF-8.
    pub fn string(&self) -> Result<String> {
        if self.type_name() != Some("S") {
            return Err(Error::TypeMismatch {
                context: "debug variable",
                expected: "S-rate string",
                actual: self.type_name().unwrap_or("unknown").to_owned(),
            });
        }
        let data = self.data_ptr()?;
        // SAFETY: an "S" variable's data is a STRINGDAT.
        let raw = unsafe { csoundGetStringData(self.csound, data as *mut _) };
        if raw.is_null() {
            return Ok(String::new());
        }
        unsafe { CStr::from_ptr(raw) }
            .to_str()
            .map(str::to_owned)
            .map_err(Error::from)
    }

    /// Reads an `f`-signal (PVS) analysis frame.
    ///
    /// Returns the interleaved `(amp, freq)` pairs of the current frame, along
    /// with its metadata.
    ///
    /// `local_ksmps` is the producing instrument or UDO's current local ksmps.
    /// Pass 0 only when the producer runs at the engine-global ksmps.
    ///
    /// An empty buffer is returned when the frame has not been allocated yet,
    /// which is normal before the first analysis pass.
    ///
    /// # Errors
    /// [`Error::TypeMismatch`] if the variable is not an f-signal, or
    /// [`Error::NullPointer`] if it has no storage.
    pub fn fsig(&self, local_ksmps: u32) -> Result<(FsigInfo, Vec<f32>)> {
        if self.type_name() != Some("f") {
            return Err(Error::TypeMismatch {
                context: "debug variable",
                expected: "f-signal",
                actual: self.type_name().unwrap_or("unknown").to_owned(),
            });
        }
        let data = self.data_ptr()?;
        let local_ksmps = i32::try_from(local_ksmps).map_err(|_| Error::IntegerOutOfRange {
            argument: "local_ksmps",
            value: u128::from(local_ksmps),
            target: IntegerTarget::I32,
        })?;

        let mut info = debug_fsig_info_t::default();

        // Ask for the size first: a null buffer with a zero maximum reports the
        // total without copying, so the buffer is never undersized.
        let total = unsafe {
            csoundDebugSerializeFsig(
                self.csound,
                data,
                ptr::null_mut(),
                0,
                &mut info as *mut _,
                local_ksmps,
            )
        };
        if total <= 0 {
            return Ok((FsigInfo::from(&info), Vec::new()));
        }

        let mut buf = vec![0.0f32; total as usize];
        unsafe {
            csoundDebugSerializeFsig(
                self.csound,
                data,
                buf.as_mut_ptr(),
                total,
                &mut info as *mut _,
                local_ksmps,
            );
        }
        Ok((FsigInfo::from(&info), buf))
    }

    /// Reads a numeric array variable.
    ///
    /// Returns the flattened element data and the array's shape. Non-numeric
    /// arrays (`S[]`, `f[]`) yield an empty buffer with the element type name
    /// set, so they can be skipped.
    ///
    /// # Errors
    /// [`Error::TypeMismatch`] if the variable is not an array, or
    /// [`Error::NullPointer`] if it has no storage.
    pub fn array(&self) -> Result<(ArrayInfo, Vec<Myflt>)> {
        if self.type_name() != Some("[") {
            return Err(Error::TypeMismatch {
                context: "debug variable",
                expected: "array",
                actual: self.type_name().unwrap_or("unknown").to_owned(),
            });
        }
        let data = self.data_ptr()?;
        let mut info = debug_array_info_t::default();

        let total = unsafe {
            csoundDebugSerializeArray(self.csound, data, ptr::null_mut(), 0, &mut info as *mut _)
        };
        if total <= 0 {
            return Ok((ArrayInfo::from(&info), Vec::new()));
        }

        let mut buf = vec![0.0 as Myflt; total as usize];
        unsafe {
            csoundDebugSerializeArray(
                self.csound,
                data,
                buf.as_mut_ptr(),
                total,
                &mut info as *mut _,
            );
        }
        Ok((ArrayInfo::from(&info), buf))
    }

    fn data_ptr(&self) -> Result<*mut c_void> {
        let data = self.as_ref().data;
        if data.is_null() {
            return Err(Error::NullPointer("debug variable has no storage"));
        }
        Ok(data)
    }
}

// -- serialized metadata --------------------------------------------------

/// Metadata for an f-signal analysis frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsigInfo {
    /// FFT size.
    pub fft_size: i32,
    /// Number of bins, `fft_size / 2 + 1`.
    pub bins: i32,
    /// Hop size.
    pub overlap: i32,
    /// Analysis window size.
    pub window_size: i32,
    /// Window type.
    pub window_type: i32,
    /// PVS analysis format.
    pub format: i32,
    /// Increments when a new analysis frame is ready.
    pub framecount: u32,
    /// True when the source frame is a sliding (MYFLT) frame.
    pub sliding: bool,
}

impl From<&debug_fsig_info_t> for FsigInfo {
    fn from(raw: &debug_fsig_info_t) -> Self {
        FsigInfo {
            fft_size: raw.N,
            bins: raw.NB,
            overlap: raw.overlap,
            window_size: raw.winsize,
            window_type: raw.wintype,
            format: raw.format,
            framecount: raw.framecount,
            sliding: raw.sliding != 0,
        }
    }
}

/// Shape and element type of a numeric array variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayInfo {
    /// Number of dimensions.
    pub dimensions: i32,
    /// Bytes per element.
    pub member_size: i32,
    /// Total number of values in the flattened data.
    pub total_elements: i32,
    /// Element type name, such as `"k"`, `"a"` or `"i"`.
    pub element_type: String,
}

impl From<&debug_array_info_t> for ArrayInfo {
    fn from(raw: &debug_array_info_t) -> Self {
        // SAFETY: a fixed-size NUL terminated buffer written by the engine.
        let element_type = unsafe { CStr::from_ptr(raw.elementTypeName.as_ptr()) }
            .to_str()
            .unwrap_or_default()
            .to_owned();
        ArrayInfo {
            dimensions: raw.dimensions,
            member_size: raw.arrayMemberSize,
            total_elements: raw.totalElements,
            element_type,
        }
    }
}

// -- helpers --------------------------------------------------------------

/// Iterator over a NULL-terminated C linked list.
struct LinkedList<T, F: Fn(*mut T) -> *mut T> {
    current: *mut T,
    next: F,
}

impl<T, F: Fn(*mut T) -> *mut T> Iterator for LinkedList<T, F> {
    type Item = *mut T;

    fn next(&mut self) -> Option<*mut T> {
        if self.current.is_null() {
            return None;
        }
        let item = self.current;
        self.current = (self.next)(item);
        Some(item)
    }
}

/// Borrows a possibly-null C string as UTF-8.
fn cstr_opt<'a>(ptr: *const libc::c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: non-null NUL terminated string owned by the engine.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

impl Csound {
    /// Initialises the debugger and returns a guard.
    ///
    /// There is a small performance cost while the debugger is active, so the
    /// guard calls `csoundDebuggerClean` when dropped.
    ///
    /// This is not thread safe and must be called before performance starts.
    ///
    /// # Errors
    /// [`Error::CsoundCall`] if the debugger could not be initialised.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use csound::Csound;
    /// let cs = Csound::new().unwrap();
    /// cs.compile_orc("instr 1\nendin\n", 0).unwrap();
    ///
    /// let mut dbg = cs.debugger().unwrap();
    /// dbg.set_instrument_breakpoint(1.0, 0);
    /// ```
    pub fn debugger(&self) -> Result<Debugger<'_>> {
        let status = unsafe { csoundDebuggerInit(self.csound_ptr()) } as c_int;
        if status != CSOUND_STATUS::CSOUND_SUCCESS {
            return Err(Error::CsoundCall {
                operation: "csoundDebuggerInit",
                status: CsoundErrorCode::from_raw(status),
            });
        }
        Ok(Debugger {
            csound: self.csound_ptr(),
            breakpoint_cb: None,
            kcycle_cb: None,
            phantom: PhantomData,
        })
    }
}
