// Copyright (c) 2024 Damir Jelić
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{num::NonZeroUsize, time::Duration};

use csscolorparser::Color;
use serde::{Deserialize, Deserializer};
use uom::si::{
    angular_velocity::revolution_per_minute,
    f64::{AngularVelocity, Ratio},
};

use super::{color_from_str, default_non_zero, duration_from_int_ms};

/// The configuration for a LED profile container which turns on LEDs based on
/// the value of the RPM of the engine.
///
/// As the RPM increases more LEDs will be turned on, the color of the LEDs will
/// be configured to follow a color gradient beginning with the
/// [`RpmContainer::start_color`] and ending in [`RpmContainer::end_color`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RpmContainer {
    /// The human readable description of the [`RpmContainer`].
    #[serde(default)]
    pub description: String,
    /// Is this container enabled.
    pub is_enabled: bool,
    /// The number of the first LED this container should control.
    #[serde(default = "default_non_zero")]
    pub start_position: NonZeroUsize,
    /// The total number of LEDs this container should control.
    pub led_count: NonZeroUsize,
    /// Should we use the specified percentages to calculate how many LEDs need
    /// to be turned on instead of the raw [`RpmContainer::rpm_min`] and
    /// [`RpmContainer::rpm_max`] values?
    #[serde(default)]
    pub use_percent: bool,
    /// The percentage of the RPM that should start turning LEDs on.
    pub percent_min: Ratio,
    /// The percentage of the RPM which should be considered the maximum RPM, or
    /// rather when the gradient should reach its end and all the LEDs
    /// should be turned on.
    pub percent_max: Ratio,
    /// The value of the RPM that should start turning LEDs on.
    #[serde(rename = "RPMMin")]
    #[serde(deserialize_with = "rpm_from_float")]
    pub rpm_min: AngularVelocity,
    /// The value of the RPM which should be considered the maximum RPM, or
    /// rather when the gradient should reach its end and all the LEDs
    /// should be turned on.
    #[serde(rename = "RPMMax")]
    #[serde(deserialize_with = "rpm_from_float")]
    pub rpm_max: AngularVelocity,
    /// The first color in the gradient, the gradient will begin with this color
    /// and transition towards the [`RpmContainer::end_color`].
    #[serde(deserialize_with = "color_from_str")]
    pub start_color: Color,
    /// The final color in the gradient.
    #[serde(deserialize_with = "color_from_str")]
    pub end_color: Color,
    /// Should the LEDs be filled out from right to left instead of the usual
    /// left to right direction?
    #[serde(default)]
    pub right_to_left: bool,
    /// Should the LEDs blink when the maximum RPM of the car is reached, the so
    /// called redline. This is not the [`RpmContainer::rpm_max`] setting,
    /// the maximum RPM of the car is defined by the simulator.
    #[serde(default)]
    pub blink_enabled: bool,
    /// How long should the LED stay on and off when blinking, in other words
    /// how long do we wait before we change the state of the LED.
    #[serde(deserialize_with = "duration_from_int_ms")]
    pub blink_delay: Duration,
    /// Should the LEDs also blink when the maximum RPM is reached in the last
    /// gear?
    #[serde(default)]
    pub blink_on_last_gear: bool,
    /// TODO: What does this setting do?
    #[serde(default)]
    pub use_led_dimming: bool,
    /// Should the same color, the one that is furthest on the gradient and
    /// enabled because of the RPM value, be used for all the currently
    /// active LEDs?
    #[serde(default)]
    pub gradient_on_all: bool,
    /// Should all the LEDs be turned on from the start, only the colors will
    /// differ based on the RPM. This only works in conjunction with the
    /// [`RpmContainer::gradient_on_all`] setting.
    #[serde(default)]
    pub fill_all_leds: bool,
}

/// The configuration for a LED profile container which turns on segments of
/// LEDs based on the value of the RPM of the engine.
///
/// This container will divide a larger number of LEDs into smaller subsets or
/// segments. Each segment can have a different configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RpmSegmentsContainer {
    /// The human readable description of the [`RpmContainer`].
    #[serde(default)]
    pub description: String,
    /// Is this container enabled.
    pub is_enabled: bool,
    /// The number of the first LED this container should control.
    #[serde(default = "default_non_zero")]
    pub start_position: NonZeroUsize,
    /// The number of segments this container has. This value isn't particularly
    /// useful since it's better to take a look at
    /// [`RPMSegmentsContainer::segments::len()`]
    pub segments_count: u32,
    /// Should the LEDs blink when TODO: When do we blink here exactly?
    #[serde(default)]
    pub blink_enabled: bool,
    /// How long should the LED stay on and off when blinking, in other words
    /// how long do we wait before we change the state of the LED.
    #[serde(default, deserialize_with = "duration_from_int_ms")]
    pub blink_delay: Duration,
    /// Should the LEDs only (or as well?) blink when the maximum RPM or
    /// percentage of it are reached in the last gear?
    #[serde(default)]
    pub blink_on_last_gear: bool,
    /// The list of LED segments.
    pub segments: Vec<LedSegment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LedSegment {
    pub start_value: Ratio,
    pub end_value: Ratio,
    #[serde(deserialize_with = "color_from_str")]
    pub normal_color: Color,
    #[serde(deserialize_with = "color_from_str")]
    pub blinking_color: Color,
    pub use_blinking_color: bool,
    pub led_count: NonZeroUsize,
    pub sample_result: SampleResult,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SampleResult {
    pub width: u32,
    pub position: u32,
    pub columns: i32,
}

/// Helper to deserialize a float containing a RPM value into a
/// [`AngularVelocity`] type.
pub fn rpm_from_float<'de, D>(deserializer: D) -> Result<AngularVelocity, D::Error>
where
    D: Deserializer<'de>,
{
    f64::deserialize(deserializer).map(AngularVelocity::new::<revolution_per_minute>)
}
