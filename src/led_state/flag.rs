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

use crate::led_profile::flag::FlagContainer;

use super::{BlinkState, LedConfiguration, LedState};

pub enum FlagColor {
    White,
    Yellow,
    Blue,
}

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

    pub fn container(&self) -> &FlagContainer {
        &self.container
    }
}
