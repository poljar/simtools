use std::{ffi::CStr, iter, time::Duration};

use anyhow::Result;
use cairo::{Format, ImageSurface};
use clap::{Parser, Subcommand};
use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{iso_8859_16::FONT_10X20, MonoTextStyle},
    pixelcolor::{raw::RawU16, BinaryColor, Rgb565},
    prelude::*,
    primitives::{
        Circle, Primitive, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment,
        Triangle,
    },
    text::{Alignment, Text},
    Drawable, Pixel,
};
use itertools::Itertools;
use rusb::{
    request_type, Context, DeviceHandle, Direction, Error, Recipient, RequestType, UsbContext,
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
    SetBrightness { brightness: u8 },
}

#[derive(Clone, Debug)]
pub struct DeviceDetails {
    pub name: String,
    pub display_width: u16,
    pub display_height: u16,
    pub version: u16,
    pub serial_number: String,
}

impl DeviceDetails {
    fn from_bytes(bytes: &[u8; 64]) -> Result<Self> {
        let name = CStr::from_bytes_until_nul(&bytes[0..20])?
            .to_string_lossy()
            .to_string();
        let display_width = u16::from_le_bytes([bytes[20], bytes[21]]);
        let display_height = u16::from_le_bytes([bytes[22], bytes[23]]);
        let version = u16::from_le_bytes([bytes[24], bytes[25]]);
        let serial_number = String::from_utf8_lossy(&bytes[26..36]).to_string();

        Ok(Self {
            name,
            display_width,
            display_height,
            version,
            serial_number,
        })
    }
}

struct USBD480Display {
    handle: DeviceHandle<Context>,
}

impl USBD480Display {
    /// The number horizontal pixels the screen contains.
    pub const WIDTH: u32 = 480;
    /// The number vertical pixels the screen contains.
    pub const HEIGHT: u32 = 272;

    /// The USB vendor ID of the display.
    const DISPLAY_VID: u16 = 0x16c0;
    /// The USB product ID of the display.
    const DISPLAY_PID: u16 = 0x08a6;

    /// The command identifier of the WRITE command for the stream decoder.
    const WRITE_COMMAND: u16 = 0x5B41;
    /// The endpoint number of the bulk endpoint the stream decoder is using.
    const BULK_ENDPOINT: u8 = 2;

    /// The default timeout that is used for USB requests.
    const REQUEST_TIMEOUT: Duration = Duration::from_millis(100);

    /// Try to find a USBD480 display connected via USB.
    ///
    /// This will use the first such display that is found, other displays will be ignored.
    pub fn open(context: &mut Context) -> Result<Self> {
        for device in context.devices()?.iter() {
            let device_desc = device.device_descriptor()?;

            if device_desc.vendor_id() == Self::DISPLAY_VID
                && device_desc.product_id() == Self::DISPLAY_PID
            {
                let handle = device.open()?;
                let display = Self { handle };

                display.enable_stream_decoder()?;
                display.set_wrap_length(Self::WIDTH as u16)?;

                return Ok(display);
            }
        }

        Err(Error::NoDevice.into())
    }

    pub fn get_device_details(&self) -> Result<DeviceDetails> {
        const GET_DEVICE_DETAILS: u8 = 0x80;

        let request_type = request_type(Direction::In, RequestType::Vendor, Recipient::Device);
        let mut data = [0u8; 64]; // Buffer for the device details

        self.handle.read_control(
            request_type,
            GET_DEVICE_DETAILS,
            0,
            0,
            &mut data,
            Self::REQUEST_TIMEOUT,
        )?;

        DeviceDetails::from_bytes(&data)
    }

    fn get_config_value(&self) -> Result<()> {
        const GET_CONFIG_VALUE: u8 = 0x83;
        let parameter_id = 20;

        let request_type = request_type(Direction::In, RequestType::Vendor, Recipient::Device);
        let mut data = [0u8; 1];

        self.handle.read_control(
            request_type,
            GET_CONFIG_VALUE,
            parameter_id,
            0,
            &mut data,
            Self::REQUEST_TIMEOUT,
        )?;

        println!("Helloo {data:#?}");

        Ok(())
    }

