//! Conversions between stable high-level Rust types and target-dependent
//! bindgen representations.
//!
//! C enum signedness is implementation-defined: bindgen commonly emits these
//! enum modules as `u32` on Unix targets and `i32` for MSVC. Keep that ABI
//! detail at this boundary instead of spreading platform casts through the
//! safe API.
//!
//! The high-level representation is normalized to `u32` because types such as
//! [`ChannelBehavior::Unknown`] preserve unknown values as `u32`. FFI
//! conversions pass through an intermediate `i32` because the non-negative
//! `i32` range (`0..=i32::MAX`) is the portable intersection of bindgen's
//! signed and unsigned representations. This rejects negative `i32` and
//! oversized `u32` values instead of silently wrapping either one.

use libc::c_int;

use crate::channels::ChannelBehavior;
use crate::enums::{ChannelDirections, ChannelType, Language};

/// Converts a high-level `u32` value through the portable C enum domain
/// (`0..=i32::MAX`) and into bindgen's target-dependent `i32` or `u32`.
fn c_enum_from_u32<T>(value: u32) -> Option<T>
where
    T: TryFrom<i32>,
{
    let portable = i32::try_from(value).ok()?;
    T::try_from(portable).ok()
}

/// Converts bindgen's target-dependent `i32` or `u32` through the portable C
/// enum domain (`0..=i32::MAX`) and into the high-level `u32` representation.
fn c_enum_into_u32<T>(value: T) -> Option<u32>
where
    i32: TryFrom<T>,
{
    let portable = i32::try_from(value).ok()?;
    u32::try_from(portable).ok()
}

pub(crate) fn channel_behavior_from_raw(
    value: csound_sys::controlChannelBehavior::Type,
) -> Option<ChannelBehavior> {
    c_enum_into_u32(value).map(ChannelBehavior::from)
}

pub(crate) fn channel_behavior_to_raw(
    value: ChannelBehavior,
) -> Option<csound_sys::controlChannelBehavior::Type> {
    c_enum_from_u32(value.to_u32())
}

pub(crate) fn channel_type_from_raw(value: c_int) -> Option<ChannelType> {
    let raw = u32::try_from(value).ok()?;
    ChannelType::try_from(raw & ChannelType::TYPE_MASK).ok()
}

pub(crate) fn channel_directions_from_raw(value: c_int) -> Option<ChannelDirections> {
    let raw = u32::try_from(value).ok()?;
    Some(ChannelDirections::from_bits_retain(
        raw & !ChannelType::TYPE_MASK,
    ))
}

pub(crate) fn channel_type_to_raw(
    channel_type: ChannelType,
    directions: ChannelDirections,
) -> Option<c_int> {
    c_int::try_from(channel_type.raw() | directions.bits()).ok()
}

