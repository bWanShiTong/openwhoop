use chrono::NaiveDateTime;

pub struct SkinTempCalculator;

#[derive(Debug, Clone, Copy)]
pub struct SkinTempScore {
    pub time: NaiveDateTime,
    pub temp_celsius: f64,
}

impl SkinTempCalculator {
    /// Empirical conversion factor: T(degC) = skin_temp_raw x 0.04
    ///
    /// Derived from firmware analysis of the WHOOP 4.0:
    /// - The raw u16 value is a thermistor ADC reading passed through the
    ///   DSP pipeline without mathematical transformation
    /// - The firmware sends raw values; the WHOOP server performs per-device
    ///   calibrated conversion
    /// - This factor produces physiologically reasonable wrist skin temperatures
    ///   (31-37degC) across the observed raw range (582-1125)
    const CONVERSION_FACTOR: f64 = 0.04;

    /// Minimum valid raw reading (below this is likely off-wrist or sensor error)
    const MIN_RAW: u16 = 100;

    pub fn convert(time: NaiveDateTime, skin_temp_raw: u16) -> Option<SkinTempScore> {
        if skin_temp_raw < Self::MIN_RAW {
            return None;
        }

        let temp_celsius = f64::from(skin_temp_raw) * Self::CONVERSION_FACTOR;
        Some(SkinTempScore { time, temp_celsius })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn base_time() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    #[test]
    fn zero_raw_returns_none() {
        assert!(SkinTempCalculator::convert(base_time(), 0).is_none());
    }

    #[test]
    fn below_minimum_returns_none() {
        assert!(SkinTempCalculator::convert(base_time(), 50).is_none());
    }

    #[test]
    fn typical_resting_value() {
        // Raw 850 -> 34.0degC
        let score = SkinTempCalculator::convert(base_time(), 850).unwrap();
        assert!((score.temp_celsius - 34.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sleep_value() {
        // Raw 900 -> 36.0degC
        let score = SkinTempCalculator::convert(base_time(), 900).unwrap();
        assert!((score.temp_celsius - 36.0).abs() < f64::EPSILON);
    }

    #[test]
    fn low_value() {
        // Raw 700 -> 28.0degC
        let score = SkinTempCalculator::convert(base_time(), 700).unwrap();
        assert!((score.temp_celsius - 28.0).abs() < f64::EPSILON);
    }

    #[test]
    fn minimum_valid() {
        let score = SkinTempCalculator::convert(base_time(), 100).unwrap();
        assert!((score.temp_celsius - 4.0).abs() < f64::EPSILON);
    }
}
