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

use simetry::Moment;

use super::{BlinkConfiguration, BlinkState, BlinkTimings, LedEffect, LedGroup};
use crate::{
    led::profiles::{flag::FlagContainer, redline::RedlineReachedContainer, SimpleBlinkContainer},
    leds,
    moment::MomentExt,
};

#[derive(Debug)]
pub enum FlagColor {
    White,
    Yellow,
    Blue,
}

#[derive(Debug)]
pub enum EffectCondition {
    Flag { color: FlagColor },
    RedLine,
}

#[derive(Debug)]
pub struct BlinkEffect {
    condition: EffectCondition,
    container: SimpleBlinkContainer,
    on_leds: LedGroup,
    off_leds: LedGroup,
    enabled: bool,
    blink_configuration: BlinkConfiguration,
}

impl BlinkEffect {
    pub fn flag(
        flag_color: FlagColor,
        container: FlagContainer,
        start_position: NonZeroUsize,
    ) -> Self {
        Self::new_helper(EffectCondition::Flag { color: flag_color }, container, start_position)
    }

    pub fn redline(container: RedlineReachedContainer, start_position: NonZeroUsize) -> Self {
        Self::new_helper(EffectCondition::RedLine, container, start_position)
    }

    fn new_helper(
        condition: EffectCondition,
        container: SimpleBlinkContainer,
        start_position: NonZeroUsize,
    ) -> Self {
        let led_count = container.led_count;

        let blink_configuration = if container.blink_enabled {
            let timings = if container.dual_blink_timing_enabled {
                BlinkTimings::Double {
                    on_timeout: container.on_delay,
                    off_timeout: container.off_delay,
                }
            } else {
                BlinkTimings::Single { timeout: container.blink_delay }
            };

            BlinkConfiguration::Enabled { state: Default::default(), timings }
        } else {
            BlinkConfiguration::Disabled
        };

        Self {
            condition,
            enabled: false,
            on_leds: LedGroup::with_color(container.color.clone(), start_position, led_count),
            off_leds: leds![start_position.into(); off; led_count.into()],
            container,
            blink_configuration,
        }
    }

    #[cfg(test)]
    pub fn new(flag_color: FlagColor, container: FlagContainer) -> Self {
        let start_position = container.start_position;
        Self::flag(flag_color, container, start_position)
    }

    pub fn update(&mut self, state: &dyn Moment) {
        let Some(flags) = state.flags() else {
            return;
        };

        let is_enabled = match &self.condition {
            EffectCondition::Flag { color } => match color {
                FlagColor::White => flags.white,
                FlagColor::Yellow => flags.yellow,
                FlagColor::Blue => flags.blue,
            },
            EffectCondition::RedLine => state.redline_reached(),
        };

        self.enabled = is_enabled;
        self.blink_configuration.update(is_enabled);
    }
}

impl LedEffect for BlinkEffect {
    fn update(&mut self, sim_state: &dyn Moment) {
        self.update(sim_state)
    }

    fn start_led(&self) -> NonZeroUsize {
        self.on_leds.start_position()
    }

    fn description(&self) -> &str {
        &self.container.description
    }

    fn leds(&self) -> Box<dyn Iterator<Item = &LedGroup> + '_> {
        let leds = match self.blink_configuration {
            BlinkConfiguration::Disabled => {
                if self.enabled {
                    &self.on_leds
                } else {
                    &self.off_leds
                }
            }
            BlinkConfiguration::Enabled { state, .. } => match state {
                BlinkState::NotBlinking | BlinkState::LedsTurnedOff { .. } => &self.off_leds,
                BlinkState::LedsTurnedOn { .. } => &self.on_leds,
            },
        };

        Box::new(std::iter::once(leds))
    }

    fn disable(&mut self) {
        self.enabled = false;
        self.blink_configuration.update(false);
    }

    fn led_count(&self) -> usize {
        self.on_leds.leds.len()
    }
}

#[cfg(test)]
pub mod test {
    use serde_json::json;
    use simetry::RacingFlags;
    use similar_asserts::assert_eq;

    use super::*;
    use crate::{led::profiles::flag::FlagContainer, leds};

    pub struct SimState {
        pub inner: RacingFlags,
    }

    impl SimState {
        pub fn new() -> Self {
            Self { inner: Default::default() }
        }
    }

    impl Moment for SimState {
        fn flags(&self) -> Option<RacingFlags> {
            Some(self.inner.clone())
        }
    }

    fn container() -> FlagContainer {
        let container = json!({
            "LedCount": 3,
            "Color": "Yellow",
            "BlinkEnabled": true,
            "BlinkDelay": 50,
            "DualBlinkTimingEnabled": false,
            "OffDelay": 75,
            "OnDelay": 12,
            "StartPosition": 14,
            "ContainerType": "YellowFlagContainer",
            "Description": "Generates a static color when the Yellow flag is ON copy",
            "IsEnabled": true
        });

        serde_json::from_value(container)
            .expect("We should be able to deserialize the default Flag container")
    }

    #[test]
    fn blinking() {
        let container = container();

        let mut flags = SimState::new();
        let mut state = BlinkEffect::new(FlagColor::Yellow, container);

        state.update(&flags);

        assert_eq!(
            &leds![14; off; 3],
            state.leds().next().unwrap(),
            "The LEDs should stay off if no flag is waving"
        );

        flags.inner.white = true;
        state.update(&flags);

        assert_eq!(
            &leds![14; off; 3],
            state.leds().next().unwrap(),
            "The white flag should not turn on LEDs for the yellow flag"
        );

        flags.inner.yellow = true;
        state.update(&flags);

        assert_eq!(
            &leds![14; "Yellow"; 3],
            state.leds().next().unwrap(),
            "The yellow flag should turn all the LEDs on"
        );

        state.update(&flags);
        assert_eq!(
            &leds![14; "Yellow"; 3],
            state.leds().next().unwrap(),
            "The state of the LEDs should not change unless the blink delay has expired"
        );

        std::thread::sleep(state.container.blink_delay);
        state.update(&flags);

        assert_eq!(
            &leds![14; off; 3],
            state.leds().next().unwrap(),
            "The LEDs should be turned off after the blink delay has passed"
        );

        state.update(&flags);
        assert_eq!(
            &leds![14; off; 3],
            state.leds().next().unwrap(),
            "The state of the LEDs should not change unless the blink delay has expired"
        );

        std::thread::sleep(state.container.blink_delay);
        state.update(&flags);

        assert_eq!(
            &leds![14; "yellow"; 3],
            state.leds().next().unwrap(),
            "The LEDs should be turned on again after the blink delay has passed"
        );

        flags.inner.yellow = false;
        state.update(&flags);

        assert_eq!(
            &leds![14; off; 3],
            state.leds().next().unwrap(),
            "The LEDs should be turned off if the flag stopped waving"
        );
    }
}
