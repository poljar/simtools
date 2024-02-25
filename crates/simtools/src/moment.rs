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

use simetry::Moment;

/// Extension trait for the [`Moment`] trait.
///
/// This extension trait contains values that get calculated from the values
/// contained in the [`Moment`] trait.
pub trait MomentExt: Moment {
    fn redline_reached(&self) -> bool {
        const ERROR_MARGIN_PERCENTAGE: f64 = 0.02;

        let Some(rpm) = self.vehicle_engine_rotation_speed() else {
            return false;
        };

        let Some(max_rpm) = self.vehicle_max_engine_rotation_speed() else {
            return false;
        };

        let error_margin = ERROR_MARGIN_PERCENTAGE * max_rpm;

        // TODO: Add an optional argument that contains a per car and per gear DB of
        // redlines or rather ideal shiftpoints.

        // If we're within 2% of the MAX RPM of a car, we're going to consider this to
        // be at the redline.
        (max_rpm - rpm).abs() < error_margin
    }

    fn is_engine_running(&self) -> bool {
        let Some(is_starting) = self.is_starter_on() else {
            return false;
        };

        let Some(rpm) = self.vehicle_engine_rotation_speed() else {
            return false;
        };

        !is_starting && rpm.value > 0.0
    }
}

impl<T> MomentExt for T where T: Moment + ?Sized {}
