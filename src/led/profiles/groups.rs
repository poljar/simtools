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

use std::{num::NonZeroUsize, time::Duration};

use serde::Deserialize;

use super::{default_non_zero, duration_from_int_ms, LedContainer};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StackingType {
    LeftToRight,
    #[default]
    Layered,
}

impl<'de> Deserialize<'de> for StackingType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let stack_left_to_right = bool::deserialize(deserializer)?;

        Ok(if stack_left_to_right { StackingType::LeftToRight } else { StackingType::Layered })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SimpleGroupContainer {
    #[serde(default)]
    pub description: String,
    pub is_enabled: bool,
    #[serde(default, rename = "StackLeftToRight")]
    pub stacking_type: StackingType,
    #[serde(default = "default_non_zero")]
    pub start_position: NonZeroUsize,
    pub led_containers: Vec<LedContainer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TimeLimitedGroupContainer {
    #[serde(default)]
    pub description: String,
    pub is_enabled: bool,
    #[serde(deserialize_with = "duration_from_int_ms")]
    pub duration: Duration,
    #[serde(default, rename = "StackLeftToRight")]
    pub stacking_type: StackingType,
    #[serde(default = "default_non_zero")]
    pub start_position: NonZeroUsize,
    pub led_containers: Vec<LedContainer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Formula {
    pub expression: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ConditionalGroupContainer {
    #[serde(default)]
    pub description: String,
    pub is_enabled: bool,
    #[serde(default, rename = "StackLeftToRight")]
    pub stacking_type: StackingType,
    #[serde(default = "default_non_zero")]
    pub start_position: NonZeroUsize,
    pub led_containers: Vec<LedContainer>,
    pub trigger_formula: Formula,
}
