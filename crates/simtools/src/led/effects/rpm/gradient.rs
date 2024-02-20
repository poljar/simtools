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

use std::{num::NonZeroUsize, time::Instant};

use colorgrad::{CustomGradient, Gradient};
use simetry::Moment;
use uom::si::{f64::AngularVelocity, ratio::ratio};

use crate::led::{
    effects::{BlinkState, LedConfiguration, LedEffect, LedGroup, MomentExt},
    profiles::rpm::RpmContainer,
};

// TODO: Support LED dimming, aka the [`RpmContainer::use_led_dimming`] setting.

#[derive(Debug)]
pub struct RpmGradientEffect {
    container: RpmContainer,
    gradient: Gradient,
    state: LedGroup,
    blink_state: BlinkState,
}

impl RpmGradientEffect {
    pub fn with_start_position(container: RpmContainer, start_position: NonZeroUsize) -> Self {
        let led_count = container.led_count.get();

        let gradient = CustomGradient::new()
            .colors(&[container.start_color.clone(), container.end_color.clone()])
            .domain(&[0.0, (led_count - 1) as f64])
            .build()
            .expect(
                "We should always be able to create a gradient from two parsed Color \
                 types and a domain that's guaranteed to be at lest 0 -> 0",
            );

        Self {
            state: LedGroup::new(start_position, container.led_count),
            gradient,
            blink_state: Default::default(),
            container,
        }
    }

    #[cfg(test)]
    pub fn new(container: RpmContainer) -> Self {
        let start_position = container.start_position;
        Self::with_start_position(container, start_position)
    }

    fn calculate_how_many_leds_to_turn_on(
        &self,
        rpm: AngularVelocity,
        max_rpm: AngularVelocity,
    ) -> usize {
        let led_count = self.state.leds.len();

        let percentage_of_leds_to_turn_on = if self.container.use_percent {
            let rpm_percentage = rpm / max_rpm * 100.0;

            let percentage_min = self.container.percent_min;
            let percentage_max = self.container.percent_max;

            (rpm_percentage - percentage_min) / (percentage_max - percentage_min)
        } else {
            let rpm_min = self.container.rpm_min;
            let rpm_max = self.container.rpm_max;

            (rpm - rpm_min) / (rpm_max - rpm_min)
        };

        (percentage_of_leds_to_turn_on * led_count as f64).floor::<ratio>().get::<ratio>() as usize
    }

    fn calculate_next_blink_state(&self, sim_state: &dyn Moment) -> BlinkState {
        let redline_reached = sim_state.redline_reached();
        let blink_enabled = self.container.blink_enabled;

        let blink = if self.container.blink_on_last_gear {
            true
        } else {
            // TODO: How do we figure out what max gear the car supports?
            sim_state.vehicle_gear() != Some(6)
        };

        if redline_reached && blink_enabled && blink {
            match &self.blink_state {
                BlinkState::NotBlinking => {
                    BlinkState::LedsTurnedOn { state_change: Instant::now() }
                }
                BlinkState::LedsTurnedOff { state_change } => {
                    if state_change.elapsed() >= self.container.blink_delay {
                        BlinkState::LedsTurnedOn { state_change: Instant::now() }
                    } else {
                        self.blink_state
                    }
                }
                BlinkState::LedsTurnedOn { state_change } => {
                    if state_change.elapsed() >= self.container.blink_delay {
                        BlinkState::LedsTurnedOff { state_change: Instant::now() }
                    } else {
                        self.blink_state
                    }
                }
            }
        } else {
            BlinkState::NotBlinking
        }
    }

    pub fn update(&mut self, sim_state: &dyn Moment) {
        let Some(rpm) = sim_state.vehicle_engine_rotation_speed() else {
            return;
        };
        let Some(max_rpm) = sim_state.vehicle_max_engine_rotation_speed() else {
            return;
        };

        let next_blink_state = self.calculate_next_blink_state(sim_state);
        let leds_to_turn_on = self.calculate_how_many_leds_to_turn_on(rpm, max_rpm);

        let led_iterator: Box<dyn Iterator<Item = &mut LedConfiguration>> =
            if self.container.right_to_left {
                Box::new(self.state.leds.iter_mut().rev())
            } else {
                Box::new(self.state.leds.iter_mut())
            };

        for (led_number, led) in led_iterator.enumerate() {
            // If we're using the [`RpmContainer::gradient_on_all`] setting, we're going to
            // pick the color of the active LED that is rightmost on the
            // gradient for all LEDs, otherwise, each LED will get their color
            // from the position on the gradient.
            let gradient_position =
                if self.container.gradient_on_all { leds_to_turn_on } else { led_number };

            let enabled = match next_blink_state {
                BlinkState::NotBlinking => {
                    // If the [`RpmContainer::gradient_on_all`] and [`RpmContainer::fill_all_leds`]
                    // settings are on, then all LEDs will be turned on and only the color of the
                    // LEDs will change. Otherwise, only the LEDs that match a certain RPM value
                    // will be turned on.
                    if self.container.gradient_on_all && self.container.fill_all_leds {
                        true
                    } else {
                        led_number < leds_to_turn_on
                    }
                }
                BlinkState::LedsTurnedOff { .. } => false,
                BlinkState::LedsTurnedOn { .. } => true,
            };

            *led = if enabled {
                let color = self.gradient.at(gradient_position as f64);
                LedConfiguration::On { color }
            } else {
                LedConfiguration::Off
            };
        }

        self.blink_state = next_blink_state;
    }
}

