use openwhoop_codec::ParsedHistoryReading;

pub struct StrainCalculator {
    pub max_hr: u8,
    pub resting_hr: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct StrainScore(pub f64);

/// WHOOP strain uses Edwards' zone-based TRIMP with Heart Rate Reserve (HRR):
/// 1. HR Reserve = max_hr - resting_hr
/// 2. Classify each HR sample into zone 1-5 based on %HRR
/// 3. Multiply time in each zone by zone weight (1-5)
/// 4. Sum for raw TRIMP score
/// 5. Map to 0-21 using: strain = 21 x ln(TRIMP + 1) / ln(7201)
///
/// Calibration anchor: 24h at max HR (zone 5) -> TRIMP = 7200 -> strain = 21.
impl StrainCalculator {
    /// Minimum number of readings required (10 minutes at 1Hz)
    pub const MIN_READINGS: usize = 600;

    /// Maximum strain value on the WHOOP scale
    const MAX_STRAIN: f64 = 21.0;

    /// ln(7201) - denominator for the log mapping.
    /// 24h at max HR = 24x60x5 = 7200 TRIMP -> ln(7201) anchors strain = 21.
    const LN_7201: f64 = 8.882_643_961_783_384;

    pub fn new(max_hr: u8, resting_hr: u8) -> Self {
        Self { max_hr, resting_hr }
    }

    pub fn calculate(&self, hr: &[ParsedHistoryReading]) -> Option<StrainScore> {
        if hr.len() < Self::MIN_READINGS || self.max_hr <= self.resting_hr {
            return None;
        }

        let sample_duration_min = Self::sample_duration_minutes(hr);
        let hr_reserve = f64::from(self.max_hr) - f64::from(self.resting_hr);
        let trimp = Self::edwards_trimp(hr, self.resting_hr, hr_reserve, sample_duration_min);

        Some(StrainScore(Self::trimp_to_strain(trimp)))
    }

    /// Estimate the sample interval in minutes from the first two readings.
    /// Falls back to 1/60 min (1 second) if only one reading or timestamps match.
    fn sample_duration_minutes(hr: &[ParsedHistoryReading]) -> f64 {
        if hr.len() < 2 {
            return 1.0 / 60.0;
        }
        let dt = (hr[1].time - hr[0].time).num_milliseconds().unsigned_abs();
        if dt == 0 {
            1.0 / 60.0
        } else {
            dt as f64 / 60_000.0
        }
    }

    /// Returns the Edwards zone weight (1-5) based on %HRR, or 0 if below zone 1.
    /// Zones use Heart Rate Reserve: %HRR = (bpm - resting_hr) / hr_reserve x 100
    fn zone_weight(bpm: u8, resting_hr: u8, hr_reserve: f64) -> u8 {
        let pct = (f64::from(bpm) - f64::from(resting_hr)) / hr_reserve * 100.0;
        if pct >= 90.0 {
            5
        } else if pct >= 80.0 {
            4
        } else if pct >= 70.0 {
            3
        } else if pct >= 60.0 {
            2
        } else if pct >= 50.0 {
            1
        } else {
            0
        }
    }

    /// Edwards' TRIMP with HRR zones: sum(duration_min x zone_weight)
    fn edwards_trimp(
        hr: &[ParsedHistoryReading],
        resting_hr: u8,
        hr_reserve: f64,
        sample_duration_min: f64,
    ) -> f64 {
        hr.iter()
            .map(|r| {
                sample_duration_min * f64::from(Self::zone_weight(r.bpm, resting_hr, hr_reserve))
            })
            .sum()
    }

