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

use std::{fs::File, io::BufReader, path::PathBuf, time::Duration};

use anyhow::{Context as _, Result};
use cairo::{Format, ImageSurface};
use clap::{Parser, Subcommand};
use simetry::{Moment, RacingFlags};
use simtools::{
    devices::{GridBrows, LmxWheel, USBD480Display},
    led::{
        effects::{groups::EffectGroup, LedEffect},
        profiles::LedProfile,
    },
};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    Draw,
    ShowDeviceDetails,
    GetConfigValue,
    SetButtonColor { red: u8, green: u8, blue: u8 },
    SetBrightness { brightness: u8 },
    RpmTest { profile: PathBuf },
}

fn draw_letter(display: &USBD480Display) -> Result<()> {
    let width = USBD480Display::WIDTH;
    let height = USBD480Display::HEIGHT;

    let mut surface = ImageSurface::create(Format::Rgb16_565, width as i32, height as i32)
        .expect("Can't create surface with RGB565 format");

    {
        let context = cairo::Context::new(&surface)?;
        context.set_source_rgb(1.0, 1.0, 1.0); // white
        context.paint()?; // fill the surface with the background color

        {
            // Set up font, size, and text color
            context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            context.set_font_size(200.0); // Adjust size as needed
            context.set_source_rgb(0.0, 0.0, 0.0); // black for the text

            // Calculate position for centering the text
            let text = "Hana";
            let extents = context.text_extents(text)?;
            let x = (width as f64 - extents.width()) / 2.0 - extents.x_bearing();
            let y = (height as f64 - extents.height()) / 2.0 - extents.y_bearing();

            // Draw text
            context.move_to(x, y);
            context.show_text(text)?;
        }
    }

    let data = surface.data()?;
    display.write_bytes(&data)?;

    Ok(())
}

async fn foo(mut device: GridBrows, mut something: EffectGroup) -> Result<()> {
    #[derive(Debug, Default)]
    struct Foo {
        yellow: bool,
    }

    impl Foo {
        fn update(&mut self) {
            self.yellow = !self.yellow;
        }
    }

    impl Moment for Foo {
        fn flags(&self) -> Option<simetry::RacingFlags> {
            Some(RacingFlags { yellow: self.yellow, ..Default::default() })
        }
    }

    println!("Running RPM based LED configuration:\n\t",);

    let mut foo = Foo::default();

    loop {
        something.update(&foo);

        for state in something.leds() {
            device.apply_led_state(state).context("Could not apply the new LED state")?;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;

        foo.update();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // let mut lmx = LmxWheel::open()?;
    let mut brows = GridBrows::open()?;

    match cli.command {
        // CliCommand::Draw => {
        //     draw_letter(lmx.display())?;
        // }
        // CliCommand::ShowDeviceDetails => {
        //     let device_details = lmx.display().get_device_details()?;
        //     println!("Got device details: {device_details:#?}");
        // }
        // CliCommand::SetBrightness { brightness } => {
        //     lmx.display().set_brightness(brightness)?;
        // }
        // CliCommand::GetConfigValue => {
        //     lmx.display().get_config_value()?;
        // }
        // CliCommand::SetButtonColor { red, green, blue } => {
        //     lmx.buttons().set_color(red, green, blue)?;
        // }
        CliCommand::RpmTest { profile } => {
            let profile = File::open(profile).context("Couldn't open the LED profile")?;
            let reader = BufReader::new(profile);

            let profile: LedProfile =
                serde_json::from_reader(reader).context("Could not deserialize the LED profile")?;
            let root_group = EffectGroup::root(profile);

            foo(brows, root_group).await?;
        }

        _ => unreachable!(),
    }

    Ok(())
}
