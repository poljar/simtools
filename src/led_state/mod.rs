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
use std::{num::NonZeroUsize, time::Instant};

use simetry::Moment;

pub mod rpm_gradient;

pub trait LedFoo {
    fn state(&self) -> &LedState;
    fn update(&mut self, sim_state: &dyn Moment);
    fn start_led(&self) -> usize;
    fn description(&self) -> &str;
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
    pub leds: Vec<LedConfiguration>,
}

impl LedState {
    pub fn new(led_count: NonZeroUsize) -> Self {
        Self {
            leds: vec![LedConfiguration::default(); led_count.get()],
        }
    }

    pub fn with_color(led_count: NonZeroUsize, color: Color) -> Self {
        Self {
            leds: vec![
                LedConfiguration {
                    enabled: false,
                    color
                };
                led_count.get()
            ],
        }
    }
}

// TODO: This should be an enum with On/Off variants.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LedConfiguration {
    pub enabled: bool,
    pub color: Color,
}
