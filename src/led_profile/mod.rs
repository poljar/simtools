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

use std::time::Duration;

use csscolorparser::Color;
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use uuid::Uuid;

use self::{
    flag::FlagContainer,
    redline::RedlineReachedContainer,
    rpm::{RpmContainer, RpmSegmentsContainer},
    speed_limiter::SpeedLimiterAnimationContainer,
};

pub mod flag;
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
    RpmContainer(RpmContainer),
    RpmSegmentsContainer(RpmSegmentsContainer),
    RedlineReachedContainer(RedlineReachedContainer),
    SpeedLimiterAnimationContainer(SpeedLimiterAnimationContainer),
    GroupContainer(GroupContainer),
    BlueFlagContainer(FlagContainer),
    WhiteFlagContainer(FlagContainer),
    YellowFlagContainer(FlagContainer),
    Unknown {
        container_type: String,
        content: Box<RawValue>,
    },
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
            "RPMContainer" => LedContainer::RpmContainer(from_str(content)?),
            "RPMSegmentsContainer" => LedContainer::RpmSegmentsContainer(from_str(content)?),
            "RedlineReachedContainer" => LedContainer::RedlineReachedContainer(from_str(content)?),
            "SpeedLimiterAnimationContainer" => {
                LedContainer::SpeedLimiterAnimationContainer(from_str(content)?)
            }
            "YellowFlagContainer" => LedContainer::YellowFlagContainer(from_str(content)?),
            "BlueFlagContainer" => LedContainer::BlueFlagContainer(from_str(content)?),
            "WhiteFlagContainer" => LedContainer::WhiteFlagContainer(from_str(content)?),
            "GroupContainer" => LedContainer::GroupContainer(from_str(content)?),
            t => LedContainer::Unknown {
                container_type: t.to_string(),
                content: json,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GroupContainer {
    pub description: String,
    pub container_id: Uuid,
    pub is_enabled: bool,
    pub stack_left_to_right: bool,
    pub start_position: u32,
    pub led_containers: Vec<LedContainer>,
}

/// Helper to deserialize a integer containing milliseconds into a [`Duration`].
pub fn duration_from_int_ms<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Duration::from_millis)
}

/// Helper to deserialize a string containing a HTML color into a [`Color`].
pub fn color_from_str<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)
        .and_then(|color| Color::from_html(color).map_err(serde::de::Error::custom))
}