pub(crate) fn language_to_raw(value: Language) -> csound_sys::cslanguage_t::Type {
    let value = value as u32;
    // `Language` is a closed enum whose current discriminants are 0..=71, so
    // both bindgen representations can express every variant. Falling back to
    // Csound's default language keeps this boundary non-panicking if that
    // invariant is ever broken by a future variant.
    c_enum_from_u32(value).unwrap_or(csound_sys::cslanguage_t::CSLANGUAGE_DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::FileTypes;
    use csound_sys::{controlChannelBehavior, controlChannelType, cslanguage_t};

    #[test]
    fn stable_channel_types_match_csound_constants() {
        let cases = [
            (
                ChannelType::Control,
                controlChannelType::CSOUND_CONTROL_CHANNEL,
            ),
            (ChannelType::Audio, controlChannelType::CSOUND_AUDIO_CHANNEL),
            (
                ChannelType::String,
                controlChannelType::CSOUND_STRING_CHANNEL,
            ),
            (ChannelType::Pvs, controlChannelType::CSOUND_PVS_CHANNEL),
            (ChannelType::Var, controlChannelType::CSOUND_VAR_CHANNEL),
            (ChannelType::Array, controlChannelType::CSOUND_ARRAY_CHANNEL),
        ];

        for (stable, raw) in cases {
            assert_eq!(Some(stable.raw()), c_enum_into_u32(raw));
        }

        assert_eq!(
            c_enum_into_u32(controlChannelType::CSOUND_CHANNEL_TYPE_MASK),
            Some(ChannelType::TYPE_MASK)
        );
        assert_eq!(
            c_enum_into_u32(controlChannelType::CSOUND_INPUT_CHANNEL),
            Some(ChannelDirections::INPUT.bits())
        );
        assert_eq!(
            c_enum_into_u32(controlChannelType::CSOUND_OUTPUT_CHANNEL),
            Some(ChannelDirections::OUTPUT.bits())
        );
    }

    #[test]
    fn channel_behavior_conversion_matches_csound_constants() {
        let cases = [
            (
                ChannelBehavior::NoHints,
                controlChannelBehavior::CSOUND_CONTROL_CHANNEL_NO_HINTS,
            ),
            (
                ChannelBehavior::Integer,
                controlChannelBehavior::CSOUND_CONTROL_CHANNEL_INT,
            ),
            (
                ChannelBehavior::Linear,
                controlChannelBehavior::CSOUND_CONTROL_CHANNEL_LIN,
            ),
            (
                ChannelBehavior::Exponential,
                controlChannelBehavior::CSOUND_CONTROL_CHANNEL_EXP,
            ),
        ];

        for (stable, raw) in cases {
            assert_eq!(
                channel_behavior_to_raw(stable).and_then(c_enum_into_u32),
                c_enum_into_u32(raw)
            );
            assert_eq!(channel_behavior_from_raw(raw), Some(stable));
        }
    }

    #[test]
    fn language_conversion_matches_csound_constants() {
        assert_eq!(
            c_enum_into_u32(language_to_raw(Language::Default)),
            c_enum_into_u32(cslanguage_t::CSLANGUAGE_DEFAULT)
        );
        assert_eq!(
            c_enum_into_u32(language_to_raw(Language::EnglishUs)),
            c_enum_into_u32(cslanguage_t::CSLANGUAGE_ENGLISH_US)
        );
    }

    #[test]
    fn file_types_do_not_wrap_unknown_c_values() {
        assert_eq!(FileTypes::from(1_i32), FileTypes::UnifiedCSD);
        assert_eq!(FileTypes::from(257_i32), FileTypes::Unknown);
    }

    #[test]
    fn channel_type_formatting_is_human_friendly() {
        assert_eq!(format!("{:?}", ChannelType::Audio), "Audio");
        assert_eq!(
            format!("{}", ChannelType::try_from(8).unwrap()),
            "Unknown(8)"
        );
    }

    #[test]
    fn message_types_ignore_formatting_and_preserve_unknown_categories() {
        assert_eq!(
            crate::enums::MessageType::from(0x1000 | 0x0007),
            crate::enums::MessageType::Error
        );
        assert_eq!(
            crate::enums::MessageType::from(0x6000),
            crate::enums::MessageType::Unknown(0x6000)
        );
    }

    #[test]
    fn channel_values_split_type_and_direction_bits() {
        let raw = controlChannelType::CSOUND_PVS_CHANNEL as c_int
            | controlChannelType::CSOUND_INPUT_CHANNEL as c_int
            | controlChannelType::CSOUND_OUTPUT_CHANNEL as c_int;

        assert_eq!(channel_type_from_raw(raw), Some(ChannelType::Pvs));
        assert_eq!(
            channel_directions_from_raw(raw),
            Some(ChannelDirections::INPUT | ChannelDirections::OUTPUT)
        );
        assert_eq!(
            channel_type_to_raw(
                ChannelType::Pvs,
                ChannelDirections::INPUT | ChannelDirections::OUTPUT
            ),
            Some(raw)
        );
    }

    #[test]
    fn conversions_reject_values_outside_their_domains() {
        let too_large = i32::MAX as u32 + 1;

        assert_eq!(c_enum_from_u32::<i32>(too_large), None);
        assert_eq!(c_enum_into_u32(-1_i32), None);
        assert_eq!(
            channel_behavior_to_raw(ChannelBehavior::Unknown(too_large)),
            None
        );
        assert_eq!(
            ChannelType::try_from(16),
            Err(crate::enums::InvalidChannelTypeCode(16))
        );
        assert_eq!(
            ChannelType::try_from(300),
            Err(crate::enums::InvalidChannelTypeCode(300))
        );
        assert!(!ChannelType::try_from(8).unwrap().is_known());
        assert_eq!(channel_type_from_raw(-1), None);
        assert_eq!(channel_directions_from_raw(-1), None);
        assert_eq!(
            channel_type_to_raw(
                ChannelType::try_from(15).unwrap(),
                ChannelDirections::from_bits_retain(u32::MAX)
            ),
            None
        );
    }
}
