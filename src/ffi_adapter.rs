//! Conversions between stable high-level Rust types and target-dependent
//! bindgen representations.
//!
//! C enum signedness is implementation-defined: bindgen commonly emits these
//! enum modules as `u32` on Unix targets and `i32` for MSVC. Keep that ABI
//! detail at this boundary instead of spreading platform casts through the
//! safe API.
//!
//! The high-level representation is normalized to `u32` because types such as
//! [`ChannelBehavior::Unknown`] preserve unknown values as `u32`, and
//! [`ControlChannelType`] is backed by `u32`. FFI conversions pass through an
//! intermediate `i32` because the non-negative `i32` range (`0..=i32::MAX`) is
//! the portable intersection of bindgen's signed and unsigned representations.
//! This rejects negative `i32` and oversized `u32` values instead of silently
//! wrapping either one.

use libc::c_int;

use crate::channels::ChannelBehavior;
use crate::enums::{ControlChannelType, Language};

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

pub(crate) fn channel_type_from_raw(value: c_int) -> Option<ControlChannelType> {
    u32::try_from(value)
        .ok()
        .map(ControlChannelType::from_bits_retain)
}

pub(crate) fn channel_type_to_raw(value: ControlChannelType) -> Option<c_int> {
    c_int::try_from(value.bits()).ok()
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
    use csound_sys::{controlChannelBehavior, controlChannelType, cslanguage_t};

    #[test]
    fn stable_channel_types_match_csound_constants() {
        let cases = [
            (
                ControlChannelType::Control,
                controlChannelType::CSOUND_CONTROL_CHANNEL,
            ),
            (
                ControlChannelType::Audio,
                controlChannelType::CSOUND_AUDIO_CHANNEL,
            ),
            (
                ControlChannelType::String,
                controlChannelType::CSOUND_STRING_CHANNEL,
            ),
            (
                ControlChannelType::Pvs,
                controlChannelType::CSOUND_PVS_CHANNEL,
            ),
            (
                ControlChannelType::Var,
                controlChannelType::CSOUND_VAR_CHANNEL,
            ),
            (
                ControlChannelType::Array,
                controlChannelType::CSOUND_ARRAY_CHANNEL,
            ),
            (
                ControlChannelType::TypeMask,
                controlChannelType::CSOUND_CHANNEL_TYPE_MASK,
            ),
            (
                ControlChannelType::Input,
                controlChannelType::CSOUND_INPUT_CHANNEL,
            ),
            (
                ControlChannelType::Output,
                controlChannelType::CSOUND_OUTPUT_CHANNEL,
            ),
        ];

        for (stable, raw) in cases {
            assert_eq!(Some(stable.bits()), c_enum_into_u32(raw));
        }
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
    fn conversions_reject_values_outside_the_portable_enum_range() {
        let too_large = i32::MAX as u32 + 1;

        assert_eq!(c_enum_from_u32::<i32>(too_large), None);
        assert_eq!(c_enum_into_u32(-1_i32), None);
        assert_eq!(
            channel_behavior_to_raw(ChannelBehavior::Unknown(too_large)),
            None
        );
        assert_eq!(
            channel_type_to_raw(ControlChannelType::from_bits_retain(u32::MAX)),
            None
        );
    }
}
