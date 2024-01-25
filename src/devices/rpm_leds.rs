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

use anyhow::{Context as _, Result};
use hidapi::{HidApi, HidDevice};
use strum::{EnumIter, IntoEnumIterator};

pub struct LmxRpmLeds {
    inner: HidDevice,
    led_segments: [LedSegment; 4],
}

#[derive(Debug, Clone, Copy, EnumIter)]
enum Led {
    One = 2,
    Two = 6,
    Three = 10,
    Four = 14,
}

struct LedSegment {
    buffer: [u8; 21],
}

impl LedSegment {
    const BYTES_PER_LED: usize = 4;

    pub fn new(segment_id: u8) -> Self {
        let mut buffer = [0u8; 21];
        buffer[1] = segment_id;

        Self { buffer }
    }

    pub fn set_color(&mut self, led: Led, r: u8, g: u8, b: u8, brightness: u8) {
        let slice = &mut self.buffer[led as usize..led as usize + Self::BYTES_PER_LED];

        slice[0] = r;
        slice[1] = g;
        slice[2] = b;
        // Brightness
        slice[3] = brightness
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }
}

impl LmxRpmLeds {
    const VID: u16 = 0x04d8;
    const PID: u16 = 0x32af;

    const LED_COUNT: u32 = 16;

    pub fn open(hidapi: &HidApi) -> Result<Self> {
        let inner = hidapi
            .open(Self::VID, Self::PID)
            .context("Could not open the LM-X RPM LEDs")?;

        Ok(Self {
            inner,
            led_segments: [
                LedSegment::new(0x02),
                LedSegment::new(0x03),
                LedSegment::new(0x07),
                LedSegment::new(0x08),
            ],
        })
    }

    fn commit(&self) -> Result<()> {
        // Data for the LED commit command.
        const COMMIT_COMMAND: &'static [u8] = &[
            0x00, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        self.inner.send_feature_report(COMMIT_COMMAND)?;

        Ok(())
    }

    pub fn gradient(&mut self, percentage: u8) -> Result<()> {
        let active_led_count =
            (((Self::LED_COUNT + 1) * percentage as u32) as f64 / 100.0).floor() as u32;

        let mut i = 0;
        let gradient = colorgrad::CustomGradient::new()
            .domain(&[0.0, 1.0])
            .html_colors(&["blue", "purple"])
            .build()?;

        for segment in &mut self.led_segments {
            for led in Led::iter() {
                let brightness = if i >= active_led_count { 0x00 } else { 0x01 };
                let percentage: f64 = (i + 1) as f64 / Self::LED_COUNT as f64;

                let [r, g, b, _] = gradient.at(percentage).to_rgba8();

                segment.set_color(led, r, g, b, brightness);

                i += 1;
            }

            self.inner.send_feature_report(segment.as_bytes())?;
        }

        self.commit()?;

        Ok(())
    }

    fn turn_off(&mut self) -> Result<()> {
        for segment in &mut self.led_segments {
            for led in Led::iter() {
                segment.set_color(led, 0x00, 0x00, 0x00, 0x00);
            }

            self.inner.send_feature_report(segment.as_bytes())?;
        }

        self.commit()?;

        Ok(())
    }

    pub fn test(&mut self) -> Result<()> {
        self.turn_off()?;

        let mut percentage: u8 = 0;

        loop {
            percentage += 1;
            percentage %= 100;

            self.gradient(percentage)?;

            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