impl LedEffect for RpmGradientEffect {
    fn update(&mut self, sim_state: &dyn Moment) {
        self.update(sim_state)
    }

    fn start_led(&self) -> NonZeroUsize {
        self.state.start_position()
    }

    fn description(&self) -> &str {
        &self.container.description
    }

    fn leds(&self) -> Box<dyn Iterator<Item = &LedGroup> + '_> {
        Box::new(std::iter::once(&self.state))
    }

    fn disable(&mut self) {
        self.blink_state = BlinkState::NotBlinking;

        for led in &mut self.state.leds {
            *led = LedConfiguration::Off;
        }
    }

    fn led_count(&self) -> usize {
        self.state.leds.len()
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;
    use similar_asserts::assert_eq;
    use uom::si::{angular_velocity::revolution_per_minute, f64::AngularVelocity};

    use super::*;
    use crate::leds;

    struct RpmSimState {
        rpm: AngularVelocity,
        max_rpm: AngularVelocity,
    }

    impl RpmSimState {
        fn new(rpm: f64, rpm_max: f64) -> Self {
            Self {
                rpm: AngularVelocity::new::<revolution_per_minute>(rpm),
                max_rpm: AngularVelocity::new::<revolution_per_minute>(rpm_max),
            }
        }

        fn update_rpm(&mut self, rpm: f64) {
            self.rpm = AngularVelocity::new::<revolution_per_minute>(rpm);
        }
    }

    impl Moment for RpmSimState {
        fn vehicle_engine_rotation_speed(&self) -> Option<AngularVelocity> {
            Some(self.rpm)
        }

        fn vehicle_max_engine_rotation_speed(&self) -> Option<AngularVelocity> {
            Some(self.max_rpm)
        }
    }

    fn container() -> RpmContainer {
        let container = json!({
            "UsePercent": true,
            "PercentMin": 85.0,
            "PercentMax": 95.0,
            "RPMMin": 1000.0,
            "RPMMax": 8000.0,
            "BlinkDelay": 200,
            "StartColor": "Lime",
            "EndColor": "Red",
            "GradientOnAll": false,
            "RightToLeft": false,
            "LedCount": 5,
            "BlinkEnabled": false,
            "BlinkOnLastGear": false,
            "UseLedDimming": false,
            "FillAllLeds": false,
            "StartPosition": 1,
            "ContainerId": "27b0421e-f669-4af6-beba-a90c5aba49a9",
            "ContainerType": "RPMContainer",
            "Description": "Turn on LEDs based on the RPM and pick a color on a gradient",
            "IsEnabled": true
        });

        serde_json::from_value(container)
            .expect("We should be able to deserialize the default RPM container")
    }

    #[test]
    fn rpm_percentage() {
        const MAX_RPM: f64 = 9000.0;
        let container = container();
        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);
        let mut rpm_led_state = RpmGradientEffect::new(container);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "The initial state should be fully disabled LEDs with default colors"
        );

        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "After the first Sim state update, the LEDs should still be turned off."
        );

        sim_state.update_rpm(MAX_RPM * 0.85);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "Updating the current RPM to 7650.0, or 85% of the MAX RPM, should not turn any LEDs \
             on, we only start turning things on *after* 85% of the MAX RPM"
        );

        sim_state.update_rpm(MAX_RPM * 0.87);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["lime", off, off, off, off],
            &rpm_led_state.state,
            "Setting the RPM to 87% of the MAX RPM, should turn the first LED on",
        );

        sim_state.update_rpm(MAX_RPM * 0.90);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["lime", (0.25, 0.75, 0.0), off, off, off],
            &rpm_led_state.state,
            "Getting to 0.9 of the MAX RPM should turn on another LED",
        );

        sim_state.update_rpm(MAX_RPM * 0.95);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["lime", (0.25, 0.75, 0.0), (0.5, 0.5, 0.0), (0.75, 0.25, 0.0), "red"],
            &rpm_led_state.state,
            "Getting to 0.95 of the MAX RPM should turn on all LEDs",
        );

        sim_state.update_rpm(MAX_RPM * 0.10);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "Going back to 0.1 of the max RPM should turn the LEDs back off",
        );
    }

    #[test]
    fn rpm_values() {
        const MAX_RPM: f64 = 9000.0;
        let mut container = container();
        container.use_percent = false;

        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);
        let mut rpm_led_state = RpmGradientEffect::new(container);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "The initial state should be fully disabled LEDs with default colors"
        );

        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "After the first Sim state update, the LEDs should still be off",
        );

        sim_state.update_rpm(1000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "Updating the current RPM to 1000, or to the of the MIN RPM setting, should not turn \
             any LEDs on, we only start turning things on *after* the MIN RPM setting"
        );

        sim_state.update_rpm(2400.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["lime", off, off, off, off],
            &rpm_led_state.state,
            "Setting the RPM to 2400 RPM, should turn the first LED on",
        );

        sim_state.update_rpm(3850.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["lime", (0.25, 0.75, 0.0), off, off, off],
            &rpm_led_state.state,
            "Getting to 3850 RPM should turn on another LED",
        );

        sim_state.update_rpm(8000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["lime", (0.25, 0.75, 0.0), (0.5, 0.5, 0.0), (0.75, 0.25, 0.0), "red"],
            &rpm_led_state.state,
            "Getting to 8000 RPM should turn on all LEDs",
        );

        sim_state.update_rpm(1000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "Going back to 1000 RPM should turn the LEDs back off",
        );
    }

    #[test]
    fn rpm_gradient_on_all() {
        const MAX_RPM: f64 = 9000.0;
        let mut container = container();
        container.use_percent = false;
        container.gradient_on_all = true;

        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);
        let mut rpm_led_state = RpmGradientEffect::new(container);

        sim_state.update_rpm(3850.0);
        rpm_led_state.update(&sim_state);

        // The [`gradient_on_all`] setting ensures that all enabled LEDs have the same
        // color.
        assert_eq!(
            &leds![(0.5, 0.5, 0.0), (0.5, 0.5, 0.0), off, off, off],
            &rpm_led_state.state,
            "Setting the RPM to 3850.0 should turn on two LEDs and they both should have a yellow \
             color",
        );

        sim_state.update_rpm(8000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["red"; 5],
            &rpm_led_state.state,
            "Setting the RPM to 8000.0 should turn on all LEDs and have all of them be red",
        );
    }

    #[test]
    fn rpm_gradient_on_all_and_fill_all_leds() {
        const MAX_RPM: f64 = 9000.0;
        let mut container = container();
        container.use_percent = false;
        container.gradient_on_all = true;
        container.fill_all_leds = true;

        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);
        let mut rpm_led_state = RpmGradientEffect::new(container);

        sim_state.update_rpm(3850.0);
        rpm_led_state.update(&sim_state);

        // The `gradient_on_all` setting ensures that all LEDs have the same color and
        // the `fill_all_leds` setting enables them all.
        assert_eq!(
            &leds![(0.5, 0.5, 0.0); 5],
            &rpm_led_state.state,
            "Setting the RPM to 3850.0 should turn on all LEDs and they should have a yellow color",
        );

        sim_state.update_rpm(8000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["red"; 5],
            &rpm_led_state.state,
            "Setting the RPM to 8000.0 should set the collor on all LEDs to red",
        );
    }

    #[test]
    fn blinking() {
        const MAX_RPM: f64 = 9000.0;
        let mut container = container();
        container.blink_enabled = true;
        container.gradient_on_all = true;

        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);
        let mut rpm_led_state = RpmGradientEffect::new(container);

        sim_state.update_rpm(0.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(&leds![off; 5], &rpm_led_state.state, "The LEDs should initially be off",);

        sim_state.update_rpm(MAX_RPM);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["red"; 5],
            &rpm_led_state.state,
            "Setting the RPM to the MAX RPM should set the collor on all LEDs to red",
        );

        std::thread::sleep(rpm_led_state.container.blink_delay);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "The LEDs should be turned off after the blink delay has passed",
        );

        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off; 5],
            &rpm_led_state.state,
            "The state of the LEDs should not change unless the blink delay has expired"
        );

        std::thread::sleep(rpm_led_state.container.blink_delay);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds!["red"; 5],
            &rpm_led_state.state,
            "The LEDs should be turned on again after the blink delay has passed"
        );
    }

    #[test]
    fn reverse() {
        const MAX_RPM: f64 = 9000.0;
        let mut container = container();
        container.right_to_left = true;

        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);
        let mut rpm_led_state = RpmGradientEffect::new(container);

        sim_state.update_rpm(MAX_RPM * 0.87);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off, off, off, off, "lime"],
            &rpm_led_state.state,
            "Setting the RPM to 87% of the MAX RPM, should turn the first LED on, the most right one",
        );

        sim_state.update_rpm(MAX_RPM * 0.90);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            &leds![off, off, off, (0.25, 0.75, 0.0), "lime"],
            &rpm_led_state.state,
            "Getting to 0.9 of the MAX RPM should turn on another LED, from the right side",
        );
    }
}
