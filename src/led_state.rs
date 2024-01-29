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

use std::num::NonZeroUsize;

use colorgrad::CustomGradient;
use csscolorparser::Color;

use simetry::{assetto_corsa_competizione::SimState, Moment};
use uom::si::ratio::ratio;

use crate::led_profile::rpm::RpmContainer;

pub struct RpmLedState {
    container: RpmContainer,
    state: LedState,
}

impl RpmLedState {
    pub fn new(container: RpmContainer) -> Self {
        Self {
            state: LedState::new(container.start_position, container.led_count),
            container,
        }
    }

    pub fn update(&mut self, sim_state: &SimState) {
        let Some(rpm) = sim_state.vehicle_engine_rotation_speed() else {
            return;
        };
        let Some(max_rpm) = sim_state.vehicle_max_engine_rotation_speed() else {
            return;
        };

        let led_count = self.state.leds.len();

        let gradient = CustomGradient::new()
            .colors(&[
                self.container.start_color.clone(),
                self.container.end_color.clone(),
            ])
            .domain(&[0.0, (led_count - 1) as f64])
            .build()
            .unwrap();

        let (percentage_of_leds_to_turn_on, _should_blink) = if self.container.use_percent {
            let rpm_percentage = rpm / max_rpm * 100.0;

            let percentage_min = self.container.percent_min;
            let percentage_max = self.container.percent_max;

            let should_blink = rpm_percentage >= percentage_max;

            let leds_to_turn_on =
                (rpm_percentage - percentage_min) / (percentage_max - percentage_min);

            (leds_to_turn_on, should_blink)
        } else {
            let rpm_min = self.container.rpm_min;
            let rpm_max = self.container.rpm_max;

            let should_blink = rpm >= rpm_max;

            let leds_to_turn_on = (rpm - rpm_min) / (rpm_max - rpm_min);

            (leds_to_turn_on, should_blink)
        };

        let leds_to_turn_on = (percentage_of_leds_to_turn_on * led_count as f64)
            .floor::<ratio>()
            .get::<ratio>() as usize;

        let led_iterator: Box<dyn Iterator<Item = &mut LedConfiguration>> =
            if self.container.right_to_left {
                Box::new(self.state.leds.iter_mut().rev())
            } else {
                Box::new(self.state.leds.iter_mut())
            };

        for (led_number, led) in led_iterator.enumerate() {
            let color = gradient.at(led_number as f64);

            led.color = color;
            led.enabled = led_number < leds_to_turn_on;
        }
    }

    pub fn state(&self) -> &LedState {
        &self.state
    }

    pub fn container(&self) -> &RpmContainer {
        &self.container
    }
}

#[derive(Debug, Clone)]
pub struct LedState {
    pub start_led: usize,
    pub leds: Vec<LedConfiguration>,
}

impl LedState {
    pub fn new(start_led: usize, led_count: NonZeroUsize) -> Self {
        Self {
            start_led,
            leds: vec![LedConfiguration::default(); led_count.get()],
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct LedConfiguration {
    pub enabled: bool,
    pub color: Color,
}
