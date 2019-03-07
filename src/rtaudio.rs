#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

use std::fmt;

#[derive(Clone, Default)]
pub struct CsAudioDevice {
    pub device_name: String,
    pub device_id: String,
    pub rt_module: String,
    pub max_nchnls: u32,
    pub isOutput: u32,
}

#[derive(Clone, Default)]
pub struct CsMidiDevice {
    pub device_name: String,
    pub interface_name: String,
    pub device_id: String,
    pub midi_module: String,
    pub isOutput: u32,
}

impl fmt::Debug for CsMidiDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CsMidiDevice")
            .field("device_name", &self.device_name)
            .field("interface_name", &self.interface_name)
            .field("device_id", &self.device_id)
            .field("midi_module", &self.midi_module)
            .field("isOutput", &self.isOutput)
            .finish()
    }
}

impl fmt::Debug for CsAudioDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CsAudioDevice")
            .field("device_name", &self.device_name)
            .field("device_id", &self.device_id)
            .field("rt_module", &self.rt_module)
            .field("max_nchnls", &self.max_nchnls)
            .field("isOutput", &self.isOutput)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RtAudioParams {
    pub devName: String,
    pub devNum: u32,
    pub bufSamp_SW: u32,
    pub bufSamp_HW: u32,
    pub nChannels: u32,
    pub sampleFormat: u32,
    pub sampleRate: f32,
}
