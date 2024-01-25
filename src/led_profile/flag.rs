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

use std::time::Duration;

use csscolorparser::Color;
use serde::Deserialize;
use uuid::Uuid;

use super::{color_from_str, duration_from_int_ms};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FlagContainer {
    pub description: String,
    pub container_id: Uuid,
    pub is_enabled: bool,
    pub led_count: u32,
    pub start_position: u32,
    #[serde(deserialize_with = "color_from_str")]
    pub color: Color,
    pub blink_enabled: bool,
    #[serde(deserialize_with = "duration_from_int_ms")]
    pub blink_delay: Duration,
    pub dual_blink_timing_enabled: bool,
    #[serde(deserialize_with = "duration_from_int_ms")]
    pub off_delay: Duration,
    #[serde(deserialize_with = "duration_from_int_ms")]
    pub on_delay: Duration,
}
