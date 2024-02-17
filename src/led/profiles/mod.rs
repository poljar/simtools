// Copyright (c) 2024 Damir Jelić
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Module containing parsers for popular LED light profile file formats for Sim racing which
//! configure how LED lights on Sim racing dashboards and steering wheels should operate.

use std::num::NonZeroUsize;

use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use uuid::Uuid;

use self::{
    flag::FlagContainer,
    groups::{ConditionalGroupContainer, SimpleGroupContainer, TimeLimitedGroupContainer},
    redline::RedlineReachedContainer,
    rpm::{RpmContainer, RpmSegmentsContainer},
    speed_limiter::SpeedLimiterAnimationContainer,
};

pub use self::helpers::*;
mod helpers;

pub mod flag;
pub mod groups;
pub mod redline;
pub mod rpm;
pub mod speed_limiter;

/// The [`LedProfile`] struct contains configurations for controlling RGB LED lights.
///
/// This struct collects configurations and definitions how LED lights on a steering wheel or data
/// display unit should behave depending on the inputs of a simracing game.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LedProfile {
    /// The name of the profile.
    pub name: String,
    /// The unique ID of the profile.
    pub profile_id: Uuid,
    /// The brightness of all the LEDs this profile configures.
    pub global_brightness: f64,
    /// Should the [`LedProfile::global_brightness`] property of the profile be used to configure
    /// the brightness of all the LEDs?
    pub use_profile_brightness: bool,
    /// TODO: What does this do?
    #[serde(default)]
    pub automatic_switch: bool,
    pub embedded_javascript: Option<String>,
    pub game_code: Option<String>,
    /// A list of [`LedContainer`] values which configure a set of LEDs.
    pub led_containers: Vec<LedContainer>,
}

/// The [`LedContainer`] contains a single configuration for the behavior of a set of LED lights.
///
/// There are different container types, each of them might react to different inputs, i.e. there
/// are containers that react to the RPM of the engine, to flags being waved on the track, and
/// other various track and car conditions being met.
#[derive(Debug, Clone)]
pub enum LedContainer {
    Rpm(RpmContainer),
    RpmSegments(RpmSegmentsContainer),
    RedlineReached(RedlineReachedContainer),
    SpeedLimiterAnimation(SpeedLimiterAnimationContainer),
    Group(GroupContainer),
    BlueFlag(FlagContainer),
    WhiteFlag(FlagContainer),
    YellowFlag(FlagContainer),
    Unknown {
        start_position: NonZeroUsize,
        container_type: String,
        content: Box<RawValue>,
    },
}

impl LedContainer {
    pub fn start_position(&self) -> NonZeroUsize {
        match self {
            LedContainer::Rpm(c) => c.start_position,
            LedContainer::RpmSegments(c) => c.start_position,
            LedContainer::RedlineReached(c) => c.start_position,
            LedContainer::SpeedLimiterAnimation(c) => c.start_position,
            LedContainer::Group(c) => c.start_position(),
            LedContainer::BlueFlag(c) => c.start_position,
            LedContainer::WhiteFlag(c) => c.start_position,
            LedContainer::YellowFlag(c) => c.start_position,
            LedContainer::Unknown { start_position, .. } => *start_position,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GroupContainer {
    Simple(SimpleGroupContainer),
    GameRunning(SimpleGroupContainer),
    CarStarted(TimeLimitedGroupContainer),
    Conditional(ConditionalGroupContainer),
}

impl GroupContainer {
    pub fn start_position(&self) -> NonZeroUsize {
        match self {
            GroupContainer::Simple(c) => c.start_position,
            GroupContainer::GameRunning(c) => c.start_position,
            GroupContainer::CarStarted(c) => c.start_position,
            GroupContainer::Conditional(c) => c.start_position,
        }
    }
}

impl<'de> Deserialize<'de> for LedContainer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        fn from_str<'a, T, E>(string: &'a str) -> Result<T, E>
        where
            T: serde::Deserialize<'a>,
            E: serde::de::Error,
        {
            serde_json::from_str(string).map_err(serde::de::Error::custom)
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct Helper<'a> {
            container_type: &'a str,
            #[serde(default = "default_non_zero")]
            start_position: NonZeroUsize,
        }

        let json = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;

        let helper: Helper<'_> =
            serde_json::from_str(json.get()).map_err(serde::de::Error::custom)?;

        // The container type might have a dot delimited prefix or it might have just the container
        // type. This will handle both cases, and give us the container type in PascalCase.
        let container_type = helper.container_type.split('.').last().ok_or_else(|| {
            serde::de::Error::custom(format!(
                "Container type doesn't have a falid form: {}",
                helper.container_type
            ))
        })?;

        let content = json.get();

        Ok(match container_type {
            "RPMContainer" => LedContainer::Rpm(from_str(content)?),
            "RPMSegmentsContainer" => LedContainer::RpmSegments(from_str(content)?),
            "RedlineReachedContainer" => LedContainer::RedlineReached(from_str(content)?),
            "SpeedLimiterAnimationContainer" => {
                LedContainer::SpeedLimiterAnimation(from_str(content)?)
            }
            "YellowFlagContainer" => LedContainer::YellowFlag(from_str(content)?),
            "BlueFlagContainer" => LedContainer::BlueFlag(from_str(content)?),
            "WhiteFlagContainer" => LedContainer::WhiteFlag(from_str(content)?),
            "GroupContainer" => LedContainer::Group(GroupContainer::Simple(from_str(content)?)),
            "GameRunningGroupContainer" => {
                LedContainer::Group(GroupContainer::GameRunning(from_str(content)?))
            }
            // Yes, this is a typo, we need to support it.
            "GameCarStatedGroupContainer" => {
                LedContainer::Group(GroupContainer::CarStarted(from_str(content)?))
            }
            "CustomConditionalGroupContainer" => {
                LedContainer::Group(GroupContainer::Conditional(from_str(content)?))
            }

            t => LedContainer::Unknown {
                start_position: helper.start_position,
                container_type: t.to_string(),
                content: json,
            },
        })
    }
}
