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

use std::time::Instant;

use simetry::Moment;

use crate::led::profiles::flag::FlagContainer;

use super::{BlinkState, LedConfiguration, LedEffect, LedState};

#[derive(Debug)]
pub enum FlagColor {
    White,
    Yellow,
    Blue,
}

#[derive(Debug)]
pub struct FlagLedState {
    flag_color: FlagColor,
    container: FlagContainer,
    state: LedState,
    blink_state: BlinkState,
}

impl FlagLedState {
    pub fn new(flag_color: FlagColor, container: FlagContainer) -> Self {
        let led_count = container.led_count;

        let state = LedState::with_color(container.color.clone(), led_count);

        Self {
            flag_color,
            container,
            state,
            blink_state: BlinkState::default(),
        }
    }

    fn calculate_next_blink_state(&self, is_flag_enabled: bool) -> BlinkState {
        if self.container.blink_enabled && is_flag_enabled {
            match self.blink_state {
                BlinkState::NotBlinking => BlinkState::LedsTurnedOn {
                    state_change: Instant::now(),
                },
                BlinkState::LedsTurnedOff { state_change } => {
                    let delay = if self.container.dual_blink_timing_enabled {
                        self.container.off_delay
                    } else {
                        self.container.blink_delay
                    };

                    if state_change.elapsed() >= delay {
                        BlinkState::LedsTurnedOn {
                            state_change: Instant::now(),
                        }
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

    pub fn update(&mut self, state: &dyn Moment) {
        let Some(flags) = state.flags() else {
            return;
        };

        let is_flag_enabled = match self.flag_color {
            FlagColor::White => flags.white,
            FlagColor::Yellow => flags.yellow,
            FlagColor::Blue => flags.blue,
        };

        let next_blink_state = self.calculate_next_blink_state(is_flag_enabled);

        for led in &mut self.state.leds {
            let led_enabled = match next_blink_state {
                BlinkState::NotBlinking => is_flag_enabled,
                BlinkState::LedsTurnedOff { .. } => false,
                BlinkState::LedsTurnedOn { .. } => true,
            };

            *led = if led_enabled {
                LedConfiguration::On {
                    color: self.container.color.clone(),
                }
            } else {
                LedConfiguration::Off
            };
        }

        self.blink_state = next_blink_state;
    }
}

impl LedEffect for FlagLedState {
    fn update(&mut self, sim_state: &dyn Moment) {
        self.update(sim_state)
    }

    fn start_led(&self) -> usize {
        self.container.start_position.into()
    }

    fn description(&self) -> &str {
        &self.container.description
    }

    fn leds(&self) -> Box<dyn Iterator<Item = &LedState> + '_> {
        Box::new(std::iter::once(&self.state))
    }

    fn disable(&mut self) {
        self.blink_state = BlinkState::NotBlinking;

        for led in &mut self.state.leds {
            *led = LedConfiguration::Off;
        }
    }
}

#[cfg(test)]
pub mod test {
    use csscolorparser::Color;
    use serde_json::json;
    use simetry::RacingFlags;

    use crate::{led, leds};

    use super::*;

    pub struct SimState {
        pub inner: RacingFlags,
    }

    impl SimState {
        pub fn new() -> Self {
            Self {
                inner: Default::default(),
            }
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
        let mut state = FlagLedState::new(FlagColor::Yellow, container);

        state.update(&flags);

        assert_eq!(
            &leds![off; 3],
            &state.state,
            "The LEDs should stay off if no flag is waving"
        );

        flags.inner.white = true;
        state.update(&flags);

        assert_eq!(
            &leds![off; 3],
            &state.state,
            "The white flag should not turn on LEDs for the yellow flag"
        );

        flags.inner.yellow = true;
        state.update(&flags);

        assert_eq!(
            &leds!["Yellow"; 3],
            &state.state,
            "The yellow flag should turn all the LEDs on"
        );

        state.update(&flags);
        assert_eq!(
            &leds!["Yellow"; 3],
            &state.state,
            "The state of the LEDs should not change unless the blink delay has expired"
        );

        std::thread::sleep(state.container.blink_delay);
        state.update(&flags);

        assert_eq!(
            &leds![off; 3],
            &state.state,
            "The LEDs should be turned off after the blink delay has passed"
        );

        state.update(&flags);
        assert_eq!(
            &leds![off; 3],
            &state.state,
            "The state of the LEDs should not change unless the blink delay has expired"
        );

        std::thread::sleep(state.container.blink_delay);
        state.update(&flags);

        assert_eq!(
            &leds!["yellow"; 3],
            &state.state,
            "The LEDs should be turned on again after the blink delay has passed"
        );

        flags.inner.yellow = false;
        state.update(&flags);

        assert_eq!(
            &leds![off; 3],
            &state.state,
            "The LEDs should be turned off if the flag stopped waving"
        );
    }
}
