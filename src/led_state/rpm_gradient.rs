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

use colorgrad::{CustomGradient, Gradient};
use simetry::Moment;
use std::time::Instant;
use uom::si::{f64::AngularVelocity, ratio::ratio};

use super::{BlinkState, LedConfiguration, LedState, MomentExt};
use crate::led_profile::rpm::RpmContainer;

// TODO: Support LED dimming, aka the [`RpmContainer::use_led_dimming`] setting.

#[derive(Debug)]
pub struct RpmLedState {
    container: RpmContainer,
    gradient: Gradient,
    state: LedState,
    blink_state: BlinkState,
}

impl RpmLedState {
    pub fn new(container: RpmContainer) -> Self {
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
            state: LedState::new(container.led_count),
            gradient,
            blink_state: Default::default(),
            container,
        }
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

        (percentage_of_leds_to_turn_on * led_count as f64)
            .floor::<ratio>()
            .get::<ratio>() as usize
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
                BlinkState::NotBlinking => BlinkState::LedsTurnedOn {
                    state_change: Instant::now(),
                },
                BlinkState::LedsTurnedOff { state_change } => {
                    if state_change.elapsed() >= self.container.blink_delay {
                        BlinkState::LedsTurnedOn {
                            state_change: Instant::now(),
                        }
                    } else {
                        self.blink_state
                    }
                }
                BlinkState::LedsTurnedOn { state_change } => {
                    if state_change.elapsed() >= self.container.blink_delay {
                        BlinkState::LedsTurnedOff {
                            state_change: Instant::now(),
                        }
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
            // If we're using the [`RpmContainer::gradient_on_all`] setting, we're going to pick
            // the color of the active LED that is rightmost on the gradient for all LEDs,
            // otherwise, each LED will get their color from the position on the gradient.
            let gradient_position = if self.container.gradient_on_all {
                leds_to_turn_on
            } else {
                led_number
            };

            led.color = self.gradient.at(gradient_position as f64);

            led.enabled = match next_blink_state {
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
            }
        }

        self.blink_state = next_blink_state;
    }

    pub fn state(&self) -> &LedState {
        &self.state
    }

    pub fn container(&self) -> &RpmContainer {
        &self.container
    }
}

#[cfg(test)]
mod test {
    use csscolorparser::Color;
    use serde_json::json;
    use uom::si::{angular_velocity::revolution_per_minute, f64::AngularVelocity};

    use super::*;

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
        let mut rpm_led_state = RpmLedState::new(container);

