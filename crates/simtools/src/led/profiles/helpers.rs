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

use csscolorparser::Color;
use serde::{Deserialize, Deserializer};

/// Helper to deserialize a integer containing milliseconds into a [`Duration`].
pub fn duration_from_int_ms<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Duration::from_millis)
}

/// Helper to deserialize a string containing a HTML color into a [`Color`].
pub fn color_from_str<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;

    let error = |e, source| {
        let message =
            format!("Couldn't parse a a digit into a RGB8 value, source: {source}: {e:?}");

        serde::de::Error::custom(message)
    };

    // First check it it's just a comma separated list of u8 values. If it isn't
    // then let [`Color::from_html`] handle the parsing.
    let numbers: Vec<&str> = string.split(',').map(|n| n.trim()).collect();

    if numbers.len() == 3 {
        let r = numbers[0].parse().map_err(|e| error(e, numbers[0]))?;
        let g = numbers[1].parse().map_err(|e| error(e, numbers[1]))?;
        let b = numbers[2].parse().map_err(|e| error(e, numbers[2]))?;

        Ok(Color::from_rgba8(r, g, b, 255))
    } else {
        Color::from_html(string).map_err(serde::de::Error::custom)
    }
}

pub fn default_non_zero() -> NonZeroUsize {
    NonZeroUsize::MIN
}

pub fn default_true() -> bool {
    true
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;

    #[derive(Debug, Deserialize)]
    struct TestProfile {
        #[serde(deserialize_with = "color_from_str")]
        color: Color,
    }

    #[test]
    fn color_parsing() {
        let TestProfile { color } = serde_json::from_value(json!({"color": "Yellow"}))
            .expect("We should be able to parse a color with a name");
        assert_eq!(color.to_rgba8(), [255, 255, 0, 255]);

        let TestProfile { color } = serde_json::from_value(json!({"color": "#ff00ff"}))
            .expect("We should be able to parse a color with a name");
        assert_eq!(color.to_rgba8(), [255, 0, 255, 255]);

        let TestProfile { color } = serde_json::from_value(json!({"color": "0, 255, 0"}))
            .expect("We should be able to parse a color with a name");
        assert_eq!(color.to_rgba8(), [0, 255, 0, 255]);

        serde_json::from_value::<TestProfile>(json!({"color": "0, 700, 0"})).expect_err(
            "We should not be able to parse a color value that is outside of our u8 range",
        );
    }
}
