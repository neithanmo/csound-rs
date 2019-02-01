
/// Indicates the channel behaivor.
#[derive(Debug, PartialEq, Clone)]
pub enum ChannelBehavior{
    CHANNEL_NO_HINTS  = 0,
    CHANNEL_INT       = 1,
    CHANNEL_LIN       = 2,
    CHANNEL_EXP       = 3,
}

impl ChannelBehavior {
    pub fn from_u32(value: u32) -> ChannelBehavior {
        match value {
            0 => ChannelBehavior::CHANNEL_NO_HINTS,
            1 => ChannelBehavior::CHANNEL_INT,
            2 => ChannelBehavior::CHANNEL_LIN,
            3 => ChannelBehavior::CHANNEL_EXP,
            _ => panic!("Unknown channel behavior type"),
        }
    }

    pub fn to_u32(&self) -> u32 {
        match self {
            ChannelBehavior::CHANNEL_NO_HINTS => 0,
            ChannelBehavior::CHANNEL_INT => 1,
            ChannelBehavior::CHANNEL_LIN => 2,
            ChannelBehavior::CHANNEL_EXP => 3,
        }
    }
}

/// Holds the channel HINTS information.
///
/// This hints(information) is metadata which describes the channel
/// and what it is used for. This hints could be configured using the
/// [`chn`](https://csound.com/docs/manual/chn.html) opcode or through of [`Csound::set_channel_hints`](struct.Csound.html#method.set_channel_hints)
/// and [`Csound::get_channel_hints`](struct.Csound.html#method.get_channel_hints) functions.
///
#[derive(Debug, Clone)]
pub struct ChannelHints {
    pub behav: ChannelBehavior,
    pub dflt: f64,
    pub min: f64,
    pub max: f64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub attributes: String,
}

impl Default for ChannelHints {
    fn default() -> ChannelHints {
        ChannelHints{
            behav: ChannelBehavior::CHANNEL_NO_HINTS,
            dflt: 0f64,
            min: 0f64,
            max: 0f64,
            x: 0i32,
            y: 0i32,
            width: 0i32,
            height: 0i32,
            attributes: String::default(),
        }
    }
}

/// Holds all relevant information about a csound bus channel.
#[derive(Debug, Clone, Default)]
pub struct ChannelInfo {
    /// The channel name.
    pub name: String,
    /// The channel type.
    pub type_: i32,
    /// Channel extra metadata.
    pub hints: ChannelHints,
}

/// Holds pvs data info of a pvs channel.
///
/// To be used with [pvsin](http://www.csounds.com/manual/html/pvsin.html),
/// [`pvsout`](http://www.csounds.com/manual/html/pvsin.html) opcodes and with
/// [`Csound::get_pvs_channel`](struct.Csound.html#method.get_pvs_channel) and [`Csound::set_pvs_channel`](struct.Csound.html#method.set_pvs_channel)
/// methods.
///
#[derive(Debug, Clone)]
pub struct pvs_DataExt{
    pub N: u32,
    pub sliding: u32,
    pub NB: i32,
    pub overlap:u32,
    pub winsize:u32,
    pub wintype:u32,
    pub format:u32,
    pub framecount: u32,
    pub frame: Vec<f32>,
}

impl pvs_DataExt{
    /// Creates a new pvs data channel struct.
    ///
    /// # Arguments
    /// * `winsize` The number of elements in the pvs window and also it is the
    /// number of samples in the frame buffer.
    pub fn new(winsize: u32) -> pvs_DataExt {
        pvs_DataExt {
            N: winsize,
            sliding: 0,
            NB: 0,
            overlap: 0,
            winsize,
            wintype: 0,
            format: 0,
            framecount:0,
            frame: vec![0.0; winsize as usize],
        }
    }
}
