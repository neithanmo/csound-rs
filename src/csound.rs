#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

use std::io;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::slice;

use callbacks::*;
use channels::{ChannelBehavior, ChannelHints, ChannelInfo, PvsDataExt};
use csound_sys;
use csound_sys::RTCLOCK;
use enums::{ChannelData, ControlChannelType, Language, MessageType, Status};
use rtaudio::{CsAudioDevice, CsMidiDevice, RtAudioParams};

use std::fmt;

use std::ffi::{CStr, CString, NulError};
use std::str;
use std::str::Utf8Error;

use libc::{c_char, c_double, c_int, c_long, c_void};

// the length in bytes of the output type name in csound
const OUTPUT_TYPE_LENGTH: usize = 6;

// The length in bytes of the output format name in csound
const OUTPUT_FORMAT_LENGTH: usize = 8;

#[derive(Default, Debug)]
pub struct OpcodeListEntry {
    pub opname: String,
    pub outypes: String,
    pub intypes: String,
    pub flags: i32,
}

#[derive(Default)]
pub(crate) struct CallbackHandler {
    pub callbacks: Callbacks<'static>,
}

#[derive(Debug)]
pub struct Csound {
    engine: Inner,
}

#[derive(Debug)]
pub(crate) struct Inner {
    csound: *mut csound_sys::CSOUND,
}

unsafe impl Send for Inner {}

impl Default for Csound {
    fn default() -> Self {
        unsafe {
            // Csound must not handle signals
            csound_sys::csoundInitialize(csound_sys::CSOUNDINIT_NO_SIGNAL_HANDLER as c_int);
            csound_sys::csoundInitialize(csound_sys::CSOUNDINIT_NO_ATEXIT as c_int);

            let callback_handler = Box::new(CallbackHandler {
                callbacks: Callbacks::default(),
            });
            let host_data_ptr = Box::into_raw(callback_handler) as *mut c_void;

            let csound_sys = csound_sys::csoundCreate(host_data_ptr);
            assert!(!csound_sys.is_null());

            let engine = Inner { csound: csound_sys };
            Csound { engine }
        }
    }
}

impl Csound {
    pub fn new() -> Csound {
        Csound::default()
    }

