mod buttons;
mod display;
mod rpm_leds;

pub use buttons::LmxButtonPlate;
pub use display::USBD480Display;
pub use rpm_leds::LmxRpmLeds;

pub struct LmxWheel {
    buttons: LmxButtonPlate,
    display: USBD480Display,
    rpm_leds: LmxRpmLeds,
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
        let rpm_leds = LmxRpmLeds::open(&hidapi)?;

        Ok(Self {
            buttons,
            display,
            rpm_leds,
        })
    }

    pub fn buttons(&self) -> &LmxButtonPlate {
        &self.buttons
    }

    pub fn rpm_leds_mut(&mut self) -> &mut LmxRpmLeds {
        &mut self.rpm_leds
    }

    pub fn display(&self) -> &USBD480Display {
        &self.display
    }
}
