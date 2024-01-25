use anyhow::Result;
use cairo::{Format, ImageSurface};
use clap::{Parser, Subcommand};

use devices::{LmxWheel, USBD480Display};

mod devices;
mod led_profile;

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
    RpmTest,
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

fn main() -> Result<()> {
    let mut lmx = LmxWheel::open()?;

    let cli = Cli::parse();

    match cli.command {
        CliCommand::Draw => {
            draw_letter(lmx.display())?;
        }
        CliCommand::ShowDeviceDetails => {
            let device_details = lmx.display().get_device_details()?;
            println!("Got device details: {device_details:#?}");
        }
        CliCommand::SetBrightness { brightness } => {
            lmx.display().set_brightness(brightness)?;
        }
        CliCommand::GetConfigValue => {
            lmx.display().get_config_value()?;
        }
        CliCommand::SetButtonColor { red, green, blue } => {
            lmx.buttons().set_color(red, green, blue)?;
        }
        CliCommand::RpmTest => lmx.rpm_leds_mut().test()?,
    }

    Ok(())
}
