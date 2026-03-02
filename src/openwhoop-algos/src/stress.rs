use chrono::NaiveDateTime;
use std::collections::BTreeMap;
use openwhoop_codec::ParsedHistoryReading;

use openwhoop_codec::WhoopError;

pub struct StressCalculator;

#[derive(Debug, Clone, Copy)]
pub struct StressScore {
    pub time: NaiveDateTime,
    pub score: f64,
}

impl StressCalculator {
    pub const MIN_READING_PERIOD: usize = 120;

    pub fn calculate_stress(hr: &[ParsedHistoryReading]) -> Option<StressScore> {
        if hr.len() < Self::MIN_READING_PERIOD {
            return None;
        }

        let time = hr.last()?.time;

        // Prefer real RR intervals from the device
        let real_rr: Vec<u16> = hr.iter().flat_map(|r| r.rr.iter().copied()).collect();

        let rr = if real_rr.len() >= Self::MIN_READING_PERIOD {
            real_rr
        } else {
            // Fall back to BPM-derived RR
            hr.iter()
                .map(|r| (60.0 / f64::from(r.bpm) * 1000.0).round() as u16)
                .collect()
        };

        let score = StressCalcParams::new(rr).ok()?.stress_score();
        Some(StressScore { time, score })
    }
}

#[derive(Debug)]
struct StressCalcParams {
    min: u16,
    max: u16,
    mode: u16,
    mode_freq: u16,
    count: u16,
}

impl StressCalcParams {
    /// Standard 50ms bin width for Baevsky's Stress Index histogram.
    const BIN_WIDTH: u16 = 50;

    fn new(rr: Vec<u16>) -> Result<Self, WhoopError> {
        let count = u16::try_from(rr.len()).map_err(|_| WhoopError::Overflow)?;

        let min = rr.iter().min().copied().unwrap_or_default();
        let max = rr.iter().max().copied().unwrap_or_default();

        // Build histogram with 50ms bins per Baevsky's standard
        let mut bins = BTreeMap::new();
        for &val in &rr {
            let bin = val / Self::BIN_WIDTH;
            *bins.entry(bin).or_insert(0_u16) += 1;
        }

        let (mode_bin, mode_freq) = bins
            .into_iter()
            .max_by(|(_ak, av), (_bk, bv)| av.cmp(bv))
            .unwrap_or_default();

        // Mode is the center of the most frequent bin
        let mode = mode_bin * Self::BIN_WIDTH + Self::BIN_WIDTH / 2;

        Ok(Self {
            min,
            max,
            mode,
            mode_freq,
            count,
        })
    }

    fn stress_score(self) -> f64 {
        let vr = f64::from(self.max - self.min) / 1000_f64;

        // Near-zero variability means the histogram is maximally narrow/tall,
        // indicating high sympathetic dominance - maximum stress.
        if vr < 0.0001 {
            return 10_f64;
        }

        let a_mode = f64::from(self.mode_freq) / f64::from(self.count) * 100_f64;
        (a_mode / (2_f64 * vr * f64::from(self.mode) / 1000_f64))
            .round()
            .min(1000_f64)
            / 100_f64
    }
}

#[cfg(test)]
mod tests {
    use crate::stress::StressCalcParams;
    use crate::StressCalculator;

    #[test]
    fn test_stress_calc_moderate_variability() {
        // RR intervals with moderate variability (530-690ms range, ~87-113 bpm equivalent)
        let rr: Vec<u16> = [
            667, 674, 682, 690, 682, 652, 638, 632, 625, 619, 612, 619, 606, 594, 583, 577, 566,
            561, 561, 556, 556, 550, 556, 556, 556, 556, 550, 550, 545, 541, 531, 531, 531, 531,
            531, 536, 541, 545, 550, 556, 561, 566, 571, 577, 577, 583, 583, 583, 588, 594, 594,
            600, 600, 600, 600, 594, 600, 612, 619, 625, 632, 632, 632, 625, 625, 619, 619, 619,
            612, 606, 594, 600, 600, 600, 600, 606, 606, 606, 606, 600, 606, 612, 612, 612, 612,
            612, 612, 612, 612, 619, 612, 612, 612, 619, 619, 625, 625, 625, 632, 638, 645, 645,
            638, 638, 632, 625, 625, 625, 625, 632, 638, 632, 632, 625, 625, 625, 625, 625, 619,
            612,
        ]
        .to_vec();
        let score = StressCalcParams::new(rr).unwrap().stress_score();
        assert!(score > 0.0, "moderate variability should have some stress: {score}");
        assert!(score <= 10.0, "moderate variability stress should be <= 10: {score}");
    }

    #[test]
    fn test_stress_calc_low_variability() {
        // RR intervals with low variability (923-1017ms range, ~59-65 bpm equivalent)
        let rr: Vec<u16> = [
            1000, 984, 1017, 1017, 1017, 1017, 1017, 1000, 1000, 1000, 1000, 1000, 984, 984, 984,
            984, 984, 984, 984, 984, 952, 952, 952, 952, 938, 952, 952, 952, 968, 968, 968, 968,
            984, 984, 984, 984, 968, 968, 968, 968, 968, 968, 968, 968, 968, 968, 968, 968, 968,
            968, 968, 968, 968, 952, 952, 952, 952, 952, 952, 952, 938, 938, 938, 938, 938, 923,
            923, 938, 938, 938, 938, 938, 938, 938, 938, 938, 938, 938, 938, 923, 923, 923, 938,
            938, 952, 952, 952, 952, 968, 968, 968, 984, 984, 984, 984, 968, 968, 968, 984, 984,
            984, 984, 968, 968, 968, 968, 968, 952, 952, 952, 952, 938, 952, 952, 952, 968, 968,
            952, 952, 952,
        ]
        .to_vec();
        let score = StressCalcParams::new(rr).unwrap().stress_score();
        assert!(score > 0.0, "low variability RR should produce a stress score: {score}");
    }

    #[test]
    fn test_stress_constant_rr_returns_max() {
        // All identical RR -> zero variability -> maximum stress
        let rr = vec![750_u16; 120];
        let score = StressCalcParams::new(rr).unwrap().stress_score();
        assert_eq!(score, 10.0);
    }

    #[test]
    fn test_stress_calculator_too_few_readings() {
        use chrono::NaiveDate;
        use openwhoop_codec::ParsedHistoryReading;

        let base = NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let readings: Vec<ParsedHistoryReading> = (0..50)
            .map(|i| ParsedHistoryReading {
                time: base + chrono::TimeDelta::seconds(i),
                bpm: 80,
                rr: vec![],
                imu_data: None,
                gravity: None,
            })
            .collect();
        assert!(StressCalculator::calculate_stress(&readings).is_none());
    }

    #[test]
    fn test_stress_calculator_sufficient_readings() {
        use chrono::NaiveDate;
        use openwhoop_codec::ParsedHistoryReading;

        let base = NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let readings: Vec<ParsedHistoryReading> = (0..120)
            .map(|i| ParsedHistoryReading {
                time: base + chrono::TimeDelta::seconds(i),
                bpm: 70 + u8::try_from(i % 10).unwrap(),
                rr: vec![],
                imu_data: None,
                gravity: None,
            })
            .collect();
        let result = StressCalculator::calculate_stress(&readings);
        assert!(result.is_some());
        assert!(result.unwrap().score >= 0.0);
    }
}
