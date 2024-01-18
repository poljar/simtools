use std::{ffi::CStr, time::Duration};

use anyhow::Result;
use clap::{Parser, Subcommand};
use rusb::{
    request_type, Context, DeviceHandle, Direction, Error, Recipient, RequestType, UsbContext,
};

const DISPLAY_VID: u16 = 0x16c0;
const DISPLAY_PID: u16 = 0x08a6;

const REQUEST_TIMEOUT: Duration = Duration::from_millis(100);

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
    pub fn open(context: &mut Context) -> Result<Self> {
        for device in context.devices()?.iter() {
            let device_desc = device.device_descriptor()?;

            if device_desc.vendor_id() == DISPLAY_VID && device_desc.product_id() == DISPLAY_PID {
                let handle = device.open()?;
                return Ok(Self { handle });
            }
        }

        Err(Error::NoDevice.into())
    }

    fn enable_stream_decoder(&self) -> Result<()> {
        const SET_STREAM_DECODER: u8 = 0xC6;
        let mode = 0x06;
        let request_type = request_type(Direction::Out, RequestType::Vendor, Recipient::Device);

        self.handle.write_control(
            request_type,
            SET_STREAM_DECODER,
            mode as u16,
            0,
            &[],
            REQUEST_TIMEOUT,
        )?;

        Ok(())
    }

    fn write_image_data(&self, data: &[u8]) -> Result<(), Error> {
        const WRITE_COMMAND: u16 = 0x5B41;
        const MAX_PACKET_SIZE: usize = 512; // Max packet size for high-speed USB 2.0
        const ENDPOINT: u8 = 2;

        let mut current_address = 0;

        for chunk in data.chunks(MAX_PACKET_SIZE - 10) {
            // 10 bytes reserved for command and address
            let addr_lo = (current_address & 0xFFFF) as u16;
            let addr_hi = ((current_address >> 16) & 0x3F) as u16;
            let data_len_lo = (chunk.len() as u32 & 0xFFFF) as u16;
            let data_len_hi = ((chunk.len() as u32 >> 16) & 0x3F) as u16;

            let mut command = vec![];
            command.extend_from_slice(&WRITE_COMMAND.to_le_bytes());
            command.extend_from_slice(&addr_lo.to_le_bytes());
            command.extend_from_slice(&addr_hi.to_le_bytes());
            command.extend_from_slice(&data_len_lo.to_le_bytes());
            command.extend_from_slice(&data_len_hi.to_le_bytes());
            command.extend_from_slice(chunk);

            self.handle
                .write_bulk(ENDPOINT, &command, REQUEST_TIMEOUT)?;

            // Update current address for next chunk
            current_address += (chunk.len() / 2) as u32; // Divide by 2 because each address holds 2 bytes (RGB565)
        }

        Ok(())
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
            REQUEST_TIMEOUT,
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
            REQUEST_TIMEOUT,
        )?;

        Ok(())
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
            REQUEST_TIMEOUT,
        )?;

        DeviceDetails::from_bytes(&data)
    }
}

// Example usage
fn main() -> Result<()> {
    let mut context = Context::new()?;
    let display = USBD480Display::open(&mut context)?;

    let cli = Cli::parse();

    const DATA: &[u8] = &[0x0Fu8; 261120];

    match cli.command {
        CliCommand::Draw => {
            display.enable_stream_decoder()?;
            display.write_image_data(DATA)?;
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
