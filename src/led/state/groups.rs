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

use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use simetry::Moment;

use crate::led::profiles::{
    groups::{Formula, StackingType},
    GroupContainer, LedContainer, LedProfile,
};

use super::{
    flag::{FlagColor, FlagLedState},
    rpm::gradient::RpmLedState,
    LedEffect, LedState, MomentExt,
};

#[derive(Debug)]
pub enum GroupCondition {
    AlwaysOn,
    GameStarted,
    CarStarted {
        duration: Duration,
        state: SimpleConditionstate,
    },
    Conditional {
        formula: Formula,
    },
}

impl From<&GroupContainer> for GroupCondition {
    fn from(value: &GroupContainer) -> Self {
        match value {
            GroupContainer::Simple(_) => GroupCondition::AlwaysOn,
            GroupContainer::GameRunning(_) => GroupCondition::GameStarted,
            GroupContainer::CarStarted(c) => GroupCondition::CarStarted {
                duration: c.duration,
                state: Default::default(),
            },
            GroupContainer::Conditional(c) => GroupCondition::Conditional {
                formula: c.trigger_formula.clone(),
            },
        }
    }
}

#[derive(Debug, Default)]
pub enum SimpleConditionstate {
    #[default]
    Waiting,
    Triggered {
        trigger_time: Instant,
    },
    Expired,
}

#[derive(Debug)]
pub struct GroupState {
    start_position: NonZeroUsize,
    stacking_type: StackingType,
    condition: GroupCondition,
    states: Vec<Box<dyn LedEffect>>,
}

impl GroupState {
    pub fn root(profile: LedProfile) -> Self {
        let condition = GroupCondition::AlwaysOn;
        let stacking_type = StackingType::Layered;
        let start_position = NonZeroUsize::MIN;

        let containers = profile.led_containers;

        Self::new_helper(condition, start_position, stacking_type, containers)
    }

    pub fn new(container: GroupContainer) -> Self {
        let condition = GroupCondition::from(&container);

        let (stacking_type, start_position, containers) = match container {
            GroupContainer::Simple(c) => (c.stacking_type, c.start_position, c.led_containers),
            GroupContainer::GameRunning(c) => (c.stacking_type, c.start_position, c.led_containers),
            GroupContainer::CarStarted(c) => (c.stacking_type, c.start_position, c.led_containers),
            GroupContainer::Conditional(c) => (c.stacking_type, c.start_position, c.led_containers),
        };

        Self::new_helper(condition, start_position, stacking_type, containers)
    }

    fn new_helper(
        condition: GroupCondition,
        start_position: NonZeroUsize,
        stacking_type: StackingType,
        containers: Vec<LedContainer>,
    ) -> Self {
        let mut states = Vec::with_capacity(containers.len());

        // TODO: We need to modify the start position of every child, if the stack type is layered
        // then every child's start position should just be pushed by the parents start position.
        // On the other hand, if the stacking type is left to right, then we need to add the
        // parents start position to the first child, the first child's start position to the
        // second childe...

        for container in containers {
            let state: Box<dyn LedEffect> = match container {
                LedContainer::Rpm(c) => Box::new(RpmLedState::new(c)),
                LedContainer::RpmSegments(_)
                | LedContainer::RedlineReached(_)
                | LedContainer::SpeedLimiterAnimation(_) => continue,
                LedContainer::Group(c) => Box::new(Self::new(c)),
                LedContainer::BlueFlag(c) => Box::new(FlagLedState::new(FlagColor::Blue, c)),
                LedContainer::WhiteFlag(c) => Box::new(FlagLedState::new(FlagColor::White, c)),
                LedContainer::YellowFlag(c) => Box::new(FlagLedState::new(FlagColor::Yellow, c)),
                LedContainer::Unknown { container_type, .. } => continue,
            };

            states.push(state);
        }

        Self {
            condition,
            stacking_type,
            start_position,
            states,
        }
    }

    fn update_states(&mut self, sim_state: &dyn Moment) {
        for state in &mut self.states {
            state.update(sim_state);
        }
    }

