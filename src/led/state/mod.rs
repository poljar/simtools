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

use csscolorparser::Color;
use std::{fmt::Debug, num::NonZeroUsize, time::Instant};

use simetry::Moment;

pub mod flag;
pub mod groups;
pub mod rpm;

pub trait LedEffect: Debug {
    fn leds(&self) -> Box<dyn Iterator<Item = &LedState> + '_>;
    fn update(&mut self, sim_state: &dyn Moment);
    fn disable(&mut self);
    fn start_led(&self) -> NonZeroUsize;
    fn description(&self) -> &str;

    fn led_count(&self) -> usize {
        self.leds().map(|led_state| led_state.leds().len()).sum()
    }
}

pub trait MomentExt: Moment {
    fn redline_reached(&self) -> bool {
        const ERROR_MARGIN_PERCENTAGE: f64 = 0.02;

        let Some(rpm) = self.vehicle_engine_rotation_speed() else {
            return false;
        };

        let Some(max_rpm) = self.vehicle_max_engine_rotation_speed() else {
            return false;
        };

        let error_margin = ERROR_MARGIN_PERCENTAGE * max_rpm;

        // If we're within 2% of the MAX RPM of a car, we're going to consider this to be at
        // the redline.
        (max_rpm - rpm).abs() < error_margin
    }

    fn is_engine_running(&self) -> bool {
        let Some(is_starting) = self.is_starter_on() else {
            return false;
        };

        let Some(rpm) = self.vehicle_engine_rotation_speed() else {
            return false;
        };

        !is_starting && rpm.value > 0.0
    }
}

impl<T> MomentExt for T where T: Moment + ?Sized {}

#[derive(Debug, Default, Clone, Copy)]
pub enum BlinkState {
    #[default]
    NotBlinking,
    LedsTurnedOff {
        state_change: Instant,
    },
    LedsTurnedOn {
        state_change: Instant,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LedState {
    start_position: NonZeroUsize,
    leds: Vec<LedConfiguration>,
}

impl LedState {
    pub fn new(start_position: NonZeroUsize, led_count: NonZeroUsize) -> Self {
        Self {
            start_position,
            leds: vec![LedConfiguration::default(); led_count.get()],
        }
    }

    pub fn with_color(color: Color, start_position: NonZeroUsize, led_count: NonZeroUsize) -> Self {
        Self {
            start_position,
            leds: vec![LedConfiguration::On { color }; led_count.get()],
        }
    }

    pub fn start_position(&self) -> NonZeroUsize {
        self.start_position
    }

    pub fn leds(&self) -> &[LedConfiguration] {
        &self.leds
    }
}

// TODO: This should be an enum with On/Off variants.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum LedConfiguration {
    On {
        color: Color,
    },
    #[default]
    Off,
}

#[cfg(test)]
mod test {
    #[macro_export]
    macro_rules! led {
        (off) => {
            $crate::led::state::LedConfiguration::Off
        };
        (($r:expr, $g:expr, $b:expr)) => {
            $crate::led::state::LedConfiguration::On {
                color: ::csscolorparser::Color::new($r, $g, $b, 1.0),
            }
        };
        ($color:expr) => {
            $crate::led::state::LedConfiguration::On {
                color: ::csscolorparser::Color::from_html($color).unwrap(),
            }
        };
    }

    #[macro_export]
    macro_rules! leds {
        ($start_position:expr; $color:tt; $n:expr) => {
            $crate::led::state::LedState {
                start_position: ::std::num::NonZeroUsize::new($start_position).expect("Invalid start position, must be non-zero"),
                leds: vec![$crate::led!($color); $n],
            }
        };

        ($color:tt; $n:expr) => {
            leds![1; $color; $n]
        };

        ($start_position:expr; $($color:tt),+ $(,)?) => {{
            let leds = vec![
                $($crate::led!($color)),+
            ];

            LedState {
                start_position: ::std::num::NonZeroUsize::new($start_position).expect("Invalid start position, must be non-zero"),
                leds
            }
        }};

        ($($color:tt),+ $(,)?) => {{
            leds![1; $($color),+]
        }};
    }
}
