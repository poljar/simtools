use anyhow::{Context as _, Result};
use hidapi::{HidApi, HidDevice};

pub struct LmxButtonPlate {
    inner: HidDevice,
}

impl LmxButtonPlate {
    const VID: u16 = 0x5758;
    const PID: u16 = 0xffff;

    pub fn open(hidapi: &HidApi) -> Result<Self> {
        let inner = hidapi
            .open(Self::VID, Self::PID)
            .context("Could not open the LM-X Wheel")?;

        Ok(Self { inner })
    }

    #[inline(always)]
    fn calculate_checksum(bytes: &[u8]) -> u8 {
        let sum: u32 = bytes.iter().map(|&byte| byte as u32).sum();
        (sum % 256) as u8
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8) -> Result<()> {
        // Repord ID ?    ?     ?  LED Nr. ?     Br   B  G  R   ?     ?   Checksum
        let mut package = [
            0x00, 0xff, 0xaa, 0x43, 0x01, 0x02, 0xe1, b, g, r, 0x00, 0x00, 0x00,
        ];

        let checksum = Self::calculate_checksum(&package[..package.len() - 1]);
        package[package.len() - 1] = checksum;

        self.inner.write(&package).context(
            "Couldn't send the HID package to set the RGB color of a button to the device",
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn checksum() {
        const EXAMPLE_PAYLOAD: &[u8] = b"\xff\xaaC\x01\x02\xe3\xff\x05\xff\x00\x01\xd6";

        let checksum =
            LmxButtonPlate::calculate_checksum(&EXAMPLE_PAYLOAD[..EXAMPLE_PAYLOAD.len() - 1]);

        assert_eq!(
            checksum,
            EXAMPLE_PAYLOAD[EXAMPLE_PAYLOAD.len() - 1],
            "The calculated checksum should match the one from the example"
        );
    }
}