    pub fn initialize(flags: i32) -> Result<(), &'static str> {
        unsafe {
            match csound_sys::csoundInitialize(flags as c_int) as i32 {
                csound_sys::CSOUND_ERROR => Err("Can't to initialize csound "),
                csound_sys::CSOUND_SUCCESS => Ok(()),
                value => {
                    if value > 0 {
                        Err("Initialization was done already")
                    } else {
                        Err("Unknown error - can to initialize")
                    }
                }
            }
        }
    }

    pub fn set_option(&self, options: &str) -> Result<(), &'static str> {
        let op = CString::new(options).map_err(|_| "Error parsing the string")?;
        unsafe {
            match csound_sys::csoundSetOption(self.engine.csound, op.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Options not valid"),
            }
        }
    }

    pub fn start(&self) -> Result<(), &'static str> {
        unsafe {
            let result: c_int = csound_sys::csoundStart(self.engine.csound);
            if result == csound_sys::CSOUND_SUCCESS {
                Ok(())
            } else {
                Err("Csound is already started, call csoundReset() before starting again.")
            }
        }
    }

    pub fn version(&self) -> u32 {
        unsafe { csound_sys::csoundGetVersion() as u32 }
    }

    pub fn api_version(&self) -> u32 {
        unsafe { csound_sys::csoundGetAPIVersion() as u32 }
    }

    /* Engine performance functions implementations ********************************************************* */

    pub fn stop(&self) {
        unsafe {
            csound_sys::csoundStop(self.engine.csound);
        }
    }

    pub fn reset(&self) {
        unsafe {
            csound_sys::csoundReset(self.engine.csound);
        }
    }

    pub fn compile(&self, args: &[&str]) -> Result<(), &'static str> {
        if args.is_empty() {
            return Err("Not enough arguments");
        }
        let arguments: Vec<CString> = args.iter().map(|&arg| CString::new(arg).unwrap()).collect();
        let args_raw: Vec<*const c_char> = arguments.iter().map(|arg| arg.as_ptr()).collect();
        let argv: *const *const c_char = args_raw.as_ptr();
        unsafe {
            match csound_sys::csoundCompile(self.engine.csound, args_raw.len() as c_int, argv) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't compile carguments"),
            }
        }
    }

    pub fn compile_csd(&self, csd: &str) -> Result<(), &'static str> {
        if csd.is_empty() {
            return Err("Empty file name");
        }
        let path = CString::new(csd).map_err(|_| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileCsd(self.engine.csound, path.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't compile the csd file"),
            }
        }
    }

    pub fn compile_csd_text(&self, csdText: &str) -> Result<(), &'static str> {
        if csdText.is_empty() {
            return Err("Empty file name");
        }
        let path = CString::new(csdText).map_err(|_e| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileCsdText(self.engine.csound, path.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't compile the csd file"),
            }
        }
    }

    pub fn compile_orc(&self, orcPath: &str) -> Result<(), &'static str> {
        if orcPath.is_empty() {
            return Err("Empty file name");
        }
        let path = CString::new(orcPath).map_err(|_e| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileOrc(self.engine.csound, path.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't to compile orc file"),
            }
        }
    }

    pub fn compile_orc_async(&self, orcPath: &str) -> Result<(), &'static str> {
        if orcPath.is_empty() {
            return Err("Empty file name");
        }
        let path = CString::new(orcPath).map_err(|_e| "Bad file name")?;
        unsafe {
            match csound_sys::csoundCompileOrcAsync(self.engine.csound, path.as_ptr()) {
                csound_sys::CSOUND_SUCCESS => Ok(()),
                _ => Err("Can't to compile orc file"),
            }
        }
    }

    pub fn eval_code(&self, code: &str) -> f64 {
        let cd = CString::new(code).unwrap();
        unsafe { csound_sys::csoundEvalCode(self.engine.csound, cd.as_ptr()) }
    }

    pub fn perform(&self) -> i32 {
        unsafe { csound_sys::csoundPerform(self.engine.csound) as i32 }
    }

    pub fn perform_ksmps(&self) -> bool {
        unsafe { csound_sys::csoundPerformKsmps(self.engine.csound) != 0 }
    }

    pub fn perform_buffer(&self) -> bool {
        unsafe { csound_sys::csoundPerformBuffer(self.engine.csound) != 0 }
    }

    /*********************************** UDP ****************************************************/

    pub fn udp_server_start(&self, port: u32) -> Result<(), Status> {
        unsafe {
            match Status::from(
                csound_sys::csoundUDPServerStart(self.engine.csound, port as c_int) as i32,
            ) {
                Status::CS_SUCCESS => Ok(()),
                status => Err(status),
            }
        }
    }

    pub fn udp_server_status(&self) -> Option<u32> {
        unsafe {
            let status = csound_sys::csoundUDPServerStatus(self.engine.csound);
            if status == csound_sys::CSOUND_ERROR {
                None
            } else {
                Some(status as u32)
            }
        }
    }

    pub fn udp_server_close(&self) -> Result<(), Status> {
        unsafe {
            match Status::from(csound_sys::csoundUDPServerClose(self.engine.csound) as i32) {
                Status::CS_SUCCESS => Ok(()),
                status => Err(status),
            }
        }
    }

    pub fn udp_console(&self, addr: &str, port: u32, mirror: bool) -> Result<(), Status> {
        unsafe {
            let ip = CString::new(addr).map_err(|_e| Status::CS_ERROR)?;
            if csound_sys::csoundUDPConsole(
                self.engine.csound,
                ip.as_ptr(),
                port as c_int,
                mirror as c_int,
            ) == csound_sys::CSOUND_SUCCESS
            {
                return Ok(());
            }
            Err(Status::CS_ERROR)
        }
    }

    pub fn udp_stop_console(&self) {
        unsafe {
            csound_sys::csoundStopUDPConsole(self.engine.csound);
        }
    }
    /* Engine Attributes functions implmentations ********************************************************* */

    pub fn get_sample_rate(&self) -> f64 {
        unsafe { csound_sys::csoundGetSr(self.engine.csound) as f64 }
    }

    pub fn get_control_rate(&self) -> f64 {
        unsafe { csound_sys::csoundGetKr(self.engine.csound) as f64 }
    }

    pub fn get_ksmps(&self) -> u32 {
        unsafe { csound_sys::csoundGetKsmps(self.engine.csound) }
    }

    pub fn output_channels(&self) -> u32 {
        unsafe { csound_sys::csoundGetNchnls(self.engine.csound) as u32 }
    }

    pub fn input_channels(&self) -> u32 {
        unsafe { csound_sys::csoundGetNchnlsInput(self.engine.csound) as u32 }
    }
    pub fn get_0dBFS(&self) -> f64 {
        unsafe { csound_sys::csoundGet0dBFS(self.engine.csound) as f64 }
    }

    pub fn get_freq(&self) -> f64 {
        unsafe { csound_sys::csoundGetA4(self.engine.csound) as f64 }
    }

    pub fn get_current_sample_time(&self) -> usize {
        unsafe { csound_sys::csoundGetCurrentTimeSamples(self.engine.csound) as usize }
    }

    pub fn get_size_myflt(&self) -> u32 {
        unsafe { csound_sys::csoundGetSizeOfMYFLT() as u32 }
    }

    pub fn get_debug_level(&self) -> u32 {
        unsafe { csound_sys::csoundGetDebug(self.engine.csound) as u32 }
    }

    pub fn set_debug_level(&self, level: i32) {
        unsafe {
            csound_sys::csoundSetDebug(self.engine.csound, level as c_int);
        }
    }

    /* Engine general InputOutput functions implmentations ********************************************************* */

    pub fn get_input_name(&self) -> Result<String, &'static str> {
        unsafe {
            let ptr = csound_sys::csoundGetInputName(self.engine.csound);
            if !ptr.is_null() {
                let name = CStr::from_ptr(ptr)
                    .to_str()
                    .map_err(|_| "Some Utf8 error have occurred while parsing the device name")?;
                return Ok(name.to_owned());
            }
            Err("Real time audio input is not configured in csound, you have to add the -iadc option into you csd file")
        }
    }

    pub fn get_output_name(&self) -> Result<String, &'static str> {
        unsafe {
            let ptr = csound_sys::csoundGetOutputName(self.engine.csound);
            if !ptr.is_null() {
                let name = CStr::from_ptr(ptr)
                    .to_str()
                    .map_err(|_| "Some Utf8 error have occurred while parsing the device name")?;
                return Ok(name.to_owned());
            }
            Err("Real time audio output is not configured in csound, you have to add the -odac option into you csd file")
        }
    }

    pub fn set_output(&self, name: &str, out_type: &str, format: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            let devType = CString::new(out_type)?;
            let devFormat = CString::new(format)?;

            csound_sys::csoundSetOutput(
                self.engine.csound,
                devName.as_ptr(),
                devType.as_ptr(),
                devFormat.as_ptr(),
            );
            Ok(())
        }
    }

    pub fn get_output_format(&self) -> Result<(String, String), Utf8Error> {
        let otype = vec![b'\0'; OUTPUT_TYPE_LENGTH];
        let format = vec![b'\0'; OUTPUT_FORMAT_LENGTH];
        unsafe {
            let otype = CString::from_vec_unchecked(otype).into_raw();
            let format = CString::from_vec_unchecked(format).into_raw();

            csound_sys::csoundGetOutputFormat(self.engine.csound, otype, format);

            let otype = CString::from_raw(otype);
            let otype = otype.to_str()?;

            let format = CString::from_raw(format);
            let format = format.to_str()?;

            Ok((otype.to_string(), format.to_string()))
        }
    }

    pub fn set_input(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetInput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    pub fn set_midi_file_input(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIFileInput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    pub fn set_midi_file_output(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIFileOutput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    pub fn set_midi_input(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIInput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    pub fn set_midi_output(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetMIDIOutput(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    /* Engine general Realtime Audio I/O functions implmentations ********************************************************* */

    pub fn set_rt_audio_module(&self, name: &str) -> Result<(), NulError> {
        unsafe {
            let devName = CString::new(name)?;
            csound_sys::csoundSetRTAudioModule(self.engine.csound, devName.as_ptr());
            Ok(())
        }
    }

    pub fn get_input_buffer_size(&self) -> usize {
        unsafe { csound_sys::csoundGetInputBufferSize(self.engine.csound) as usize }
    }

    pub fn get_output_buffer_size(&self) -> usize {
        unsafe { csound_sys::csoundGetOutputBufferSize(self.engine.csound) as usize }
    }

    pub fn get_input_buffer(&self) -> Option<BufferPtr<Writable>> {
        unsafe {
            let ptr = csound_sys::csoundGetInputBuffer(self.engine.csound) as *mut f64;
            let len = self.get_input_buffer_size();
            if !ptr.is_null() {
                return Some(BufferPtr {
                    ptr,
                    len,
                    phantom: PhantomData,
                });
            }
            None
        }
    }

    pub fn get_output_buffer(&self) -> Option<BufferPtr<Readable>> {
        unsafe {
            let ptr = csound_sys::csoundGetOutputBuffer(self.engine.csound) as *mut f64;
            let len = self.get_output_buffer_size();
            if !ptr.is_null() {
                return Some(BufferPtr {
                    ptr,
                    len,
                    phantom: PhantomData,
                });
            }
            None
        }
    }

    pub fn get_spin(&self) -> Option<BufferPtr<Writable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpin(self.engine.csound) as *mut f64;
            let len = (self.get_ksmps() * self.input_channels()) as usize;
            if !ptr.is_null() {
                return Some(BufferPtr {
                    ptr,
                    len,
                    phantom: PhantomData,
                });
            }
            None
        }
    }

    pub fn get_spout(&self) -> Option<BufferPtr<Readable>> {
        unsafe {
            let ptr = csound_sys::csoundGetSpout(self.engine.csound) as *mut f64;
            let len = (self.get_ksmps() * self.output_channels()) as usize;
            if !ptr.is_null() {
                return Some(BufferPtr {
                    ptr,
                    len,
                    phantom: PhantomData,
                });
            }
            None
        }
    }

    pub fn read_output_buffer(&self, output: &mut [f64]) -> Result<usize, &'static str> {
        let size = self.get_output_buffer_size();
        let obuffer =
            unsafe { csound_sys::csoundGetOutputBuffer(self.engine.csound) as *const f64 };
        let mut len = output.len();
        if size < len {
            len = size;
        }
        if !obuffer.is_null() {
            unsafe {
                std::ptr::copy(obuffer, output.as_ptr() as *mut f64, len);
                return Ok(len);
            }
        }
        Err("The output buffer is not initialized, call the 'staticompile()' and 'start()' methods.")
    }

    pub fn write_input_buffer(&self, input: &[f64]) -> Result<usize, &'static str> {
        let size = self.get_input_buffer_size();
        let ibuffer = unsafe { csound_sys::csoundGetInputBuffer(self.engine.csound) as *mut f64 };
        let mut len = input.len();
        if size < len {
            len = size;
        }
        if !ibuffer.is_null() {
            unsafe {
                std::ptr::copy(input.as_ptr(), ibuffer, len);
                return Ok(len);
            }
        }
        Err("The input buffer is not initialized, call the 'staticompile()' and 'start()' methods.")
    }

    pub fn read_spout_buffer(&self, output: &mut [f64]) -> Result<usize, &'static str> {
        let size = self.get_ksmps() as usize * self.output_channels() as usize;
        let spout = unsafe { csound_sys::csoundGetSpout(self.engine.csound) as *const f64 };
        let mut len = output.len();
        if size < len {
            len = size;
        }
        if !spout.is_null() {
            unsafe {
                std::ptr::copy(spout, output.as_mut_ptr(), len);
                return Ok(len);
            }
        }
        Err("The spout buffer is not initialized, call the 'staticompile()' and 'start()' methods.")
    }

    pub fn write_spin_buffer(&self, input: &[f64]) -> Result<usize, &'static str> {
        let size = self.get_ksmps() as usize * self.input_channels() as usize;
        let spin = unsafe { csound_sys::csoundGetSpin(self.engine.csound) as *mut f64 };
        let mut len = input.len();
        if size < len {
            len = size;
        }
        if !spin.is_null() {
            unsafe {
                std::ptr::copy(input.as_ptr(), spin, len);
                return Ok(len);
            }
        }
        Err("The spin buffer is not initialized, call the 'staticompile()' and 'start()' methods.")
    }

    pub fn clear_spin(&self) {
        unsafe {
            csound_sys::csoundClearSpin(self.engine.csound);
        }
    }

    pub fn add_spin_sample(&self, frame: u32, channel: u32, sample: f64) {
        unsafe {
            csound_sys::csoundAddSpinSample(
                self.engine.csound,
                frame as i32,
                channel as i32,
                sample as c_double,
            );
        }
    }

    pub fn set_spin_sample(&self, frame: u32, channel: u32, sample: f64) {
        unsafe {
            csound_sys::csoundSetSpinSample(
                self.engine.csound,
                frame as i32,
                channel as i32,
                sample as c_double,
            );
        }
    }

    pub fn get_spout_sample(&self, frame: u32, channel: u32) -> f64 {
        unsafe {
            csound_sys::csoundGetSpoutSample(self.engine.csound, frame as i32, channel as i32)
                as f64
        }
    }

    pub fn set_host_implemented_audioIO(&self, state: u32, bufSize: u32) {
        unsafe {
            csound_sys::csoundSetHostImplementedAudioIO(
                self.engine.csound,
                state as c_int,
                bufSize as c_int,
            );
        }
    }

    pub fn get_audio_devices(&self) -> (Vec<CsAudioDevice>, Vec<CsAudioDevice>) {
        let mut input_devices = Vec::new();
        let mut output_devices = Vec::new();

        unsafe {
            let num_of_idevices =
                csound_sys::csoundGetAudioDevList(self.engine.csound, ptr::null_mut(), 0);
            let num_of_odevices =
                csound_sys::csoundGetAudioDevList(self.engine.csound, ptr::null_mut(), 0);

            let mut in_vec = vec![csound_sys::CS_AUDIODEVICE::default(); num_of_idevices as usize];
            let mut out_vec = vec![csound_sys::CS_AUDIODEVICE::default(); num_of_odevices as usize];

            csound_sys::csoundGetAudioDevList(self.engine.csound, in_vec.as_mut_ptr(), 0);
            csound_sys::csoundGetAudioDevList(self.engine.csound, out_vec.as_mut_ptr(), 1);

            for dev in &in_vec {
                input_devices.push(CsAudioDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    rt_module: Trampoline::ptr_to_string(dev.rt_module.as_ptr()),
                    max_nchnls: dev.max_nchnls as u32,
                    isOutput: 0,
                });
            }
            for dev in &out_vec {
                output_devices.push(CsAudioDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    rt_module: Trampoline::ptr_to_string(dev.rt_module.as_ptr()),
                    max_nchnls: dev.max_nchnls as u32,
                    isOutput: 1,
                });
            }
        }
        (output_devices, input_devices)
    }

    /* Real time MIDI IO functions implmentations *************************************************************** */

    pub fn set_midi_module(&self, name: &str) {
        unsafe {
            let devName = CString::new(name);
            if devName.is_ok() {
                csound_sys::csoundSetMIDIModule(self.engine.csound, devName.unwrap().as_ptr());
            }
        }
    }

    pub fn set_host_implemented_midiIO(&self, state: u32) {
        unsafe {
            csound_sys::csoundSetHostImplementedMIDIIO(self.engine.csound, state as c_int);
        }
    }

    pub fn get_midi_devices(&self) -> (Vec<CsMidiDevice>, Vec<CsMidiDevice>) {
        let mut input_devices = Vec::new();
        let mut output_devices = Vec::new();

        unsafe {
            let num_of_idevices =
                csound_sys::csoundGetMIDIDevList(self.engine.csound, ptr::null_mut(), 0);
            let num_of_odevices =
                csound_sys::csoundGetMIDIDevList(self.engine.csound, ptr::null_mut(), 0);

            let mut in_vec = vec![csound_sys::CS_MIDIDEVICE::default(); num_of_idevices as usize];
            let mut out_vec = vec![csound_sys::CS_MIDIDEVICE::default(); num_of_odevices as usize];

            csound_sys::csoundGetMIDIDevList(self.engine.csound, in_vec.as_mut_ptr(), 0);
            csound_sys::csoundGetMIDIDevList(self.engine.csound, out_vec.as_mut_ptr(), 1);

            for dev in &in_vec {
                input_devices.push(CsMidiDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    midi_module: Trampoline::ptr_to_string(dev.midi_module.as_ptr()),
                    interface_name: Trampoline::ptr_to_string(dev.interface_name.as_ptr()),
                    isOutput: 0,
                });
            }
            for dev in &out_vec {
                output_devices.push(CsMidiDevice {
                    device_name: Trampoline::ptr_to_string(dev.device_name.as_ptr()),
                    device_id: Trampoline::ptr_to_string(dev.device_id.as_ptr()),
                    midi_module: Trampoline::ptr_to_string(dev.midi_module.as_ptr()),
                    interface_name: Trampoline::ptr_to_string(dev.interface_name.as_ptr()),
                    isOutput: 1,
                });
            }
        }
        (output_devices, input_devices)
    }

    /* Score Handling functions implmentations ********************************************************* */

    pub fn read_score(&self, score: &str) -> Result<(), &'static str> {
        unsafe {
            match CString::new(score) {
                Ok(s) => {
                    if csound_sys::csoundReadScore(self.engine.csound, s.as_ptr())
                        == csound_sys::CSOUND_SUCCESS
                    {
                        Ok(())
                    } else {
                        Err("Can't to read the score")
                    }
                }
                _ => Err("Invalid score"),
            }
        }
    }

    pub fn read_score_async(&self, score: &str) -> Result<(), &'static str> {
        unsafe {
            match CString::new(score) {
                Ok(s) => {
                    csound_sys::csoundReadScoreAsync(self.engine.csound, s.as_ptr());
                    Ok(())
                }
                _ => Err("Invalid score"),
            }
        }
    }

    pub fn get_score_time(&self) -> f64 {
        unsafe { csound_sys::csoundGetScoreTime(self.engine.csound) as f64 }
    }

    pub fn is_score_pending(&self) -> i32 {
        unsafe { csound_sys::csoundIsScorePending(self.engine.csound) as i32 }
    }

    pub fn set_score_pending(&self, pending: i32) {
        unsafe {
            csound_sys::csoundSetScorePending(self.engine.csound, pending as c_int);
        }
    }

    pub fn get_score_offset_seconds(&self) -> f64 {
        unsafe { csound_sys::csoundGetScoreOffsetSeconds(self.engine.csound) as f64 }
    }

    pub fn set_score_offset_seconds(&self, offset: f64) {
        unsafe {
            csound_sys::csoundSetScoreOffsetSeconds(self.engine.csound, offset as c_double);
        }
    }

    pub fn rewindScore(&self) {
        unsafe {
            csound_sys::csoundRewindScore(self.engine.csound);
        }
    }
    // TODO SCORE SORT FUNCTIONS

    /* Engine general messages functions implmentations ********************************************************* */

    pub fn get_message_level(&self) -> u32 {
        unsafe { csound_sys::csoundGetMessageLevel(self.engine.csound) as u32 }
    }

    pub fn set_message_level(&self, level: u32) {
        unsafe {
            csound_sys::csoundSetMessageLevel(self.engine.csound, level as c_int);
        }
    }

    pub fn create_message_buffer(&self, stdout: i32) {
        unsafe {
            csound_sys::csoundCreateMessageBuffer(self.engine.csound, stdout as c_int);
        }
    }

    pub fn destroy_message_buffer(&self) {
        unsafe {
            csound_sys::csoundDestroyMessageBuffer(self.engine.csound);
        }
    }

    pub fn get_first_message(&self) -> Option<String> {
        unsafe {
            match CStr::from_ptr(csound_sys::csoundGetFirstMessage(self.engine.csound)).to_str() {
                Ok(m) => Some(m.to_owned()),
                _ => None,
            }
        }
    }

    pub fn get_first_message_attr(&self) -> MessageType {
        unsafe {
            MessageType::from_u32(csound_sys::csoundGetFirstMessageAttr(self.engine.csound) as u32)
        }
    }

    pub fn pop_first_message(&self) {
        unsafe {
            csound_sys::csoundPopFirstMessage(self.engine.csound);
        }
    }

    pub fn get_message_count(&self) -> u32 {
        unsafe { csound_sys::csoundGetMessageCnt(self.engine.csound) as u32 }
    }

    /* Engine general Channels, Control and Events implementations ********************************************** */

    pub fn list_channels(&self) -> Option<Vec<ChannelInfo>> {
        let mut ptr = ptr::null_mut() as *mut csound_sys::controlChannelInfo_t;
        let ptr2: *mut *mut csound_sys::controlChannelInfo_t = &mut ptr as *mut *mut _;

        unsafe {
            let count = csound_sys::csoundListChannels(self.engine.csound, ptr2) as i32;
            let mut ptr = *ptr2;

            if count > 0 {
                let mut list = Vec::new();
                for _ in 0..count {
                    let name = (CStr::from_ptr((*ptr).name).to_str().unwrap()).to_owned();
                    let ctype = (*ptr).type_ as i32;
                    let hints = (*ptr).hints;
                    let mut attributes = if !(hints.attributes).is_null() {
                        (CStr::from_ptr(hints.attributes).to_str().unwrap()).to_owned()
                    } else {
                        String::new()
                    };

                    list.push(ChannelInfo {
                        name,
                        type_: ctype,
                        hints: ChannelHints {
                            behav: ChannelBehavior::from_u32(hints.behav as u32),
                            dflt: hints.dflt as f64,
                            min: hints.min as f64,
                            max: hints.max as f64,
                            x: hints.x as i32,
                            y: hints.y as i32,
                            width: hints.width as i32,
                            height: hints.height as i32,
                            attributes,
                        },
                    });
                    ptr = ptr.add(1);
                }
                csound_sys::csoundDeleteChannelList(self.engine.csound, *ptr2);
                Some(list)
            } else {
                None
            }
        }
    }

    pub fn get_channel_ptr<'a>(
        &'a self,
        name: &str,
        channel_type: ControlChannelType,
    ) -> Result<ControlChannelPtr<'a>, Status> {
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
        let mut ptr = ptr::null_mut() as *mut f64;
        let ptr = &mut ptr as *mut *mut _;
        let channel = ControlChannelType::from_bits(
            channel_type.bits() & ControlChannelType::CSOUND_CHANNEL_TYPE_MASK.bits(),
        )
        .unwrap();
        let len: usize = match channel {
            ControlChannelType::CSOUND_CONTROL_CHANNEL => std::mem::size_of::<f64>(),
            ControlChannelType::CSOUND_AUDIO_CHANNEL => self.get_ksmps() as usize,
            /*ControlChannelType::CSOUND_STRING_CHANNEL => {
                self.get_channel_data_size(name) / std::mem::size_of::<f64>()
            }*/
            _ => return Err(Status::CS_ERROR),
        };
        unsafe {
            let result = Status::from(csound_sys::csoundGetChannelPtr(
                self.engine.csound,
                ptr,
                cname.as_ptr(),
                channel_type.bits() as c_int,
            ));
            match result {
                Status::CS_SUCCESS => Ok(ControlChannelPtr {
                    ptr: *ptr,
                    channel_type: channel,
                    len,
                    phantom: PhantomData,
                }),
                Status::CS_OK(channel) => Err(Status::CS_OK(channel)),
                result => Err(result),
            }
        }
    }

    pub fn set_channel_hints(&self, name: &str, hint: &ChannelHints) -> Result<(), Status> {
        let attr = &hint.attributes[..];
        let attr = CString::new(attr).map_err(|_| Status::CS_ERROR)?;
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
        let channel_hint = csound_sys::controlChannelHints_t {
            behav: ChannelBehavior::to_u32(&hint.behav),
            dflt: hint.dflt,
            min: hint.min,
            max: hint.max,
            x: hint.x,
            y: hint.y,
            width: hint.width as c_int,
            height: hint.height as c_int,
            attributes: attr.as_ptr() as *mut c_char,
        };
        unsafe {
            match Status::from(csound_sys::csoundSetControlChannelHints(
                self.engine.csound,
                cname.as_ptr(),
                channel_hint,
            ) as i32)
            {
                Status::CS_SUCCESS => Ok(()),
                status => Err(status),
            }
        }
    }

    pub fn get_channel_hints(&self, name: &str) -> Result<ChannelHints, Status> {
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
        let hint = Box::new(csound_sys::controlChannelHints_t::default());
        unsafe {
            let hint = Box::into_raw(hint);
            match csound_sys::csoundGetControlChannelHints(
                self.engine.csound,
                cname.as_ptr() as *mut c_char,
                hint,
            ) {
                csound_sys::CSOUND_SUCCESS => {
                    let hint = Box::from_raw(hint);
                    let mut attr = if !(*hint).attributes.is_null() {
                        (CStr::from_ptr(hint.attributes).to_str().unwrap()).to_owned()
                    } else {
                        String::new()
                    };

                    let hints = ChannelHints {
                        behav: ChannelBehavior::from_u32(hint.behav as u32),
                        dflt: hint.dflt,
                        min: hint.min,
                        max: hint.max,
                        x: hint.x as i32,
                        y: hint.y as i32,
                        width: hint.width as i32,
                        height: hint.height as i32,
                        attributes: attr,
                    };
                    Ok(hints)
                }

                status => Err(Status::from(status)),
            }
        }
    }

    pub fn get_control_channel(&self, name: &str) -> Result<f64, &'static str> {
        let cname = CString::new(name).map_err(|_| "invalid channel name")?;
        let err = Box::new(csound_sys::CSOUND_ERROR);
        unsafe {
            let err = Box::into_raw(err);
            let ret =
                csound_sys::csoundGetControlChannel(self.engine.csound, cname.as_ptr(), err) as f64;
            if (*err) == csound_sys::CSOUND_SUCCESS {
                Ok(ret)
            } else {
                Err("channel not exist or is not a control channel")
            }
        }
    }

    pub fn set_control_channel(&self, name: &str, value: f64) {
        let cname = CString::new(name).unwrap();
        unsafe {
            csound_sys::csoundSetControlChannel(self.engine.csound, cname.as_ptr(), value);
        }
    }

    pub fn read_audio_channel(&self, name: &str, output: &mut [f64]) {
        let size = self.get_ksmps() as usize;
        let bytes = output.len();
        let cname = CString::new(name).unwrap();
        assert!(
            size <= bytes,
            "The audio channel's capacity is {} so, it isn't possible to copy {} samples",
            size,
            bytes
        );
        unsafe {
            csound_sys::csoundGetAudioChannel(
                self.engine.csound,
                cname.as_ptr(),
                output.as_ptr() as *mut c_double,
            );
        }
    }

    pub fn write_audio_channel(&self, name: &str, input: &[f64]) {
        let size = self.get_ksmps() as usize * self.input_channels() as usize;
        let bytes = input.len();
        let cname = CString::new(name).unwrap();
        assert!(
            size <= bytes,
            "The audio channel's capacity is {} so, it isn't possible to copy {} bytes",
            size,
            bytes
        );
        unsafe {
            csound_sys::csoundSetAudioChannel(
                self.engine.csound,
                cname.as_ptr(),
                input.as_ptr() as *mut c_double,
            );
        }
    }

    pub fn get_string_channel(&self, name: &str) -> String {
        let cname = CString::new(name).unwrap();
        let mut data = String::with_capacity(self.get_channel_data_size(name));
        unsafe {
            let ptr = data.as_mut_vec();
            csound_sys::csoundGetStringChannel(
                self.engine.csound,
                cname.as_ptr(),
                ptr.as_ptr() as *mut _,
            );
        }
        data
    }

    pub fn set_string_channel(&self, name: &str, content: &str) {
        let cname = CString::new(name).unwrap();
        let content = CString::new(content).unwrap();
        unsafe {
            csound_sys::csoundSetStringChannel(
                self.engine.csound,
                cname.as_ptr(),
                content.as_ptr() as *mut _,
            );
        }
    }

    pub fn get_channel_data_size(&self, name: &str) -> usize {
        let cname = CString::new(name).unwrap();
        unsafe { csound_sys::csoundGetChannelDatasize(self.engine.csound, cname.as_ptr()) as usize }
    }

    pub fn get_pvs_channel(&self, name: &str, pvs_data: &mut PvsDataExt) -> Result<(), Status> {
        let cname = CString::new(name).map_err(|_| Status::CS_ERROR)?;
        let mut ptr = ptr::null_mut() as *mut f64;
        unsafe {
            if csound_sys::csoundGetChannelPtr(
                self.engine.csound,
                &mut ptr as *mut *mut _,
                cname.as_ptr(),
                (csound_sys::CSOUND_PVS_CHANNEL | csound_sys::CSOUND_INPUT_CHANNEL) as c_int,
            ) == csound_sys::CSOUND_SUCCESS
            {
                if (*(ptr as *mut csound_sys::PVSDATEXT)).N == pvs_data.N as c_int {
                    let data = &mut csound_sys::PVSDATEXT::default();
                    data.frame = pvs_data.frame.as_mut_slice().as_ptr() as *mut f32;
                    let result = csound_sys::csoundGetPvsChannel(
                        self.engine.csound,
                        &mut *data,
                        cname.as_ptr(),
                    );
                    match result {
                        csound_sys::CSOUND_SUCCESS => {
                            pvs_data.N = data.N as u32;
                            pvs_data.sliding = data.sliding as u32;
                            pvs_data.NB = data.NB as i32;
                            pvs_data.overlap = data.overlap as u32;
                            pvs_data.winsize = data.winsize as u32;
                            pvs_data.wintype = data.wintype as u32;
                            pvs_data.format = data.format as u32;
                            pvs_data.framecount = data.framecount as u32;
                            Ok(())
                        }
                        err => Err(Status::from(err)),
                    }
                } else {
                    Err(Status::CS_MEMORY)
                }
            } else {
                Err(Status::CS_ERROR)
            }
        }
    }

    pub fn set_pvs_channel(&self, name: &str, pvs_data: &PvsDataExt) {
        unsafe {
            let cname = CString::new(name);
            if cname.is_ok() {
                let data = &mut csound_sys::PVSDATEXT {
                    N: pvs_data.N as _,
                    sliding: pvs_data.sliding as _,
                    NB: pvs_data.NB as _,
                    overlap: pvs_data.overlap as _,
                    winsize: pvs_data.winsize as _,
                    wintype: pvs_data.wintype as _,
                    format: pvs_data.format as _,
                    framecount: pvs_data.framecount as _,
                    frame: pvs_data.frame.as_slice().as_ptr() as *mut f32,
                };
                csound_sys::csoundSetPvsChannel(
                    self.engine.csound,
                    &*data,
                    cname.unwrap().as_ptr(),
                );
            }
        }
    }

    pub fn send_score_event(&self, event_type: char, pfields: &[f64]) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEvent(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
            ) as i32)
        }
    }

    pub fn send_score_event_absolute(
        &self,
        event_type: char,
        pfields: &[f64],
        time_offset: f64,
    ) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEventAbsolute(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
                time_offset as c_double,
            ) as i32)
        }
    }

    pub fn send_score_event_async(&self, event_type: char, pfields: &[f64]) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEventAsync(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
            ) as i32)
        }
    }

    pub fn send_score_event_absolute_async(
        &self,
        event_type: char,
        pfields: &[f64],
        time_offset: f64,
    ) -> Status {
        unsafe {
            Status::from(csound_sys::csoundScoreEventAbsoluteAsync(
                self.engine.csound,
                event_type as c_char,
                pfields.as_ptr() as *const c_double,
                pfields.len() as c_long,
                time_offset as c_double,
            ) as i32)
        }
    }

    pub fn send_input_message(&self, message: &str) -> Result<(), NulError> {
        let cmessage = CString::new(message)?;
        unsafe {
            csound_sys::csoundInputMessage(self.engine.csound, cmessage.as_ptr() as *const c_char);
            Ok(())
        }
    }

    pub fn send_input_message_async(&self, message: &str) -> Result<(), NulError> {
        let cmessage = CString::new(message)?;
        unsafe {
            csound_sys::csoundInputMessageAsync(
                self.engine.csound,
                cmessage.as_ptr() as *const c_char,
            );
            Ok(())
        }
    }

    pub fn kill_instrument(
        &self,
        instr: f64,
        name: Option<&str>,
        mode: u32,
        allow_release: bool,
    ) -> Status {
        let cname = CString::new(name.unwrap_or_else(|| "")).unwrap();
        unsafe {
            Status::from(csound_sys::csoundKillInstance(
                self.engine.csound,
                instr as c_double,
                cname.as_ptr() as *const c_char,
                mode as c_int,
                allow_release as c_int,
            ) as i32)
        }
    }

    pub fn key_press(&self, key: char) {
        unsafe {
            csound_sys::csoundKeyPress(self.engine.csound, key as c_char);
        }
    }

    /* Engine general Table function  implementations **************************************************************************************** */

    pub fn table_length(&self, table: u32) -> Result<usize, &'static str> {
        unsafe {
            let value = csound_sys::csoundTableLength(self.engine.csound, table as c_int) as i32;
            if value > 0 {
                Ok(value as usize)
            } else {
                Err("Table doesn't exist")
            }
        }
    }

    pub fn table_get(&self, table: u32, index: u32) -> Result<f64, &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if index < size as u32 {
                Ok(
                    csound_sys::csoundTableGet(self.engine.csound, table as c_int, index as c_int)
                        as f64,
                )
            } else {
                Err("index out of range")
            }
        }
    }

    pub fn table_set(&self, table: u32, index: u32, value: f64) -> Result<(), &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if index < size as u32 {
                csound_sys::csoundTableSet(
                    self.engine.csound,
                    table as c_int,
                    index as c_int,
                    value,
                );
                Ok(())
            } else {
                Err("index out of range")
            }
        }
    }

    pub fn table_copy_out(&self, table: u32, output: &mut [f64]) -> Result<(), &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if output.len() < size {
                Err("Not enough memory to copy the table")
            } else {
                csound_sys::csoundTableCopyOut(
                    self.engine.csound,
                    table as c_int,
                    output.as_ptr() as *mut c_double,
                );
                Ok(())
            }
        }
    }

    pub fn table_copy_out_async(&self, table: u32, output: &mut [f64]) -> Result<(), &'static str> {
        unsafe {
            let size = self.table_length(table)?;
            if output.len() < size {
                Err("Not enough memory to copy the table")
            } else {
                csound_sys::csoundTableCopyOutAsync(
                    self.engine.csound,
                    table as c_int,
                    output.as_ptr() as *mut c_double,
                );
                Ok(())
            }
        }
    }

    pub fn table_copy_in(&self, table: u32, src: &[f64]) -> Result<(), &'static str> {
        let size = self.table_length(table)?;
        if size < src.len() {
            Err("Table doesn't have enough capacity")
        } else {
            unsafe {
                csound_sys::csoundTableCopyIn(
                    self.engine.csound,
                    table as c_int,
                    src.as_ptr() as *const c_double,
                );
                Ok(())
            }
        }
    }

    pub fn table_copy_in_async(&self, table: u32, src: &[f64]) -> Result<(), &'static str> {
        let size = self.table_length(table)?;
        if size < src.len() {
            Err("Table doesn't have enough capacity")
        } else {
            unsafe {
                csound_sys::csoundTableCopyInAsync(
                    self.engine.csound,
                    table as c_int,
                    src.as_ptr() as *const c_double,
                );
                Ok(())
            }
        }
    }

    pub fn get_table(&self, table: u32) -> Option<Table> {
        let mut ptr = ptr::null_mut() as *mut c_double;
        let length;
        unsafe {
            length = csound_sys::csoundGetTable(
                self.engine.csound,
                &mut ptr as *mut *mut c_double,
                table as c_int,
            ) as i32;
        }
        match length {
            -1 => None,
            _ => Some(Table {
                ptr,
                length: length as usize,
                phantom: PhantomData,
            }),
        }
    }

    pub fn get_table_args(&self, table: u32) -> Option<Vec<f64>> {
        let mut ptr = ptr::null_mut() as *mut c_double;
        let length;
        unsafe {
            length = csound_sys::csoundGetTableArgs(
                self.engine.csound,
                &mut ptr as *mut *mut c_double,
                table as c_int,
            );
            if length < 0 {
                None
            } else {
                let mut result = Vec::with_capacity(length as usize);
                for pos in 0..length as isize {
                    result.push(*ptr.offset(pos));
                }
                Some(result)
            }
        }
    }

    pub fn is_named_gen(&self, gen: u32) -> usize {
        unsafe { csound_sys::csoundIsNamedGEN(self.engine.csound, gen as c_int) as usize }
    }

    pub fn get_gen_name(&self, gen: u32) -> Option<String> {
        unsafe {
            let len = self.is_named_gen(gen);
            if len > 0 {
                let name = vec![0u8; len];
                let name_raw = CString::from_vec_unchecked(name).into_raw();
                csound_sys::csoundGetNamedGEN(
                    self.engine.csound,
                    gen as c_int,
                    name_raw,
                    len as c_int,
                );
                let name = CString::from_raw(name_raw);
                let name = name.to_str().unwrap().to_owned();
                Some(name)
            } else {
                None
            }
        }
    }

    /* Engine general Opcode function  implementations **************************************************************************************** */

    pub fn get_opcode_list_entry(&self) -> Option<Vec<OpcodeListEntry>> {
        let mut ptr = ptr::null_mut() as *mut csound_sys::opcodeListEntry;
        let length;
        unsafe {
            length = csound_sys::csoundNewOpcodeList(
                self.engine.csound,
                &mut ptr as *mut *mut csound_sys::opcodeListEntry,
            );
        }
        if length < 0 {
            None
        } else {
            let mut result: Vec<OpcodeListEntry> = Vec::with_capacity(length as usize);
            for pos in 0..length as isize {
                unsafe {
                    let opname = (CStr::from_ptr((*ptr.offset(pos)).opname)).to_owned();
                    let opname = opname.into_string().unwrap();
                    let outypes = (CStr::from_ptr((*ptr.offset(pos)).outypes)).to_owned();
                    let outypes = outypes.into_string().unwrap();
                    let intypes = (CStr::from_ptr((*ptr.offset(pos)).intypes)).to_owned();
                    let intypes = intypes.into_string().unwrap();
                    let flags = (*ptr.offset(pos)).flags as i32;
                    result.push(OpcodeListEntry {
                        opname,
                        outypes,
                        intypes,
                        flags,
                    });
                }
            }
            unsafe {
                csound_sys::csoundDisposeOpcodeList(self.engine.csound, ptr);
                Some(result)
            }
        }
    }

    /**
    TODO genName and appendOpcode functions
    *****/

    /* Engine miscellaneous functions **************************************************************************************** */

    pub fn set_language(lang_code: Language) {
        unsafe {
            csound_sys::csoundSetLanguage(lang_code as u32);
        }
    }

    pub fn get_random_seed_from_time() -> u32 {
        unsafe { csound_sys::csoundGetRandomSeedFromTime() as u32 }
    }

    pub fn get_rand31(seed: &mut u32) -> Result<u32, &'static str> {
        unsafe {
            match seed {
                1...2_147_483_646 => {
                    let ptr: *mut u32 = &mut *seed;
                    let res = csound_sys::csoundRand31(ptr as *mut c_int) as u32;
                    Ok(res)
                }
                _ => Err("invalid seed value"),
            }
        }
    }

    pub fn init_timer() -> RTCLOCK {
        let mut timer = RTCLOCK::default();
        unsafe {
            let ptr: *mut RTCLOCK = &mut timer as *mut RTCLOCK;
            csound_sys::csoundInitTimerStruct(ptr);
        }
        timer
    }

    pub fn get_real_time(timer: &RTCLOCK) -> f64 {
        unsafe {
            let ptr: *mut csound_sys::RTCLOCK = &mut csound_sys::RTCLOCK {
                starttime_real: timer.starttime_real as c_long,
                starttime_CPU: timer.starttime_CPU as c_long,
            };
            csound_sys::csoundGetRealTime(ptr) as f64
        }
    }

    pub fn get_cpu_time(timer: &mut RTCLOCK) -> f64 {
        unsafe { csound_sys::csoundGetCPUTime(timer as *mut RTCLOCK) as f64 }
    }

    pub fn create_circular_buffer<'a, T: 'a + Copy>(&'a self, num_elem: u32) -> CircularBuffer<T> {
        unsafe {
            let ptr: *mut T = csound_sys::csoundCreateCircularBuffer(
                self.engine.csound,
                num_elem as c_int,
                mem::size_of::<T>() as c_int,
            ) as *mut T;
            CircularBuffer {
                csound: self.engine.csound,
                ptr,
                phantom: PhantomData,
            }
        }
    }

    // Threading function

    pub fn sleep(&self, milli_seconds: usize) {
        unsafe {
            csound_sys::csoundSleep(milli_seconds);
        }
    }

    // TODO global variables functions

    /********************************** Callback settings using the custom callback Handler implementation******/

    pub fn audio_device_list_callback<F>(&self, f: F)
    where
        F: FnMut(CsAudioDevice) + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .audio_dev_list_cb = Some(Box::new(f));
        }
        self.enable_callback(AUDIO_DEV_LIST);
    }

    pub fn play_open_audio_callback<F>(&self, f: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .play_open_cb = Some(Box::new(f));
        }
        self.enable_callback(PLAY_OPEN);
    }

    pub fn rec_open_audio_callback<F>(&self, f: F)
    where
        F: FnMut(&RtAudioParams) -> Status + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .play_open_cb = Some(Box::new(f));
        }
        self.enable_callback(REC_OPEN);
    }

    pub fn rt_audio_play_callback<F>(&self, f: F)
    where
        F: FnMut(&[f64]) + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .rt_play_cb = Some(Box::new(f));
        }
        self.enable_callback(REAL_TIME_PLAY);
    }

    pub fn rt_audio_rec_callback<F>(&self, f: F)
    where
        F: FnMut(&mut [f64]) -> usize + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .rt_rec_cb = Some(Box::new(f));
        }
        self.enable_callback(REAL_TIME_REC);
    }

    pub fn rt_close_callback<F>(&self, f: F)
    where
        F: FnMut() + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .rt_close_cb = Some(Box::new(f));
        }
        self.enable_callback(RT_CLOSE_CB);
    }

    pub fn sense_event_callback<F>(&self, f: F)
    where
        F: FnMut() + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .sense_event_cb = Some(Box::new(f));
        }
        self.enable_callback(SENSE_EVENT);
    }

    /*fn cscore_callback<F>(&mut self, f:F)
        where F: FnMut() + 'static
    {
        self.engine.inner.handler.callbacks.cscore_cb = Some(Box::new(f));
        self.engine.enable_callback(CSCORE_CB);
    }*/

    pub fn message_string_callback<F>(&self, f: F)
    where
        F: FnMut(MessageType, &str) + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .message_cb = Some(Box::new(f));
        }
        self.enable_callback(MESSAGE_CB);
    }

    /*fn keyboard_callback<F>(&self, f: F)
    where
        F: FnMut() -> char + 'static,
    {
        unsafe{(&mut *(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler)).callbacks.keyboard_cb = Some(Box::new(f));}
        self.enable_callback(KEYBOARD_CB);
    }*/

    pub fn input_channel_callback<F>(&self, f: F)
    where
        F: FnMut(&str) -> ChannelData + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .input_channel_cb = Some(Box::new(f));
        }
        self.enable_callback(CHANNEL_INPUT_CB);
    }

    pub fn output_channel_callback<F>(&self, f: F)
    where
        F: FnMut(&str, ChannelData) + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .output_channel_cb = Some(Box::new(f));
        }
        self.enable_callback(CHANNEL_OUTPUT_CB);
    }

    pub fn file_open_callback<F>(&self, f: F)
    where
        F: FnMut(&FileInfo) + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .file_open_cb = Some(Box::new(f));
        }
        self.enable_callback(FILE_OPEN_CB);
    }

    pub fn midi_in_open_callback<F>(&self, f: F)
    where
        F: FnMut(&str) + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_in_open_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_IN_OPEN_CB);
    }

    pub fn midi_out_open_callback<F>(&self, f: F)
    where
        F: FnMut(&str) + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_out_open_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_OUT_OPEN_CB);
    }

    pub fn midi_read_callback<F>(&self, f: F)
    where
        F: FnMut(&mut [u8]) -> usize + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_read_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_READ_CB);
    }

    pub fn midi_write_callback<F>(&self, f: F)
    where
        F: FnMut(&[u8]) -> usize + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_write_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_WRITE_CB);
    }

    pub fn midi_in_close_callback<F>(&self, f: F)
    where
        F: FnMut() + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_in_close_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_IN_CLOSE);
    }

    pub fn midi_out_close_callback<F>(&self, f: F)
    where
        F: FnMut() + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .midi_out_close_cb = Some(Box::new(f));
        }
        self.enable_callback(MIDI_OUT_CLOSE);
    }

    pub fn yield_callback<F>(&self, f: F)
    where
        F: FnMut() -> bool + 'static,
    {
        unsafe {
            (*(csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler))
                .callbacks
                .yield_cb = Some(Box::new(f));
        }
        self.enable_callback(YIELD_CB);
    }

    fn enable_callback(&self, callback_type: u32) {
        match callback_type {
            SENSE_EVENT => unsafe {
                csound_sys::csoundRegisterSenseEventCallback(
                    self.engine.csound,
                    Some(Trampoline::senseEventCallback),
                    ::std::ptr::null_mut() as *mut c_void,
                );
            },
            MESSAGE_CB => unsafe {
                csound_sys::csoundSetMessageStringCallback(
                    self.engine.csound,
                    Trampoline::message_string_cb,
                )
            },

            AUDIO_DEV_LIST => unsafe {
                csound_sys::csoundSetAudioDeviceListCallback(
                    self.engine.csound,
                    Some(Trampoline::audioDeviceListCallback),
                );
            },
            PLAY_OPEN => unsafe {
                csound_sys::csoundSetPlayopenCallback(
                    self.engine.csound,
                    Some(Trampoline::playOpenCallback),
                );
            },
            REC_OPEN => unsafe {
                csound_sys::csoundSetRecopenCallback(
                    self.engine.csound,
                    Some(Trampoline::recOpenCallback),
                );
            },

            REAL_TIME_PLAY => unsafe {
                csound_sys::csoundSetRtplayCallback(
                    self.engine.csound,
                    Some(Trampoline::rtplayCallback),
                );
            },

            REAL_TIME_REC => unsafe {
                csound_sys::csoundSetRtrecordCallback(
                    self.engine.csound,
                    Some(Trampoline::rtrecordCallback),
                );
            },

            /*KEYBOARD_CB => unsafe {
                let host_data_ptr = &*self.engine as *const _ as *const _;
                csound_sys::csoundRegisterKeyboardCallback(
                    self.engine.csound,
                    Some(keyboard_callback::<H>),
                    host_data_ptr as *mut c_void,
                    csound_sys::CSOUND_CALLBACK_KBD_EVENT | csound_sys::CSOUND_CALLBACK_KBD_TEXT,
                );
                csound_sys::csoundKeyPress(self.engine.csound, '\n' as i8);
            },*/
            RT_CLOSE_CB => unsafe {
                csound_sys::csoundSetRtcloseCallback(
                    self.engine.csound,
                    Some(Trampoline::rtcloseCallback),
                );
            },

            CSCORE_CB => unsafe {
                csound_sys::csoundSetCscoreCallback(
                    self.engine.csound,
                    Some(Trampoline::scoreCallback),
                );
            },

            CHANNEL_INPUT_CB => unsafe {
                csound_sys::csoundSetInputChannelCallback(
                    self.engine.csound,
                    Some(Trampoline::inputChannelCallback),
                );
            },

            CHANNEL_OUTPUT_CB => unsafe {
                csound_sys::csoundSetOutputChannelCallback(
                    self.engine.csound,
                    Some(Trampoline::outputChannelCallback),
                );
            },

            FILE_OPEN_CB => unsafe {
                csound_sys::csoundSetFileOpenCallback(
                    self.engine.csound,
                    Some(Trampoline::fileOpenCallback),
                );
            },

            MIDI_IN_OPEN_CB => unsafe {
                csound_sys::csoundSetExternalMidiInOpenCallback(
                    self.engine.csound,
                    Some(Trampoline::midiInOpenCallback),
                );
            },

            MIDI_OUT_OPEN_CB => unsafe {
                csound_sys::csoundSetExternalMidiOutOpenCallback(
                    self.engine.csound,
                    Some(Trampoline::midiOutOpenCallback),
                );
            },

            MIDI_READ_CB => unsafe {
                csound_sys::csoundSetExternalMidiReadCallback(
                    self.engine.csound,
                    Some(Trampoline::midiReadCallback),
                );
            },

            MIDI_WRITE_CB => unsafe {
                csound_sys::csoundSetExternalMidiWriteCallback(
                    self.engine.csound,
                    Some(Trampoline::midiWriteCallback),
                );
            },

            MIDI_IN_CLOSE => unsafe {
                csound_sys::csoundSetExternalMidiInCloseCallback(
                    self.engine.csound,
                    Some(Trampoline::midiInCloseCallback),
                );
            },

            MIDI_OUT_CLOSE => unsafe {
                csound_sys::csoundSetExternalMidiOutCloseCallback(
                    self.engine.csound,
                    Some(Trampoline::midiOutCloseCallback),
                );
            },

            YIELD_CB => unsafe {
                csound_sys::csoundSetYieldCallback(
                    self.engine.csound,
                    Some(Trampoline::yieldCallback),
                );
            },

            _ => {}
        }
    }
} //End impl block

