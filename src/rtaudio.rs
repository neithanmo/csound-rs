use std::fmt;

/// Struct with specific audio device information.
#[derive(Clone, Default)]
pub struct CsAudioDevice {
    pub device_name: String,
    pub device_id: String,
    pub rt_module: String,
    pub max_nchnls: u32,
    pub is_output: u32,
}

/// Struct with specific MIDI device information.
#[derive(Clone, Default)]
pub struct CsMidiDevice {
    pub device_name: String,
    pub interface_name: String,
    pub device_id: String,
    pub midi_module: String,
    pub is_output: u32,
}

impl fmt::Debug for CsMidiDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CsMidiDevice")
            .field("device_name", &self.device_name)
            .field("interface_name", &self.interface_name)
            .field("device_id", &self.device_id)
            .field("midi_module", &self.midi_module)
            .field("is_output", &self.is_output)
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
            .field("is_output", &self.is_output)
            .finish()
    }
}

/// Real time audio params for a specific
/// audio Device.
#[derive(Debug, Clone, Default)]
pub struct RtAudioParams {
    /// Device Name.
    pub dev_name: Option<String>,
    /// Device number.
    pub dev_num: u32,
    /// Device software buffer size.
    pub buf_samp_sw: u32,
    /// Device hardware buffer size.
    pub buf_samp_hw: u32,
    /// Device max number of channels supported.
    pub n_channels: u32,
    /// Device audio sample format.
    pub sample_format: u32,
    /// Device max sample rate.
    pub sample_rate: f32,
}
