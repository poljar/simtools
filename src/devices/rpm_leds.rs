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

use std::num::NonZeroUsize;

use anyhow::{Context as _, Result};
use csscolorparser::Color;
use hidapi::{HidApi, HidDevice};
use simetry::assetto_corsa_competizione::Client;
use strum::{EnumIter, IntoEnumIterator};

use crate::led::state::{rpm_gradient::RpmLedState, LedConfiguration, LedState};

pub struct LmxRpmLeds {
    device: HidDevice,
    leds: [u8; 21 * 4],
}

#[derive(Debug, Clone, Copy, EnumIter)]
pub enum LedNumber {
    One = 2,
    Two = 6,
    Three = 10,
    Four = 14,
}

pub struct Led<'a> {
    buffer: &'a mut [u8],
}

impl<'a> Led<'a> {
    fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer }
    }

    pub fn set_color(&mut self, color: &Color) {
        let [r, g, b, _] = color.to_rgba8();

        self.buffer[0] = r;
        self.buffer[1] = g;
        self.buffer[2] = b;
    }

    pub fn set_brightness(&mut self, brightness: u8) {
        self.buffer[3] = brightness;
    }
}

pub struct LedSegment<'a> {
    buffer: &'a mut [u8],
    device: &'a HidDevice,
}

impl<'a> LedSegment<'a> {
    const BYTES_PER_LED: usize = 4;

    pub fn get_led(&mut self, led: LedNumber) -> Led {
        let buffer = &mut self.buffer[led as usize..led as usize + Self::BYTES_PER_LED];

        Led { buffer }
    }

    #[allow(dead_code)]
    pub fn leds(&mut self) -> impl Iterator<Item = Led> {
        self.buffer[2..]
            .chunks_exact_mut(Self::BYTES_PER_LED)
            .map(Led::new)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.buffer
    }

    pub fn commit_segment(&self) -> Result<()> {
        self.device
            .send_feature_report(self.as_bytes())
            .with_context(|| {
                let segment_id = self.buffer[1];

                format!("Could not commit the LED segment {segment_id:x}")
            })
    }
}

impl LmxRpmLeds {
    const VID: u16 = 0x04d8;
    const PID: u16 = 0x32af;

    const COMMAND_BUFFER_SIZE: usize = 21;
    const SEGMENT_COUNT: usize = 4;

    pub fn open(hidapi: &HidApi) -> Result<Self> {
        let inner = hidapi
            .open(Self::VID, Self::PID)
            .context("Could not open the LM-X RPM LEDs")?;

        let mut leds = [0u8; Self::COMMAND_BUFFER_SIZE * Self::SEGMENT_COUNT];

        for (i, chunk) in leds.chunks_exact_mut(Self::COMMAND_BUFFER_SIZE).enumerate() {
            let segment_id = match i {
                0 => 0x02,
                1 => 0x03,
                2 => 0x07,
                3 => 0x08,
                _ => unreachable!("We should only have 4 LED segments"),
            };

            chunk[1] = segment_id;
        }

        Ok(Self {
            device: inner,
            leds,
        })
    }

    fn commit(&self) -> Result<()> {
        // Data for the LED commit command.
        const COMMIT_COMMAND: &[u8] = &[
            0x00, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        self.device
            .send_feature_report(COMMIT_COMMAND)
            .context("Could not commit the new LED data")?;

        Ok(())
    }

    fn turn_off(&mut self) -> Result<()> {
        let segments = self.segments();

        for mut segment in segments {
            for led in LedNumber::iter() {
                let mut led = segment.get_led(led);

                led.set_color(&Color::from_rgba8(0x00, 0x00, 0x00, 0x00));
                led.set_brightness(0x00);
            }

            segment.commit_segment()?;
        }

        self.commit()?;

        Ok(())
    }

    pub fn segments(&mut self) -> impl Iterator<Item = LedSegment> {
        self.leds.chunks_exact_mut(21).map(|buffer| LedSegment {
            buffer,
            device: &self.device,
        })
    }

    pub fn leds(&mut self) -> impl Iterator<Item = Led> {
        self.leds
            .chunks_exact_mut(21)
            .flat_map(|segment| segment[2..].chunks_exact_mut(LedSegment::BYTES_PER_LED))
            .map(Led::new)
    }

    pub fn apply_led_state(&mut self, start_led: NonZeroUsize, led_state: &LedState) -> Result<()> {
        let start_led = start_led.get();

        for (mut led, led_config) in self.leds().skip(start_led - 1).zip(&led_state.leds) {
            // TODO: Don't hardcode the brightness here.
            match led_config {
                LedConfiguration::On { color } => {
                    led.set_color(&color);
                    led.set_brightness(0x02);
                }
                LedConfiguration::Off => led.set_brightness(0x00),
            }
        }

        for segment in self.segments() {
            segment
                .commit_segment()
                .context("Could not commit a LED segment while applying a new LED state")?;
        }

        self.commit()
            .context("Could not commit the new LED data after applying a new LED state")?;

        Ok(())
    }

    pub async fn run_led_profile(&mut self, mut led_state: RpmLedState) -> Result<()> {
        println!(
            "Running RPM based LED configuration:\n\t{}",
            led_state.container().description
        );

        self.turn_off()
            .context("Could not turn off the RPM LEDs to go back to the initial state")?;

        let mut client = Client::try_connect()
            .await
            .context("Could not connect to the Assetto Corsa Competizione SHM file")?;

        while let Some(sim_state) = client.next_sim_state().await {
            led_state.update(&sim_state);

            self.apply_led_state(led_state.container().start_position, led_state.state())
                .context("Could not apply the new LED state")?;
        }

        Ok(())
    }
}
