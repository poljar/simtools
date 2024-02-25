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

use std::num::NonZeroUsize;

use crate::{
    led::{
        effects::{
            groups::{EffectGroup, GroupCondition},
            BlinkConfiguration, BlinkState, BlinkTimings, LedEffect, LedGroup,
        },
        profiles::rpm::{RpmSegmentsContainer, StartValue},
    },
    leds,
    moment::MomentExt,
};

#[derive(Debug, Clone)]
struct LedSegment {
    start_value: StartValue,
    enabled: bool,
    on_leds: LedGroup,
    off_leds: LedGroup,
    blink_leds: Option<LedGroup>,
    blink_configuration: BlinkConfiguration,
}

impl LedEffect for LedSegment {
    fn leds(&self) -> Box<dyn Iterator<Item = &crate::led::effects::LedGroup> + '_> {
        let leds = if self.enabled {
            if let BlinkConfiguration::Enabled { state, .. } = self.blink_configuration {
                match state {
                    BlinkState::NotBlinking => &self.on_leds,
                    BlinkState::LedsTurnedOff { .. } => &self.off_leds,
                    BlinkState::LedsTurnedOn { .. } => {
                        self.blink_leds.as_ref().unwrap_or(&self.on_leds)
                    }
                }
            } else {
                &self.on_leds
            }
        } else {
            &self.off_leds
        };

        Box::new(std::iter::once(leds))
    }

    fn update(&mut self, sim_state: &dyn simetry::Moment) {
        let Some(rpm) = sim_state.vehicle_engine_rotation_speed() else {
            return;
        };

        let enabled = match self.start_value {
            StartValue::Rpm(start_value) => rpm >= start_value,
            StartValue::RpmPercentage(start_percentage) => {
                let rpm_percentage = sim_state.rpm_percentage();
                rpm_percentage >= start_percentage
            }
            StartValue::RedlinePercentage(start_percentage) => {
                let redline = sim_state.redline_rpm();
                let redline_percentage = rpm / redline * 100.0;

                redline_percentage >= start_percentage
            }
        };
        let redline_reached = sim_state.redline_reached();

        self.enabled = enabled;
        self.blink_configuration.update(redline_reached);
    }

    fn disable(&mut self) {
        self.enabled = false;
        self.blink_configuration.disable();
    }

    fn start_led(&self) -> NonZeroUsize {
        self.on_leds.start_position()
    }

    fn description(&self) -> &str {
        ""
    }
}

#[derive(Debug)]
pub struct RpmSegmentsEffect {
    inner: EffectGroup,
}

impl RpmSegmentsEffect {
    pub fn with_start_position(
        container: RpmSegmentsContainer,
        start_position: NonZeroUsize,
    ) -> Self {
        let mut segments: Vec<Box<dyn LedEffect>> = Vec::with_capacity(container.segments.len());
        let mut segment_position = start_position;

        for segment in container.segments {
            let led_count = segment.led_count;

            let blink_configuration = if container.blink_enabled {
                BlinkConfiguration::Enabled {
                    state: Default::default(),
                    timings: BlinkTimings::Single { timeout: container.blink_delay },
                }
            } else {
                BlinkConfiguration::Disabled
            };

            let segment = LedSegment {
                start_value: segment.start_value,
                enabled: false,
                on_leds: LedGroup::with_color(segment.normal_color, segment_position, led_count),
                off_leds: leds![segment_position.get(); off; led_count.get()],
                blink_leds: segment
                    .blinking_color
                    .map(|color| LedGroup::with_color(color, segment_position, led_count)),
                blink_configuration,
            };

            segment_position = segment_position.saturating_add(led_count.get());

            segments.push(Box::new(segment));
        }

        Self {
            inner: EffectGroup {
                start_position,
                condition: GroupCondition::AlwaysOn,
                states: segments,
            },
        }
    }
}