    pub fn set_brightness(&self, brightness: u8) -> Result<()> {
        const SET_BRIGHTNESS: u8 = 0x81;

        let request_type = request_type(Direction::Out, RequestType::Vendor, Recipient::Device);

        self.handle.write_control(
            request_type,
            SET_BRIGHTNESS,
            brightness as u16,
            0,
            &[],
            Self::REQUEST_TIMEOUT,
        )?;

        Ok(())
    }

    /// Enable the stream decoder.
    ///
    /// The stream decoder allows controlling the basic display functionality using the bulk USB
    /// endpoint for data transfers.
    ///
    /// The stream decoder has three subcommands:
    /// 1. WRITE - Write image data in the RGB565 format to the framebuffer of the display.
    /// 2. FRAMEBASE - Set the start address for the visible frame. The display will read 480x272
    ///    pixels starting from this address and display them on the screen. The address change is
    ///    synchronized with the display refresh VSYNC.
    /// 3. WRAPLENGTH - This controls when the WRITE command should automatically move to the next
    ///    row, i.e. when this is set to 480 (the default value) the WRITE command will fill out a
    ///    whole row on the screen, if it's instead set to 240, the same WRITE command will fill
    ///    out two rows but only half of the screen width.
    fn enable_stream_decoder(&self) -> Result<()> {
        /// The `bRequest` value for the SET_STREAM_DECODER vendor request.
        const SET_STREAM_DECODER: u8 = 0xC6;

        // The wValue of the SET_STREAM_DECODER vendor request. This either enables or disables
        // the stream decoder. `0x06` is to enable the stream decoder, `0x0` to disable it.
        let mode: u16 = 0x06;
        let request_type = request_type(Direction::Out, RequestType::Vendor, Recipient::Device);

        self.handle.write_control(
            request_type,
            SET_STREAM_DECODER,
            mode,
            // wIndex is defined to be 0.
            0,
            // No data is returned by this request.
            &[],
            Self::REQUEST_TIMEOUT,
        )?;

        Ok(())
    }

    /// Write the given command to the bulk endpoint of the stream decoder.
    ///
    /// The command needs to be a valid stream decoder command, one of WRITE, WRAPLENGTH, or
    /// FRAMEBASE.
    fn write_to_bulk_endpoint(&self, command: &[u8]) -> Result<usize> {
        Ok(self
            .handle
            .write_bulk(Self::BULK_ENDPOINT, &command, Self::REQUEST_TIMEOUT)?)
    }

    /// Set the wrap length of the stream decoder to the given length.
    ///
    /// When writing data to the framebuffer the WRITE will automatically wrap to the next row
    /// after WRAPLENGTH number of pixels.
    fn set_wrap_length(&self, length: u16) -> Result<()> {
        /// The command identifier for the WRAPLENGTH stream encoder command.
        const WRAPLENGTH_COMMAND: u16 = 0x5B43;

        // The wrap length is defined in the spec to be the actual length - 1. A value of 479
        // means the full screen width, 0 is 1 pixel wide.
        let wrap_length_minus_one = length - 1;

        let mut command = vec![];

        command.extend_from_slice(&WRAPLENGTH_COMMAND.to_le_bytes());
        command.extend_from_slice(&wrap_length_minus_one.to_le_bytes());

        self.write_to_bulk_endpoint(&command)?;

        Ok(())
    }