    /// Map raw TRIMP to 0-21 using calibrated log transform.
    /// strain = 21 x ln(TRIMP + 1) / ln(7201)
    fn trimp_to_strain(trimp: f64) -> f64 {
        if trimp <= 0.0 {
            return 0.0;
        }
        let raw = Self::MAX_STRAIN * (trimp + 1.0).ln() / Self::LN_7201;
        // Round to 2 decimal places - sub-centesimal precision is meaningless for strain
        (raw * 100.0).round() / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rand::Rng;

    fn make_readings(avg_bpm: u8, size: usize) -> Vec<ParsedHistoryReading> {
        let base = NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        let mut rng = rand::rng();
        let lo = avg_bpm.saturating_sub(15).max(1);
        let hi = avg_bpm.saturating_add(15);

        (0..size)
            .map(|i| ParsedHistoryReading {
                time: base + chrono::Duration::seconds(i64::try_from(i).unwrap()),
                bpm: rng.random_range(lo..=hi),
                rr: vec![],
                imu_data: None,
                gravity: None,
            })
            .collect()
    }

    fn make_constant_readings(bpm: u8, size: usize) -> Vec<ParsedHistoryReading> {
        let base = NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        (0..size)
            .map(|i| ParsedHistoryReading {
                time: base + chrono::Duration::seconds(i64::try_from(i).unwrap()),
                bpm,
                rr: vec![],
                imu_data: None,
                gravity: None,
            })
            .collect()
    }

    #[test]
    fn too_few_readings_returns_none() {
        let calc = StrainCalculator::new(200, 60);
        let readings = make_readings(80, 500);
        assert!(calc.calculate(&readings).is_none());
    }

    #[test]
    fn invalid_hr_params_returns_none() {
        let readings = make_readings(80, 600);
        assert!(StrainCalculator::new(60, 60).calculate(&readings).is_none());
        assert!(StrainCalculator::new(50, 60).calculate(&readings).is_none());
    }

    #[test]
    fn resting_hr_produces_zero_strain() {
        let calc = StrainCalculator::new(190, 60);
        // 65 bpm -> %HRR = (65-60)/(190-60) = 3.8% -> below zone 1 -> weight 0
        let readings = make_constant_readings(65, 600);
        let strain = calc.calculate(&readings).unwrap().0;
        assert!(
            strain == 0.0,
            "resting HR below zone 1 should yield zero strain, got {}",
            strain
        );
    }

    #[test]
    fn high_hr_produces_high_strain() {
        let calc = StrainCalculator::new(190, 60);
        // 170 bpm -> %HRR = (170-60)/(190-60) = 84.6% -> zone 4 (weight 4)
        // 30 min at zone 4 -> TRIMP = 30x4 = 120 -> strain ~= 11.34
        let readings = make_constant_readings(170, 1800);
        let strain = calc.calculate(&readings).unwrap().0;
        assert!(
            strain > 10.0,
            "sustained high HR should yield high strain, got {}",
            strain
        );
    }

    #[test]
    fn strain_capped_at_21() {
        let calc = StrainCalculator::new(190, 60);
        // 24h at max HR -> zone 5 -> TRIMP = 7200 -> strain = 21.0
        let readings = make_constant_readings(190, 86400);
        let strain = calc.calculate(&readings).unwrap().0;
        assert_eq!(strain, 21.0);
    }

    #[test]
    fn higher_hr_means_more_strain() {
        let calc = StrainCalculator::new(190, 60);
        let low = make_readings(100, 600);
        let high = make_readings(160, 600);
        let low_strain = calc.calculate(&low).unwrap().0;
        let high_strain = calc.calculate(&high).unwrap().0;
        assert!(
            high_strain > low_strain,
            "higher HR should produce more strain: {} vs {}",
            high_strain,
            low_strain
        );
    }

    #[test]
    fn zone_weights_with_hrr() {
        // max_hr=200, resting_hr=50 -> HR reserve = 150
        // Zone thresholds in bpm: 50%->125, 60%->140, 70%->155, 80%->170, 90%->185
        let resting_hr: u8 = 50;
        let hr_reserve: f64 = 150.0;

        // Below zone 1: bpm < 125 (< 50% HRR)
        assert_eq!(
            StrainCalculator::zone_weight(120, resting_hr, hr_reserve),
            0
        );
        // Zone 1: 50-60% HRR -> bpm 125-139
        assert_eq!(
            StrainCalculator::zone_weight(125, resting_hr, hr_reserve),
            1
        );
        assert_eq!(
            StrainCalculator::zone_weight(139, resting_hr, hr_reserve),
            1
        );
        // Zone 2: 60-70% HRR -> bpm 140-154
        assert_eq!(
            StrainCalculator::zone_weight(140, resting_hr, hr_reserve),
            2
        );
        assert_eq!(
            StrainCalculator::zone_weight(154, resting_hr, hr_reserve),
            2
        );
        // Zone 3: 70-80% HRR -> bpm 155-169
        assert_eq!(
            StrainCalculator::zone_weight(155, resting_hr, hr_reserve),
            3
        );
        assert_eq!(
            StrainCalculator::zone_weight(169, resting_hr, hr_reserve),
            3
        );
        // Zone 4: 80-90% HRR -> bpm 170-184
        assert_eq!(
            StrainCalculator::zone_weight(170, resting_hr, hr_reserve),
            4
        );
        assert_eq!(
            StrainCalculator::zone_weight(184, resting_hr, hr_reserve),
            4
        );
        // Zone 5: 90-100% HRR -> bpm 185-200
        assert_eq!(
            StrainCalculator::zone_weight(185, resting_hr, hr_reserve),
            5
        );
        assert_eq!(
            StrainCalculator::zone_weight(200, resting_hr, hr_reserve),
            5
        );
    }
}
