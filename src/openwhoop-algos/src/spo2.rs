use chrono::NaiveDateTime;

pub struct SpO2Calculator;

#[derive(Debug, Clone)]
pub struct SpO2Reading {
    pub time: NaiveDateTime,
    pub spo2_red: u16,
    pub spo2_ir: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct SpO2Score {
    pub time: NaiveDateTime,
    pub spo2_percentage: f64,
}

impl SpO2Calculator {
    pub const WINDOW_SIZE: usize = 30;

    pub fn calculate(readings: &[SpO2Reading]) -> Option<SpO2Score> {
        if readings.len() < Self::WINDOW_SIZE {
            return None;
        }

        let valid: Vec<_> = readings
            .iter()
            .filter(|r| r.spo2_red > 0 && r.spo2_ir > 0)
            .collect();

        if valid.len() < Self::WINDOW_SIZE {
            return None;
        }

        let n = valid.len() as f64;

        let mean_red = valid.iter().map(|r| f64::from(r.spo2_red)).sum::<f64>() / n;
        let mean_ir = valid.iter().map(|r| f64::from(r.spo2_ir)).sum::<f64>() / n;

        if mean_red < 1.0 || mean_ir < 1.0 {
            return None;
        }

        let ac_red = (valid
            .iter()
            .map(|r| {
                let diff = f64::from(r.spo2_red) - mean_red;
                diff * diff
            })
            .sum::<f64>()
            / n)
            .sqrt();

        let ac_ir = (valid
            .iter()
            .map(|r| {
                let diff = f64::from(r.spo2_ir) - mean_ir;
                diff * diff
            })
            .sum::<f64>()
            / n)
            .sqrt();

        if ac_red < 0.001 || ac_ir < 0.001 {
            return None;
        }

        let r = (ac_red / mean_red) / (ac_ir / mean_ir);
        let spo2 = (110.0 - 25.0 * r).clamp(70.0, 100.0);

        let time = valid.last()?.time;
        Some(SpO2Score {
            time,
            spo2_percentage: spo2,
        })
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

    fn make_readings(red: &[u16], ir: &[u16]) -> Vec<SpO2Reading> {
        red.iter()
            .zip(ir.iter())
            .enumerate()
            .map(|(i, (&r, &ir))| SpO2Reading {
                time: base_time() + chrono::TimeDelta::seconds(i64::try_from(i).unwrap()),
                spo2_red: r,
                spo2_ir: ir,
            })
            .collect()
    }

    #[test]
    fn too_few_readings() {
        let readings = make_readings(&[1000; 10], &[2000; 10]);
        assert!(SpO2Calculator::calculate(&readings).is_none());
    }

    #[test]
    fn all_zeros() {
        let readings = make_readings(&[0; 30], &[0; 30]);
        assert!(SpO2Calculator::calculate(&readings).is_none());
    }

    #[test]
    fn constant_signal() {
        let readings = make_readings(&[1000; 30], &[2000; 30]);
        assert!(SpO2Calculator::calculate(&readings).is_none());
    }

    #[test]
    fn ratio_of_one() {
        // When R=1, SpO2 = 110 - 25*1 = 85
        // R=1 when (ac_red/dc_red) == (ac_ir/dc_ir)
        // Same coefficient of variation -> same mean and stddev ratio
        let red: Vec<u16> = (0..30).map(|i| 1000 + (i % 3) * 10).collect();
        let ir: Vec<u16> = (0..30).map(|i| 2000 + (i % 3) * 20).collect();
        let readings = make_readings(&red, &ir);
        let result = SpO2Calculator::calculate(&readings).unwrap();
        assert!(
            (result.spo2_percentage - 85.0).abs() < 1.0,
            "Expected ~85%, got {}%",
            result.spo2_percentage
        );
    }

    #[test]
    fn synthetic_normal() {
        // IR has more AC variation relative to DC than red -> R < 1 -> SpO2 > 85%
        let red: Vec<u16> = (0..30).map(|i| 1000 + (i % 5) * 5).collect();
        let ir: Vec<u16> = (0..30).map(|i| 2000 + (i % 5) * 20).collect();
        let readings = make_readings(&red, &ir);
        let result = SpO2Calculator::calculate(&readings).unwrap();
        assert!(
            result.spo2_percentage >= 94.0 && result.spo2_percentage <= 100.0,
            "Expected 94-100%, got {}%",
            result.spo2_percentage
        );
    }
}