    /// Write a single RGB565 pixel to the screen.
    fn write_pixel(&self, pixel: Pixel<Rgb565>) -> Result<()> {
        let Pixel(point, color) = pixel;

        let address: u32 = point.y as u32 * 480 + point.x as u32;
        let color = RawU16::from(color).into_inner();
        let data_len: u32 = 0;

        let mut command = Vec::with_capacity(16);

        command.extend_from_slice(&Self::WRITE_COMMAND.to_le_bytes());
        command.extend_from_slice(&address.to_le_bytes());
        command.extend_from_slice(&data_len.to_le_bytes());
        command.extend_from_slice(&color.to_le_bytes());

        self.write_to_bulk_endpoint(&command)?;

        Ok(())
    }

    fn write_bytes(&self, pixels: &[u8]) -> Result<()> {
        const MAX_PACKET_SIZE: usize = 4096;

        let mut current_address: u32 = 0;
        let mut command = Vec::with_capacity(MAX_PACKET_SIZE);

        for chunk in pixels.chunks(MAX_PACKET_SIZE) {
            let pixel_count = (chunk.len() / 2) as u32 - 1;
            let address = current_address.to_le_bytes();

            command.extend_from_slice(&Self::WRITE_COMMAND.to_le_bytes());
            command.extend_from_slice(&address);
            command.extend_from_slice(&pixel_count.to_le_bytes());
            command.extend_from_slice(&chunk);

            self.write_to_bulk_endpoint(&command)?;

            command.clear();

            current_address += pixel_count as u32 + 1;
        }

        Ok(())
    }

    fn write_pixels_contiguous(
        &self,
        area: &Rectangle,
        pixels: impl Iterator<Item = u8>,
    ) -> Result<()> {
        const MAX_PACKET_SIZE: usize = 4096;

        let top_left = area.top_left;
        let mut current_address = top_left.y as u32 * 480 + top_left.x as u32;
        let mut command = Vec::with_capacity(MAX_PACKET_SIZE);

        for chunk in &pixels.chunks(MAX_PACKET_SIZE) {
            let chunk = chunk.collect_vec();

            let pixel_count = (chunk.len() / 2) as u32 - 1;
            let address = current_address.to_le_bytes();

            command.extend_from_slice(&Self::WRITE_COMMAND.to_le_bytes());
            command.extend_from_slice(&address);
            command.extend_from_slice(&pixel_count.to_le_bytes());
            command.extend_from_slice(&chunk);

            self.write_to_bulk_endpoint(&command)?;

            command.clear();

            current_address += pixel_count as u32 + 1;
        }

        Ok(())
    }
}

impl OriginDimensions for USBD480Display {
    fn size(&self) -> Size {
        Size {
            width: Self::WIDTH,
            height: Self::HEIGHT,
        }
    }
}

impl DrawTarget for USBD480Display {
    type Color = embedded_graphics::pixelcolor::Rgb565;

