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

#[macro_export]
macro_rules! leds {
    ($start_position:expr; $color:tt; $n:expr) => {
        $crate::led::effects::LedGroup {
            start_position: ::std::num::NonZeroUsize::new($start_position).expect("Invalid start position, must be non-zero"),
            leds: vec![leds!(@led $color); $n],
        }
    };

    ($color:tt; $n:expr) => {
        leds![1; $color; $n]
    };

    ($start_position:expr; $($color:tt),+ $(,)?) => {{
        let leds = vec![
            $(leds!(@led $color)),+
        ];

        LedGroup {
            start_position: ::std::num::NonZeroUsize::new($start_position).expect("Invalid start position, must be non-zero"),
            leds
        }
    }};

    ($($color:tt),+ $(,)?) => {{
        leds![1; $($color),+]
    }};

    (@led off) => {
        $crate::led::effects::LedConfiguration::Off
    };
    (@led ($r:expr, $g:expr, $b:expr)) => {
        $crate::led::effects::LedConfiguration::On {
            color: ::csscolorparser::Color::new($r, $g, $b, 1.0),
        }
    };
    (@led $color:expr) => {
        $crate::led::effects::LedConfiguration::On {
            color: ::csscolorparser::Color::from_html($color).unwrap(),
        }
    };
}

#[macro_export]
macro_rules! assert_led_group_eq {
    ([$($tt:tt),* $(,)?], $segments:expr $(,)?) => {
        assert_led_group_eq!(
            [$({ $tt });*],
            $segments,
            "The LED segment doesn't match to the asserted LEDs."
        )
    };

    ([$($tt:tt),* $(,)?], $segments:expr, $description:literal $(,)?) => {
        let mut index: isize = -1_isize;

        assert_led_group_eq!(
            @inner
            [$({
                { index += 1_isize; index };
                $tt
            });*],
            $segments,
            $description
        )
    };

    (@inner [$({$index:expr; $tt:tt});+ $(,)?], $segments:expr, $description:expr) => {
        $({
            let index = $index;

            let expected = $crate::leds!$tt;
            let computed = $segments
                .leds()
                .nth(index as usize)
                .expect(&format!("Couldn't find the {index}th LED segment"));

            similar_asserts::assert_eq!(
                expected.start_position(),
                computed.start_position(),
                "The start position of the first LED in the LedGroups should match"
            );
            similar_asserts::assert_eq!(&expected, computed, $description);
        })*
    };
}