        let mut expected_led_configs = vec![LedConfiguration::default(); 5];

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "The initial state should be fully disabled LEDs with default colors"
        );

        rpm_led_state.update(&sim_state);

        expected_led_configs[0].color = Color::new(0.0, 1.0, 0.0, 1.0);
        expected_led_configs[1].color = Color::new(0.25, 0.75, 0.0, 1.0);
        expected_led_configs[2].color = Color::new(0.5, 0.5, 0.0, 1.0);
        expected_led_configs[3].color = Color::new(0.75, 0.25, 0.0, 1.0);
        expected_led_configs[4].color = Color::new(1.0, 0.0, 0.0, 1.0);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "After the first Sim state update, the LEDs should be properly colorized in a \
             gradient but they should still be turned off."
        );

        sim_state.update_rpm(MAX_RPM * 0.85);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Updating the current RPM to 7650.0, or 85% of the MAX RPM, should not turn any LEDs \
             on, we only start turning things on *after* 85% of the MAX RPM"
        );

        sim_state.update_rpm(MAX_RPM * 0.87);
        rpm_led_state.update(&sim_state);
        expected_led_configs[0].enabled = true;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Setting the RPM to 87% of the MAX RPM, should turn the first LED on",
        );

        sim_state.update_rpm(MAX_RPM * 0.90);
        rpm_led_state.update(&sim_state);
        expected_led_configs[1].enabled = true;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Getting to 0.9 of the MAX RPM should turn on another LED",
        );

        sim_state.update_rpm(MAX_RPM * 0.95);
        rpm_led_state.update(&sim_state);
        expected_led_configs[2].enabled = true;
        expected_led_configs[3].enabled = true;
        expected_led_configs[4].enabled = true;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Getting to 0.95 of the MAX RPM should turn on all LEDs",
        );

        sim_state.update_rpm(MAX_RPM * 0.10);
        rpm_led_state.update(&sim_state);
        expected_led_configs[0].enabled = false;
        expected_led_configs[1].enabled = false;
        expected_led_configs[2].enabled = false;
        expected_led_configs[3].enabled = false;
        expected_led_configs[4].enabled = false;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Going back to 0.1 of the max RPM should turn the LEDs back off",
        );
    }

    #[test]
    fn rpm_values() {
        const MAX_RPM: f64 = 9000.0;
        let mut container = container();
        container.use_percent = false;

        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);
        let mut rpm_led_state = RpmLedState::new(container);

        let mut expected_led_configs = vec![LedConfiguration::default(); 5];

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "The initial state should be fully disabled LEDs with default colors"
        );

        rpm_led_state.update(&sim_state);

        expected_led_configs[0].color = Color::new(0.0, 1.0, 0.0, 1.0);
        expected_led_configs[1].color = Color::new(0.25, 0.75, 0.0, 1.0);
        expected_led_configs[2].color = Color::new(0.5, 0.5, 0.0, 1.0);
        expected_led_configs[3].color = Color::new(0.75, 0.25, 0.0, 1.0);
        expected_led_configs[4].color = Color::new(1.0, 0.0, 0.0, 1.0);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "After the first Sim state update, the LEDs should be properly colorized in a \
             gradient but they should still be turned off."
        );

        sim_state.update_rpm(1000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Updating the current RPM to 1000, or to the of the MIN RPM setting, should not turn \
             any LEDs on, we only start turning things on *after* the MIN RPM setting"
        );

        sim_state.update_rpm(2400.0);
        rpm_led_state.update(&sim_state);
        expected_led_configs[0].enabled = true;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Setting the RPM to 2400 RPM, should turn the first LED on",
        );

        sim_state.update_rpm(3850.0);
        rpm_led_state.update(&sim_state);
        expected_led_configs[1].enabled = true;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Getting to 3850 RPM should turn on another LED",
        );

        sim_state.update_rpm(8000.0);
        rpm_led_state.update(&sim_state);
        expected_led_configs[2].enabled = true;
        expected_led_configs[3].enabled = true;
        expected_led_configs[4].enabled = true;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Getting to 8000 RPM should turn on all LEDs",
        );

        sim_state.update_rpm(1000.0);
        rpm_led_state.update(&sim_state);
        expected_led_configs[0].enabled = false;
        expected_led_configs[1].enabled = false;
        expected_led_configs[2].enabled = false;
        expected_led_configs[3].enabled = false;
        expected_led_configs[4].enabled = false;

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
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
        let mut rpm_led_state = RpmLedState::new(container);

        rpm_led_state.update(&sim_state);

        // The [`gradient_on_all`] setting ensures that all LEDs have the same color.
        let mut expected_led_configs = vec![
            LedConfiguration {
                enabled: false,
                color: Color::new(0.5, 0.5, 0.0, 1.0),
            };
            5
        ];

        expected_led_configs[0].enabled = true;
        expected_led_configs[1].enabled = true;

        sim_state.update_rpm(3850.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Setting the RPM to 3850.0 should turn on two LEDs and they both should have a yellow \
             color",
        );

        for expected_led in &mut expected_led_configs {
            expected_led.enabled = true;
            expected_led.color = Color::new(1.0, 0.0, 0.0, 1.0);
        }

        sim_state.update_rpm(8000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
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
        let mut rpm_led_state = RpmLedState::new(container);

        rpm_led_state.update(&sim_state);

        // The [`gradient_on_all`] setting ensures that all LEDs have the same color.
        let mut expected_led_configs = vec![
            LedConfiguration {
                enabled: true,
                color: Color::new(0.5, 0.5, 0.0, 1.0),
            };
            5
        ];

        sim_state.update_rpm(3850.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Setting the RPM to 3850.0 should turn on all LEDs and they should have a yellow color",
        );

        for expected_led in &mut expected_led_configs {
            expected_led.enabled = true;
            expected_led.color = Color::new(1.0, 0.0, 0.0, 1.0);
        }

        sim_state.update_rpm(8000.0);
        rpm_led_state.update(&sim_state);

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "Setting the RPM to 8000.0 should set the collor on all LEDs to red",
        );
    }
}
