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

use std::{fmt::Debug, num::NonZeroUsize, time::Instant};

use csscolorparser::Color;
use simetry::Moment;

pub mod blink;
pub mod groups;
pub mod rpm;

pub trait LedEffect: Debug {
    /// Get an iterator over the LEDs this effect controls.
    fn leds(&self) -> Box<dyn Iterator<Item = &LedGroup> + '_>;

    /// Update the state of the effect with the latest [`Moment`] in the
    /// simulator.
    fn update(&mut self, sim_state: &dyn Moment);

    /// Disable all the LEDs this effect controls.
    fn disable(&mut self);

    /// The start LED of this [`LedEffect`], this is the position of the LED
    /// where the first LED in the [`LedEffect::leds()`] iterator should be
    /// applied to on the device.
    fn start_led(&self) -> NonZeroUsize;

    /// The description of this [`LedEffect`].
    fn description(&self) -> &str;

    /// The number of LEDs this [`LedEffect`] controls.
    fn led_count(&self) -> usize {
        self.leds().map(|led_state| led_state.leds().len()).sum()
    }
}

/// Common state for LED effects which support blinking LEDs.
#[derive(Debug, Default, Clone, Copy)]
pub enum BlinkState {
    /// The LEDs are not blinking currently.
    #[default]
    NotBlinking,
    /// The LEDs are currently blinking and are in the off state.
    LedsTurnedOff {
        /// The instant in time when the LEDs turned off.
        state_change: Instant,
    },
    /// The LEDs are currently blinking and are in the on state.
    LedsTurnedOn {
        /// The instant in time when the LEDs turned on.
        state_change: Instant,
    },
}

/// A group of LEDs an [`LedEffect`] produces.
///
/// This struct contains a collection of [`LedConfiguration`] a RGB LED device
/// should apply to its LEDs.
#[derive(Debug, Clone, PartialEq)]
pub struct LedGroup {
    start_position: NonZeroUsize,
    leds: Vec<LedConfiguration>,
}

impl LedGroup {
    /// Create a new [`LedGroup`] group with the given start position and LED
    /// count.
    pub fn new(start_position: NonZeroUsize, led_count: NonZeroUsize) -> Self {
        Self { start_position, leds: vec![LedConfiguration::default(); led_count.get()] }
    }

    /// Create a new [`LedGroup`] group with the given start position and LED
    /// count, each LED will be enabled and configured to display the given
    /// [`Color`].
    pub fn with_color(color: Color, start_position: NonZeroUsize, led_count: NonZeroUsize) -> Self {
        Self { start_position, leds: vec![LedConfiguration::On { color }; led_count.get()] }
    }

    /// Get the start position of the first LED for this LED group.
    ///
    /// The RGB LED device might contain more LEDs than this group contains and
    /// the group might not map to the first LED the device has.
    pub fn start_position(&self) -> NonZeroUsize {
        self.start_position
    }

    /// Get the list of [`LedConfiguration`]s this LED group contains.
    pub fn leds(&self) -> &[LedConfiguration] {
        &self.leds
    }
}

/// Configuration for a single LED.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum LedConfiguration {
    /// The LED is currently turned on.
    On {
        /// The color the RGB LED should display.
        color: Color,
    },
    /// The LED is currently turned off.
    #[default]
    Off,
}

#[macro_export]
macro_rules! leds {
    ($start_position:expr; $color:tt; $n:expr) => {
        $crate::led::effects::LedGroup {
            start_position: ::std::num::NonZeroUsize::new($start_position).expect("Invalid start position, must be non-zero"),
            leds: vec![leds!(@led $color); $n],
        }
    };

    ($color:tt; $n:expr) => {
        leds![1; $color; $n]
    };

    ($start_position:expr; $($color:tt),+ $(,)?) => {{
        let leds = vec![
            $(leds!(@led $color)),+
        ];

        LedGroup {
            start_position: ::std::num::NonZeroUsize::new($start_position).expect("Invalid start position, must be non-zero"),
            leds
        }
    }};

    ($($color:tt),+ $(,)?) => {{
        leds![1; $($color),+]
    }};

    (@led off) => {
        $crate::led::effects::LedConfiguration::Off
    };
    (@led ($r:expr, $g:expr, $b:expr)) => {
        $crate::led::effects::LedConfiguration::On {
            color: ::csscolorparser::Color::new($r, $g, $b, 1.0),
        }
    };
    (@led $color:expr) => {
        $crate::led::effects::LedConfiguration::On {
            color: ::csscolorparser::Color::from_html($color).unwrap(),
        }
    };
}
