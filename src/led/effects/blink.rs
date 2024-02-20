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

use simetry::Moment;

use super::{BlinkState, LedConfiguration, LedEffect, LedGroup, MomentExt};
use crate::led::profiles::{
    flag::FlagContainer, redline::RedlineReachedContainer, SimpleBlinkContainer,
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
    state: LedGroup,
    blink_state: BlinkState,
}

impl BlinkEffect {
    pub fn flag(
        flag_color: FlagColor,
        container: FlagContainer,
        start_position: NonZeroUsize,
    ) -> Self {
        let led_count = container.led_count;

        Self {
            condition: EffectCondition::Flag { color: flag_color },
            state: LedGroup::with_color(container.color.clone(), start_position, led_count),
            container,
            blink_state: BlinkState::default(),
        }
    }

    pub fn redline(container: RedlineReachedContainer, start_position: NonZeroUsize) -> Self {
        let led_count = container.led_count;

        Self {
            condition: EffectCondition::RedLine,
            state: LedGroup::with_color(container.color.clone(), start_position, led_count),
            container,
            blink_state: BlinkState::default(),
        }
    }

    #[cfg(test)]
    pub fn new(flag_color: FlagColor, container: FlagContainer) -> Self {
        let start_position = container.start_position;
        Self::flag(flag_color, container, start_position)
    }

    fn calculate_next_blink_state(&self, is_flag_enabled: bool) -> BlinkState {
        if self.container.blink_enabled && is_flag_enabled {
            match self.blink_state {
                BlinkState::NotBlinking => {
                    BlinkState::LedsTurnedOn { state_change: Instant::now() }
                }
                BlinkState::LedsTurnedOff { state_change } => {
                    let delay = if self.container.dual_blink_timing_enabled {
                        self.container.off_delay
                    } else {
                        self.container.blink_delay
                    };

                    if state_change.elapsed() >= delay {
                        BlinkState::LedsTurnedOn { state_change: Instant::now() }
                    } else {
                        self.blink_state
                    }
                }
                BlinkState::LedsTurnedOn { state_change } => {
                    let delay = if self.container.dual_blink_timing_enabled {
                        self.container.on_delay
                    } else {
                        self.container.blink_delay
                    };

                    if state_change.elapsed() >= delay {
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

        let next_blink_state = self.calculate_next_blink_state(is_enabled);

        let leds_enabled = match next_blink_state {
            BlinkState::NotBlinking => is_enabled,
            BlinkState::LedsTurnedOff { .. } => false,
            BlinkState::LedsTurnedOn { .. } => true,
        };

        // TODO: We could avoid this for loop if we had two LED arrays, one for the
        // turned on state, one for the turned off state. Then we just flip a
        // boolean.
        for led in &mut self.state.leds {
            *led = if leds_enabled {
                LedConfiguration::On { color: self.container.color.clone() }
            } else {
                LedConfiguration::Off
            };
        }

        self.blink_state = next_blink_state;
    }
}

impl LedEffect for BlinkEffect {
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
            &state.state,
            "The LEDs should stay off if no flag is waving"
        );

        flags.inner.white = true;
        state.update(&flags);

        assert_eq!(
            &leds![14; off; 3],
            &state.state,
            "The white flag should not turn on LEDs for the yellow flag"
        );

        flags.inner.yellow = true;
        state.update(&flags);

        assert_eq!(
            &leds![14; "Yellow"; 3],
            &state.state,
            "The yellow flag should turn all the LEDs on"
        );

        state.update(&flags);
        assert_eq!(
            &leds![14; "Yellow"; 3],
            &state.state,
            "The state of the LEDs should not change unless the blink delay has expired"
        );

        std::thread::sleep(state.container.blink_delay);
        state.update(&flags);

        assert_eq!(
            &leds![14; off; 3],
            &state.state,
            "The LEDs should be turned off after the blink delay has passed"
        );

        state.update(&flags);
        assert_eq!(
            &leds![14; off; 3],
            &state.state,
            "The state of the LEDs should not change unless the blink delay has expired"
        );

        std::thread::sleep(state.container.blink_delay);
        state.update(&flags);

        assert_eq!(
            &leds![14; "yellow"; 3],
            &state.state,
            "The LEDs should be turned on again after the blink delay has passed"
        );

        flags.inner.yellow = false;
        state.update(&flags);

        assert_eq!(
            &leds![14; off; 3],
            &state.state,
            "The LEDs should be turned off if the flag stopped waving"
        );
    }
}