// Drop method to free the memory using during the csound performance and instantiation
impl Drop for Csound {
    fn drop(&mut self) {
        unsafe {
            csound_sys::csoundStop(self.engine.csound);
            csound_sys::csoundCleanup(self.engine.csound);
            csound_sys::csoundDestroy(self.engine.csound);
            let _ = Box::from_raw(
                csound_sys::csoundGetHostData(self.engine.csound) as *mut CallbackHandler
            );
        }
    }
}

pub struct CircularBuffer<'a, T: 'a + Copy> {
    csound: *mut csound_sys::CSOUND,
    ptr: *mut T,
    //pub num_elem: u32,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> CircularBuffer<'a, T>
where
    T: Copy,
{
    pub fn read(&self, out: &mut [T], items: u32) -> Result<usize, &'static str> {
        if items as usize <= out.len() {
            return Err("your buffer has not enough capacity");
        }
        unsafe {
            Ok(csound_sys::csoundReadCircularBuffer(
                self.csound,
                self.ptr as *mut c_void,
                out.as_mut_ptr() as *mut c_void,
                items as c_int,
            ) as usize)
        }
    }

    pub fn peek(&self, out: &mut [T], items: u32) -> Result<usize, &'static str> {
        if items as usize <= out.len() {
            return Err("your buffer has not enough capacity");
        }
        unsafe {
            Ok(csound_sys::csoundPeekCircularBuffer(
                self.csound,
                self.ptr as *mut c_void,
                out.as_mut_ptr() as *mut c_void,
                items as c_int,
            ) as usize)
        }
    }

    pub fn write(&self, input: &[T], items: u32) -> Result<usize, &'static str> {
        if items as usize <= input.len() {
            return Err("your buffer has not enough capacity");
        }
        unsafe {
            Ok(csound_sys::csoundWriteCircularBuffer(
                self.csound,
                self.ptr as *mut c_void,
                input.as_ptr() as *const c_void,
                items as c_int,
            ) as usize)
        }
    }

    pub fn flush(&self) {
        unsafe {
            csound_sys::csoundFlushCircularBuffer(self.csound, self.ptr as *mut c_void);
        }
    }
}

