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

use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use simetry::Moment;

use super::{
    blink::{BlinkEffect, FlagColor},
    rpm::gradient::RpmGradientEffect,
    LedEffect, LedGroup,
};
use crate::{
    led::profiles::{
        groups::{Formula, StackingType},
        GroupContainer, LedContainer, LedProfile,
    },
    moment::MomentExt,
};

#[derive(Debug)]
pub enum GroupCondition {
    AlwaysOn,
    GameStarted,
    CarStarted { duration: Duration, state: SimpleConditionstate },
    Expression { formula: Formula },
}

impl From<&GroupContainer> for GroupCondition {
    fn from(value: &GroupContainer) -> Self {
        match value {
            GroupContainer::Simple(_) => GroupCondition::AlwaysOn,
            GroupContainer::GameRunning(_) => GroupCondition::GameStarted,
            GroupContainer::CarStarted(c) => {
                GroupCondition::CarStarted { duration: c.duration, state: Default::default() }
            }
            GroupContainer::Conditional(c) => {
                GroupCondition::Expression { formula: c.trigger_formula.clone() }
            }
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
pub struct EffectGroup {
    pub(super) start_position: NonZeroUsize,
    pub(super) condition: GroupCondition,
    pub(super) states: Vec<Box<dyn LedEffect>>,
}

impl EffectGroup {
    pub fn root(profile: LedProfile) -> Self {
        Self::new_helper(
            GroupCondition::AlwaysOn,
            NonZeroUsize::MIN,
            StackingType::Layered,
            profile.led_containers,
        )
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

    fn create_led_effect(
        container: LedContainer,
        start_position: NonZeroUsize,
    ) -> Option<Box<dyn LedEffect>> {
        match container {
            LedContainer::Rpm(c) => {
                Some(Box::new(RpmGradientEffect::with_start_position(c, start_position)))
            }
            LedContainer::RpmSegments(_) | LedContainer::SpeedLimiterAnimation(_) => None,
            LedContainer::Group(c) => Some(Box::new(Self::new(c))),
            LedContainer::BlueFlag(c) => {
                Some(Box::new(BlinkEffect::flag(FlagColor::Blue, c, start_position)))
            }
            LedContainer::WhiteFlag(c) => {
                Some(Box::new(BlinkEffect::flag(FlagColor::White, c, start_position)))
            }
            LedContainer::YellowFlag(c) => {
                Some(Box::new(BlinkEffect::flag(FlagColor::Yellow, c, start_position)))
            }
            LedContainer::RedlineReached(c) => {
                Some(Box::new(BlinkEffect::redline(c, start_position)))
            }
            LedContainer::Unknown { .. } => None,
        }
    }

    fn new_helper(
        condition: GroupCondition,
        group_start_position: NonZeroUsize,
        stacking_type: StackingType,
        containers: Vec<LedContainer>,
    ) -> Self {
        let mut states = Vec::with_capacity(containers.len());

        let mut start_position = group_start_position;

        for container in containers {
            // If the container is defined in the profile, but not enabled, skip it. No need
            // to waste memory on effects that shouldn't modify the LED states.
            if !container.enabled() {
                continue;
            }

            if stacking_type == StackingType::Layered {
                start_position =
                    group_start_position.saturating_add(container.start_position().get() - 1);
            }

            let Some(state) = Self::create_led_effect(container, start_position) else {
                continue;
            };

            if stacking_type == StackingType::LeftToRight {
                start_position = start_position.saturating_add(state.led_count());
            }

            states.push(state);
        }

        Self { condition, start_position, states }
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
                        *state = SimpleConditionstate::Triggered { trigger_time: Instant::now() };
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
            GroupCondition::Expression { .. } => (),
        }
    }
}

impl LedEffect for EffectGroup {
    fn update(&mut self, sim_state: &dyn Moment) {
        self.update(sim_state)
    }

    fn start_led(&self) -> NonZeroUsize {
        self.start_position
    }

    fn description(&self) -> &str {
        ""
    }

    fn leds(&self) -> Box<dyn Iterator<Item = &LedGroup> + '_> {
        Box::new(self.states.iter().flat_map(|s| s.leds()))
    }

    fn disable(&mut self) {
        for state in &mut self.states {
            state.disable()
        }
    }

    fn led_count(&self) -> usize {
        self.states.iter().map(|state| state.led_count()).sum()
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;
    use crate::{assert_led_group_eq, led::effects::blink::test::SimState, leds};

    fn container(stack_left_to_right: bool) -> GroupContainer {
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
          "StackLeftToRight": stack_left_to_right,
          "StartPosition": 3,
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
    fn white_flag() {
        let container = container(false);
        let mut effect = EffectGroup::new(container);
        let mut flags = SimState::new();

        effect.update(&flags);

        assert_led_group_eq!(
            [
                [3; off; 3],
                [16; off; 3],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        flags.inner.white = true;
        effect.update(&flags);

        assert_led_group_eq!(
            [
                [3; "White"; 3],
                [16; "White"; 3],
            ],
            effect,
            "The white flag should turn all the LEDs on"
        );
    }

    #[test]
    fn white_flag_left_to_right_stacking() {
        let container = container(true);
        let mut effect = EffectGroup::new(container);
        let mut flags = SimState::new();

        effect.update(&flags);

        assert_led_group_eq!(
            [
                [3; off; 3],
                [6; off; 3],
            ],
            effect,
            "The LEDs should stay off if no flag is waving"
        );

        flags.inner.white = true;
        effect.update(&flags);

        assert_led_group_eq!(
            [
                [3; "White"; 3],
                [6; "White"; 3],
            ],
            effect,
            "The white yellow flag should turn all the LEDs on"
        );
    }
}