impl LedEffect for RpmSegmentsEffect {
    fn leds(&self) -> Box<dyn Iterator<Item = &crate::led::effects::LedGroup> + '_> {
        self.inner.leds()
    }

    fn update(&mut self, sim_state: &dyn simetry::Moment) {
        self.inner.update(sim_state)
    }

    fn disable(&mut self) {
        self.inner.disable()
    }

    fn start_led(&self) -> NonZeroUsize {
        self.inner.start_position
    }

    fn description(&self) -> &str {
        self.inner.description()
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;
    use similar_asserts::assert_eq;

    use super::*;
    use crate::{assert_led_group_eq, led::effects::rpm::test::RpmSimState};

    fn rpm_percentage_container() -> RpmSegmentsContainer {
        let json = json!({
            "BlinkDelay": 125,
            "Segments": [
                {
                  "StartValue": 70.0,
                  "NormalColor": "Lime",
                  "BlinkingColor": "Blue",
                  "LedCount": 5
                },
                {
                  "StartValue": 85.0,
                  "NormalColor": "Red",
                  "BlinkingColor": "Blue",
                  "UseBlinkingColor": false,
                  "LedCount": 5
                },
                {
                  "StartValue": 90.0,
                  "NormalColor": "Blue",
                  "BlinkingColor": "Blue",
                  "LedCount": 5
                }
            ],
            "SegmentsCount": 3,
            "BlinkEnabled": true,
            "RpmMode": 0,
            "RelativeToRedline": false,
            "BlinkOnLastGear": true,
            "ContainerType": "RPMSegmentsContainer",
            "IsEnabled": true
        });

        serde_json::from_value(json)
            .expect("We should be able to deserialize the RPM segments container")
    }

    fn rpm_container() -> RpmSegmentsContainer {
        let json = json!({
            "BlinkDelay": 125,
            "Segments": [
                {
                  "StartValue": 5000.0,
                  "NormalColor": "Lime",
                  "BlinkingColor": "Blue",
                  "LedCount": 3
                },
                {
                  "StartValue": 8000.0,
                  "NormalColor": "Blue",
                  "BlinkingColor": "Blue",
                  "LedCount": 3
                }
            ],
            "SegmentsCount": 2,
            "BlinkEnabled": false,
            "RpmMode": 2,
            "RelativeToRedline": false,
            "BlinkOnLastGear": true,
            "ContainerType": "RPMSegmentsContainer",
            "IsEnabled": true
        });

        serde_json::from_value(json)
            .expect("We should be able to deserialize the RPM segments container")
    }

    fn redline_percentage_container() -> RpmSegmentsContainer {
        let json = json!({
            "BlinkDelay": 125,
            "Segments": [
                {
                  "StartValue": 5000.0,
                  "NormalColor": "Lime",
                  "BlinkingColor": "Blue",
                  "LedCount": 3
                },
                {
                  "StartValue": 8000.0,
                  "NormalColor": "Blue",
                  "BlinkingColor": "Blue",
                  "LedCount": 3
                }
            ],
            "SegmentsCount": 2,
            "BlinkEnabled": false,
            "RpmMode": 2,
            "RelativeToRedline": false,
            "BlinkOnLastGear": true,
            "ContainerType": "RPMSegmentsContainer",
            "IsEnabled": true
        });

        serde_json::from_value(json)
            .expect("We should be able to deserialize the RPM segments container")
    }

    #[test]
    fn effect_constructor() {
        let container = rpm_percentage_container();
        let description = container.description.clone();
        let effect = RpmSegmentsEffect::with_start_position(container, NonZeroUsize::MIN);

        assert_eq!(effect.start_led(), NonZeroUsize::MIN);
        assert_eq!(effect.description(), description);
        assert_eq!(effect.inner.states[1].start_led(), NonZeroUsize::new(6).unwrap());
    }

    #[test]
    fn rpm_percentage() {
        const MAX_RPM: f64 = 9000.0;

        let container = rpm_percentage_container();
        let blink_delay = container.blink_delay;
        let mut effect = RpmSegmentsEffect::with_start_position(container, NonZeroUsize::MIN);
        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);

        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                [off; 5],
                [6; off; 5],
                [11; off; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        sim_state.update_rpm(0.70 * MAX_RPM);
        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                ["Lime"; 5],
                [6; off; 5],
                [11; off; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        sim_state.update_rpm(0.85 * MAX_RPM);
        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                ["Lime"; 5],
                [6; "Red"; 5],
                [11; off; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        sim_state.update_rpm(0.90 * MAX_RPM);
        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                ["Lime"; 5],
                [6; "Red"; 5],
                [11; "Blue"; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        sim_state.update_rpm(0.99 * MAX_RPM);
        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                ["Blue"; 5],
                [6; "Red"; 5],
                [11; "Blue"; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        std::thread::sleep(blink_delay);

        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                [off; 5],
                [6; off; 5],
                [11; off; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        std::thread::sleep(blink_delay);
        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                ["Blue"; 5],
                [6; "Red"; 5],
                [11; "Blue"; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        effect.disable();

        assert_led_group_eq!(
            [
                [off; 5],
                [6; off; 5],
                [11; off; 5],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );
    }

    #[test]
    fn rpm() {
        const MAX_RPM: f64 = 9000.0;

        let container = rpm_container();
        let mut effect = RpmSegmentsEffect::with_start_position(container, NonZeroUsize::MIN);
        let mut sim_state = RpmSimState::new(0.0, MAX_RPM);

        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                [off; 3],
                [4; off; 3],
            ],
            effect,
            "Initially, the LEDs should be turned off"
        );

        sim_state.update_rpm(5000.0);
        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                ["Lime"; 3],
                [4; off; 3],
            ],
            effect,
            "The first LED segment should be lime when we reach 5000 RPM"
        );

        sim_state.update_rpm(9000.0);
        effect.update(&sim_state);
        assert_led_group_eq!(
            [
                ["Lime"; 3],
                [4; "Blue"; 3],
            ],
            effect,
            "The next segment, the blue one, should turn on after 9000 RPM as well"
        );
    }
}