    pub fn update(&mut self, sim_state: &dyn Moment) {
        match &mut self.condition {
            // TODO: Once simetry exposes if the game has started or not, use that information to
            // guard the `GameStarted` condition.
            GroupCondition::AlwaysOn | GroupCondition::GameStarted => self.update_states(sim_state),
            GroupCondition::CarStarted { duration, state } => match state {
                SimpleConditionstate::Waiting => {
                    if sim_state.is_engine_running() {
                        *state = SimpleConditionstate::Triggered {
                            trigger_time: Instant::now(),
                        };
                        self.update_states(sim_state);
                    }
                }
                SimpleConditionstate::Triggered { trigger_time } => {
                    if &trigger_time.elapsed() >= duration {
                        *state = SimpleConditionstate::Expired;
                        self.disable();
                    } else {
                        self.update_states(sim_state);
                    }
                }
                SimpleConditionstate::Expired => {
                    if !sim_state.is_engine_running() {
                        *state = SimpleConditionstate::Waiting;
                    }
                }
            },
            // TODO: Support ncalc style expressions.
            GroupCondition::Conditional { formula } => (),
        }
    }
}

impl LedEffect for GroupState {
    fn update(&mut self, sim_state: &dyn Moment) {
        self.update(sim_state)
    }

    fn start_led(&self) -> usize {
        self.start_position.into()
    }

    fn description(&self) -> &str {
        ""
    }

    fn leds(&self) -> Box<dyn Iterator<Item = &LedState> + '_> {
        Box::new(self.states.iter().map(|s| s.leds()).flatten())
    }

    fn disable(&mut self) {
        for state in &mut self.states {
            state.disable()
        }
    }
}

#[cfg(test)]
mod test {
    use csscolorparser::Color;
    use serde_json::json;

    use crate::{
        led,
        led::state::{flag::test::SimState, LedConfiguration},
        leds,
    };

    use super::*;

    fn container() -> GroupContainer {
        let container = json!({
          "LedContainers": [
              {
                  "LedCount": 3,
                  "Color": "White",
                  "BlinkEnabled": true,
                  "BlinkDelay": 500,
                  "DualBlinkTimingEnabled": false,
                  "OffDelay": 750,
                  "OnDelay": 125,
                  "StartPosition": 1,
                  "ContainerId": "97b5f4af-d098-443b-818e-0c1a1e79fb87",
                  "ContainerType": "WhiteFlagContainer",
                  "Description": "Generates a static color when the White flag is ON",
                  "IsEnabled": true
              },
              {
                  "LedCount": 3,
                  "Color": "White",
                  "BlinkEnabled": true,
                  "BlinkDelay": 500,
                  "DualBlinkTimingEnabled": false,
                  "OffDelay": 750,
                  "OnDelay": 125,
                  "StartPosition": 14,
                  "ContainerId": "e079f63b-f727-4f97-8017-1796298697cd",
                  "ContainerType": "WhiteFlagContainer",
                  "Description": "Generates a static color when the White flag is ON copy",
                  "IsEnabled": true
              }
          ],
          "StackLeftToRight": false,
          "StartPosition": 1,
          "ContainerType": "GroupContainer",
          "Description": "Group",
          "IsEnabled": true
        });

        GroupContainer::Simple(
            serde_json::from_value(container)
                .expect("We should be able to deserialize the default Flag container"),
        )
    }

    #[test]
    fn white_flag_group() {
        let container = container();
        let mut state = GroupState::new(container);

        let mut flags = SimState::new();

        state.update(&flags);

        assert_eq!(
            &leds![off; 3],
            state.states[0].leds().next().unwrap(),
            "The LEDs should stay off if no flag is waving"
        );

        flags.inner.white = true;
        state.update(&flags);

        assert_eq!(
            &leds!["White"; 3],
            state.states[0].leds().next().unwrap(),
            "The yellow flag should turn all the LEDs on"
        );

        assert_eq!(
            &leds!["White"; 3],
            state.states[1].leds().next().unwrap(),
            "The yellow flag should turn all the LEDs on"
        );
    }
}
