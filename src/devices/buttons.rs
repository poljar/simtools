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

use anyhow::{Context as _, Result};
use hidapi::{HidApi, HidDevice};

pub struct LmxButtonPlate {
    inner: HidDevice,
}

impl LmxButtonPlate {
    const VID: u16 = 0x5758;
    const PID: u16 = 0xffff;

    pub fn open(hidapi: &HidApi) -> Result<Self> {
        let inner = hidapi.open(Self::VID, Self::PID).context("Could not open the LM-X Wheel")?;

        Ok(Self { inner })
    }

    #[inline(always)]
    fn calculate_checksum(bytes: &[u8]) -> u8 {
        let sum: u32 = bytes.iter().map(|&byte| byte as u32).sum();
        (sum % 256) as u8
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8) -> Result<()> {
        // Repord ID ?    ?     ?  LED Nr. ?     Br   B  G  R   ?     ?   Checksum
        let mut package = [0x00, 0xff, 0xaa, 0x43, 0x01, 0x02, 0xe1, b, g, r, 0x00, 0x00, 0x00];

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
    use similar_asserts::assert_eq;

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
