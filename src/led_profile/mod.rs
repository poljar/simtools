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
    pub automatic_switch: bool,
    /// A list of [`LedContainer`] values which configure a set of LEDs.
    pub led_containers: Vec<LedContainer>,
}

/// The [`LedContainer`] contains a single configuration for the behavior of a set of LED lights.
///
/// There are different container types, each of them might react to different inputs, i.e. there
/// are containers that react to the RPM of the engine, to flags being waved on the track, and
/// other various track and car conditions being met.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "ContainerType", rename_all = "PascalCase")]
pub enum LedContainer {
    #[serde(rename = "RPMContainer")]
    RPMContainer(RpmContainer),
    #[serde(rename = "RPMSegmentsContainer")]
    RpmSegmentsContainer(RpmSegmentsContainer),
    RedlineReachedContainer(RedlineReachedContainer),
    SpeedLimiterAnimationContainer(SpeedLimiterAnimationContainer),
    GroupContainer(GroupContainer),
    BlueFlagContainer(FlagContainer),
    WhiteFlagContainer(FlagContainer),
    YellowFlagContainer(FlagContainer),
    #[serde(other)]
    Unknown,
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
