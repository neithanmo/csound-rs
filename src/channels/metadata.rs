use crate::Myflt;

/// Indicates the channel behavior.
// Unknown(u32) preserves unrecognized values from the C API, keeping
// forward-compatibility as csound adds new behavior types.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ChannelBehavior {
    /// No hints provided.
    NoHints,
    /// Integer values.
    Integer,
    /// Linear interpolation.
    Linear,
    /// Exponential interpolation.
    Exponential,
    /// Unrecognized behavior value from the C API.
    Unknown(u32),
}

impl From<u32> for ChannelBehavior {
    fn from(value: u32) -> Self {
        match value {
            0 => ChannelBehavior::NoHints,
            1 => ChannelBehavior::Integer,
            2 => ChannelBehavior::Linear,
            3 => ChannelBehavior::Exponential,
            other => ChannelBehavior::Unknown(other),
        }
    }
}

impl ChannelBehavior {
    pub fn to_u32(self) -> u32 {
        match self {
            ChannelBehavior::NoHints => 0,
            ChannelBehavior::Integer => 1,
            ChannelBehavior::Linear => 2,
            ChannelBehavior::Exponential => 3,
            ChannelBehavior::Unknown(v) => v,
        }
    }
}

/// Holds the channel HINTS information.
///
/// This hints (information) is metadata which describes the channel
/// and for what it is used for. These hints can be configured using the
/// [`chn`](https://csound.com/docs/manual/chn.html) opcode or through
/// [`Csound::set_channel_hints`](crate::Csound::set_channel_hints)
/// and [`Csound::get_channel_hints`](crate::Csound::get_channel_hints).
#[derive(Debug, Clone)]
pub struct ChannelHints {
    /// The channel behavior hint (e.g., linear, exponential scaling).
    pub behav: ChannelBehavior,
    /// Default value for the channel.
    pub dflt: Myflt,
    /// Minimum value for the channel.
    pub min: Myflt,
    /// Maximum value for the channel.
    pub max: Myflt,
    /// Suggested x position for GUI display.
    pub x: i32,
    /// Suggested y position for GUI display.
    pub y: i32,
    /// Suggested width for GUI display.
    pub width: i32,
    /// Suggested height for GUI display.
    pub height: i32,
    /// Optional free-form attributes string for GUI controllers.
    ///
    /// From the Csound C API: "This member must be set explicitly to NULL if not used."
    ///
    /// This field corresponds to the `Sattributes` parameter in the `chn_k` opcode.
    /// It provides additional metadata that front-ends can use to customize
    /// channel/controller presentation or behavior.
    ///
    /// - `None`: No attributes were set (maps to NULL in C API)
    /// - `Some(String)`: Contains the attributes string
    pub attributes: Option<String>,
}

impl Default for ChannelHints {
    fn default() -> ChannelHints {
        ChannelHints {
            behav: ChannelBehavior::NoHints,
            dflt: 0.0 as Myflt,
            min: 0.0 as Myflt,
            max: 0.0 as Myflt,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            attributes: None,
        }
    }
}

/// Holds all relevant information about a Csound bus channel.
#[derive(Debug, Clone, Default)]
pub struct ChannelInfo {
    /// The channel name.
    pub name: String,
    /// The channel type.
    pub type_: i32,
    /// Channel extra metadata.
    pub hints: ChannelHints,
}
