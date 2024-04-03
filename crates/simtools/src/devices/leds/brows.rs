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

use std::time::Duration;

use anyhow::{Context as _, Result};
use csscolorparser::Color;
use rusb::{Context, DeviceHandle, Error, UsbContext};
use simetry::assetto_corsa_competizione::Client;

use crate::led::effects::{groups::EffectGroup, LedConfiguration, LedEffect, LedGroup};

const BULK_ENDPOINT: u8 = 0x02;
const USB_TIMEOUT: Duration = Duration::from_millis(5);

pub struct GridBrows {
    handle: DeviceHandle<Context>,
    buffer: [u8; Self::LED_COUNT * Self::COMMAND_BUFFER_SIZE],
}

pub struct Led<'a> {
    handle: &'a DeviceHandle<Context>,
    buffer: &'a mut [u8],
}

impl<'a> Led<'a> {
    fn new(buffer: &'a mut [u8], handle: &'a DeviceHandle<Context>) -> Self {
        Self { buffer, handle }
    }

    pub fn set_color(&mut self, color: &Color) {
        let [r, g, b, _] = color.to_rgba8();

        self.buffer[0] = r;
        self.buffer[1] = g;
        self.buffer[2] = b;
    }

    pub fn set_brightness(&mut self, _: u8) {
        // The Brows don't seem to support setting brighntess.
        ();
    }

    pub fn commit(&self) -> Result<()> {
        self.handle.write_bulk(BULK_ENDPOINT, &self.buffer, USB_TIMEOUT)?;

        Ok(())
    }
}

impl GridBrows {
    const VID: u16 = 0x04d8;
    const PID: u16 = 0xe690;

    const INTERFACE: u8 = 0x00;

    const LED_COUNT: usize = 18;
    const COMMAND_BUFFER_SIZE: usize = 4;

    pub fn open() -> Result<Self> {
        let context = Context::new()?;

        for device in context.devices()?.iter() {
            let device_desc = device.device_descriptor()?;

            if device_desc.vendor_id() == Self::VID && device_desc.product_id() == Self::PID {
                let mut handle = device.open()?;
                handle.set_auto_detach_kernel_driver(true)?;
                handle.claim_interface(Self::INTERFACE)?;

                let mut buffer = [0u8; Self::COMMAND_BUFFER_SIZE * Self::LED_COUNT];

                for (i, chunk) in buffer.chunks_exact_mut(Self::COMMAND_BUFFER_SIZE).enumerate() {
                    chunk[3] = i as u8;
                }

                return Ok(Self { handle, buffer });
            }
        }

        Err(Error::NoDevice.into())
    }

    fn turn_off(&mut self) -> Result<()> {
        for mut led in self.leds() {
            led.set_color(&Color::from_rgba8(0x00, 0x00, 0x00, 0x00));
            led.commit()?;
        }

        self.commit()?;

        Ok(())
    }

    fn commit(&self) -> Result<()> {
        self.handle.write_bulk(BULK_ENDPOINT, &[0x01], USB_TIMEOUT)?;

        Ok(())
    }

    pub fn leds(&mut self) -> impl Iterator<Item = Led> {
        self.buffer
            .chunks_exact_mut(Self::COMMAND_BUFFER_SIZE)
            .map(|buffer| Led::new(buffer, &self.handle))
    }

    pub fn apply_led_state(&mut self, led_state: &LedGroup) -> Result<()> {
        let start_led = led_state.start_position().get();

        for (mut led, led_config) in self.leds().skip(start_led - 1).zip(led_state.leds()) {
            match led_config {
                LedConfiguration::On { color } => {
                    led.set_color(color);
                    led.set_brightness(0x04);
                }
                LedConfiguration::Off => led.set_color(&Color::from_rgba8(0x00, 0x00, 0x00, 0x00)),
            }
        }

        for led in self.leds() {
            led.commit()?;
        }

        self.commit()?;

        Ok(())
    }

    pub async fn run_led_profile(&mut self, mut led_state: EffectGroup) -> Result<()> {
        println!("Running RPM based LED configuration:\n\t",);

        self.turn_off()
            .context("Could not turn off the RPM LEDs to go back to the initial state")?;

        loop {
            let mut client = Client::try_connect()
                .await
                .context("Could not connect to the Assetto Corsa Competizione SHM file")?;

            while let Some(sim_state) = client.next_sim_state().await {
                led_state.update(&sim_state);

                for state in led_state.leds() {
                    self.apply_led_state(state).context("Could not apply the new LED state")?;
                }
            }
        }
    }
}
