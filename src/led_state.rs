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

use simetry::Moment;
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

    pub fn update(&mut self, sim_state: &dyn Moment) {
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

#[derive(Debug, Default, Clone, PartialEq)]
pub struct LedConfiguration {
    pub enabled: bool,
    pub color: Color,
}

#[cfg(test)]
mod test {
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

        assert_eq!(
            rpm_led_state.state().start_led,
            1,
            "We should have correctly configured the start LED"
        );

        let mut expected_led_configs = vec![LedConfiguration::default(); 5];

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "The initial state should be fully disabled LEDs with default colors"
        );

        rpm_led_state.update(&sim_state);

        assert_eq!(
            rpm_led_state.state().start_led,
            1,
            "Updating the RPM LED state with the new Sim state should not affect the start LED"
        );

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

        assert_eq!(
            rpm_led_state.state().start_led,
            1,
            "We should have correctly configured the start LED"
        );
        let mut expected_led_configs = vec![LedConfiguration::default(); 5];

        assert_eq!(
            expected_led_configs,
            rpm_led_state.state().leds,
            "The initial state should be fully disabled LEDs with default colors"
        );

        rpm_led_state.update(&sim_state);

        assert_eq!(
            rpm_led_state.state().start_led,
            1,
            "Updating the RPM LED state with the new Sim state should not affect the start LED"
        );

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
}