impl<'a, T> Drop for CircularBuffer<'a, T>
where
    T: Copy,
{
    fn drop(&mut self) {
        unsafe {
            csound_sys::csoundDestroyCircularBuffer(self.csound, self.ptr as *mut c_void);
        }
    }
}

#[derive(Debug)]
pub struct Table<'a> {
    ptr: *mut f64,
    length: usize,
    phantom: PhantomData<&'a f64>,
}

impl<'a> Table<'a> {
    pub fn get_size(&self) -> usize {
        self.length
    }

    pub fn as_slice(&self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.ptr, self.length) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.length) }
    }

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

pub enum Readable {}
pub enum Writable {}

pub struct BufferPtr<'a, T> {
    ptr: *mut f64,
    len: usize,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> BufferPtr<'a, T> {
    pub fn get_size(&self) -> usize {
        self.len
    }

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

    pub fn as_slice(&self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<'a> BufferPtr<'a, Writable> {
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }

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

impl<'a, T> AsRef<[f64]> for BufferPtr<'a, T> {
    fn as_ref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> AsMut<[f64]> for BufferPtr<'a, Writable> {
    fn as_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}

impl<'a, T> Deref for BufferPtr<'a, T> {
    type Target = [f64];
    fn deref(&self) -> &[f64] {
        self.as_slice()
    }
}

impl<'a> DerefMut for BufferPtr<'a, Writable> {
    fn deref_mut(&mut self) -> &mut [f64] {
        self.as_mut_slice()
    }
}

#[derive(Debug)]
pub struct ControlChannelPtr<'a> {
    ptr: *mut f64,
    len: usize,
    channel_type: ControlChannelType,
    phantom: PhantomData<&'a f64>,
}

impl<'a> ControlChannelPtr<'a> {
    pub fn get_size(&self) -> usize {
        self.len
    }

    pub fn read<T: Copy>(&self, dest: &mut [T]) -> Result<usize, io::Error> {
        let mut len: usize = dest.len();
        if self.len < len {
            len = self.len;
        }
        if self.len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Missing data: requesting {} but only got {}.",
                    len, self.len
                ),
            ));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(self.ptr as *const T, dest.as_mut_ptr(), len);
        }
        Ok(len)
    }

    pub fn write<T: Copy>(&self, src: &[T]) -> Result<usize, io::Error> {
        let mut len: usize = src.len();
        if self.len < len {
            len = self.len;
        }
        if self.len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Not memory for data: writing {} but only got {}.",
                    len, self.len
                ),
            ));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.ptr as *mut T, len);
        }
        Ok(len)
    }
}