    type Error = anyhow::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> std::result::Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            self.write_pixel(pixel)?;
        }

        Ok(())
    }

    fn fill_contiguous<C>(&mut self, area: &Rectangle, colors: C) -> Result<(), Self::Error>
    where
        C: IntoIterator<Item = Self::Color>,
    {
        let drawable_area = area.intersection(&Rectangle::new(Point::zero(), self.size()));

        if drawable_area.is_zero_sized() {
            return Ok(());
        }

        let pixels_inside_drawable_area = area
            .points()
            .zip(colors)
            .filter(|(pos, _)| drawable_area.contains(*pos))
            .map(|(_, color)| RawU16::from(color).into_inner().to_le_bytes())
            .flatten();

        let width = area.size.width as u16;
        let mut current_address = (area.top_left.y as u32 * 480) + area.top_left.x as u32;

        // Set the wrap length to the width of the area, this ensures that we can just write the
        // pixels to the framebuffer in a coniguous manner, the display will ensure that we go to
        // the next row when we have written a `width` number of pixels.
        self.set_wrap_length(width)?;

        let mut command = Vec::with_capacity(512);

        for chunk in &pixels_inside_drawable_area.chunks(502) {
            let chunk = chunk.collect_vec();

            let address = current_address.to_le_bytes();

            let pixel_count = (chunk.len() / 2) as u32 - 1;
            let row_count = (pixel_count + 1) / width as u32;
            let remainder = (pixel_count + 1) % width as u32;

            let old_remainder = current_address % width as u32;
            let address_without_remainder = current_address - old_remainder;

            let new_remainder = (remainder + old_remainder) % width as u32;
            let remainder_row = (remainder + old_remainder) / width as u32;

            let new_row_count = row_count + remainder_row;
            let new_address =
                address_without_remainder + (new_row_count * Self::WIDTH) + (new_remainder);

            command.extend_from_slice(&Self::WRITE_COMMAND.to_le_bytes());
            command.extend_from_slice(&address);
            command.extend_from_slice(&pixel_count.to_le_bytes());
            command.extend_from_slice(&chunk);

            if let Err(e) = self.write_to_bulk_endpoint(&command) {
                self.set_wrap_length(Self::WIDTH as u16)?;
                return Err(e);
            }

            command.clear();

            current_address = new_address;
        }

        self.set_wrap_length(Self::WIDTH as u16)?;

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> std::result::Result<(), Self::Error> {
        let Size { width, height } = self.size();
        let drawable_area = Rectangle::new(Point::zero(), self.size());

        let pixels = iter::repeat(RawU16::from(color).into_inner().to_le_bytes())
            .flatten()
            .take((width * height * 2) as usize);

        self.write_pixels_contiguous(&drawable_area, pixels)?;

        Ok(())
    }
}

#[allow(dead_code)]
fn draw(display: &mut USBD480Display) -> Result<()> {
    // Create styles used by the drawing operations.
    let thin_stroke = PrimitiveStyle::with_stroke(Rgb565::from(BinaryColor::On), 1);
    let thick_stroke = PrimitiveStyle::with_stroke(Rgb565::from(BinaryColor::On), 3);
    let border_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On.into())
        .stroke_width(3)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();
    let fill = PrimitiveStyle::with_fill(Rgb565::from(BinaryColor::On));
    let character_style = MonoTextStyle::new(&FONT_10X20, Rgb565::from(BinaryColor::On));

    let yoffset = 10;

    display.clear(Rgb565::new(100, 50, 50))?;

    // Draw a 3px wide outline around the display.
    display
        .bounding_box()
        .into_styled(border_stroke)
        .draw(display)?;

    // Draw a triangle.
    Triangle::new(
        Point::new(16, 16 + yoffset),
        Point::new(16 + 16, 16 + yoffset),
        Point::new(16 + 8, yoffset),
    )
    .into_styled(thin_stroke)
    .draw(display)?;

    // Draw a filled square
    Rectangle::new(Point::new(52, yoffset), Size::new(16, 16))
        .into_styled(fill)
        .draw(display)?;

    // Draw a circle with a 3px wide stroke.
    Circle::new(Point::new(88, yoffset), 17)
        .into_styled(thick_stroke)
        .draw(display)?;

    // Draw centered text.
    let text = "It works";
    Text::with_alignment(
        text,
        display.bounding_box().center() + Point::new(0, 15),
        character_style,
        Alignment::Center,
    )
    .draw(display)?;

    Ok(())
}

fn draw_letter(display: &mut USBD480Display) -> Result<()> {
    let Size { width, height } = display.size();

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
    let mut context = Context::new()?;
    let mut display = USBD480Display::open(&mut context)?;

    let cli = Cli::parse();

    match cli.command {
        CliCommand::Draw => {
            draw_letter(&mut display)?;
        }
        CliCommand::ShowDeviceDetails => {
            let device_details = display.get_device_details()?;
            println!("Got device details: {device_details:#?}");
        }
        CliCommand::SetBrightness { brightness } => {
            display.set_brightness(brightness)?;
        }
        CliCommand::GetConfigValue => {
            display.get_config_value()?;
        }
    }

    Ok(())
}
