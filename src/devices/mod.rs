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

mod buttons;
mod display;
mod leds;

pub use buttons::LmxButtonPlate;
pub use display::USBD480Display;
pub use leds::LmxLeds;

pub struct LmxWheel {
    buttons: LmxButtonPlate,
    display: USBD480Display,
    rpm_leds: LmxLeds,
}

impl LmxWheel {
    pub fn open() -> anyhow::Result<Self> {
        use anyhow::Context as _;
        use hidapi::HidApi;
        use rusb::Context;

        let context = Context::new()?;
        let hidapi = HidApi::new().context("Could not create a HidApi object")?;

        let display = USBD480Display::open(&context)?;
        let buttons = LmxButtonPlate::open(&hidapi)?;
        let rpm_leds = LmxLeds::open(&hidapi)?;

        Ok(Self {
            buttons,
            display,
            rpm_leds,
        })
    }

    pub fn buttons(&self) -> &LmxButtonPlate {
        &self.buttons
    }

    pub fn rpm_leds_mut(&mut self) -> &mut LmxLeds {
        &mut self.rpm_leds
    }

    pub fn display(&self) -> &USBD480Display {
        &self.display
    }
}
