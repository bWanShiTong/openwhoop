use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use openwhoop_codec::ParsedHistoryReading;

use openwhoop_codec::WhoopError;

use super::ActivityPeriod;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SleepCycle {
    pub id: NaiveDate,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub min_bpm: u8,
    pub max_bpm: u8,
    pub avg_bpm: u8,
    pub min_hrv: u16,
    pub max_hrv: u16,
    pub avg_hrv: u16,
    pub score: f64,
}

impl SleepCycle {
    pub fn from_event(
        event: ActivityPeriod,
        history: &[ParsedHistoryReading],
    ) -> Result<SleepCycle, WhoopError> {
        let (heart_rate, rr): (Vec<u64>, Vec<Vec<_>>) = history
            .iter()
            .filter(|h| h.time >= event.start && h.time <= event.end)
            .map(|h| (u64::from(h.bpm), h.rr.clone()))
            .unzip();

        let rr = Self::clean_rr(rr);
        let rolling_hrv = Self::rolling_hrv(rr);

        let min_hrv = u16::try_from(rolling_hrv.iter().min().copied().unwrap_or_default())
            .map_err(|_| WhoopError::Overflow)?;
        let max_hrv = u16::try_from(rolling_hrv.iter().max().copied().unwrap_or_default())
            .map_err(|_| WhoopError::Overflow)?;

        let hrv_count = u64::try_from(rolling_hrv.len()).map_err(|_| WhoopError::Overflow)?;
        let hrv = rolling_hrv.into_iter().sum::<u64>() / hrv_count.max(1);
        let avg_hrv = u16::try_from(hrv).map_err(|_| WhoopError::Overflow)?;

        let min_bpm = u8::try_from(heart_rate.iter().min().copied().unwrap_or_default())
            .map_err(|_| WhoopError::Overflow)?;
        let max_bpm = u8::try_from(heart_rate.iter().max().copied().unwrap_or_default())
            .map_err(|_| WhoopError::Overflow)?;

        let heart_rate_count = u64::try_from(heart_rate.len()).map_err(|_| WhoopError::Overflow)?;
        let bpm = heart_rate.into_iter().sum::<u64>() / heart_rate_count.max(1);
        let avg_bpm = u8::try_from(bpm).map_err(|_| WhoopError::Overflow)?;

        let id = event.end.date();

        Ok(Self {
            id,
            start: event.start,
            end: event.end,
            min_bpm,
            max_bpm,
            avg_bpm,
            min_hrv,
            max_hrv,
            avg_hrv,
            score: Self::sleep_score(event.start, event.end),
        })
    }

    pub fn duration(&self) -> TimeDelta {
        self.end - self.start
    }

    fn clean_rr(rr: Vec<Vec<u16>>) -> Vec<u64> {
        rr.into_iter()
            .flatten()
            .filter(|&v| v > 0)
            .map(u64::from)
            .collect()
    }

    fn rolling_hrv(rr: Vec<u64>) -> Vec<u64> {
        rr.windows(300).filter_map(Self::calculate_rmssd).collect()
    }

    fn calculate_rmssd(window: &[u64]) -> Option<u64> {
        if window.len() < 2 {
            return None;
        }

        let rr_diff: Vec<f64> = window
            .windows(2)
            .map(|w| (w[1] as f64 - w[0] as f64).powi(2))
            .collect();

        let rr_count = rr_diff.len() as f64;
        Some((rr_diff.into_iter().sum::<f64>() / rr_count).sqrt() as u64)
    }

    pub fn sleep_score(start: NaiveDateTime, end: NaiveDateTime) -> f64 {
        let duration = (end - start).num_seconds();
        const IDEAL_DURATION: i64 = 60 * 60 * 8;

        let score = (duration / IDEAL_DURATION) as f64;

        (score * 100.0).clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn dt(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    #[test]
    fn sleep_score_8h_is_100() {
        let score = SleepCycle::sleep_score(dt(22, 0), dt(22, 0) + TimeDelta::hours(8));
        assert_eq!(score, 100.0);
    }

    #[test]
    fn sleep_score_4h_is_0() {
        // 4h / 8h = 0.5 -> integer division = 0 -> score = 0
        let score = SleepCycle::sleep_score(dt(22, 0), dt(22, 0) + TimeDelta::hours(4));
        assert_eq!(score, 0.0);
    }

    #[test]
    fn sleep_score_clamped_at_100() {
        let score = SleepCycle::sleep_score(dt(0, 0), dt(0, 0) + TimeDelta::hours(24));
        assert_eq!(score, 100.0);
    }

    #[test]
    fn duration_returns_difference() {
        let cycle = SleepCycle {
            id: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            start: dt(22, 0),
            end: dt(22, 0) + TimeDelta::hours(8),
            min_bpm: 50,
            max_bpm: 70,
            avg_bpm: 60,
            min_hrv: 30,
            max_hrv: 80,
            avg_hrv: 55,
            score: 100.0,
        };
        assert_eq!(cycle.duration(), TimeDelta::hours(8));
    }

    #[test]
    fn clean_rr_flattens_samples() {
        let rr = vec![vec![800, 900], vec![1000], vec![]];
        let result = SleepCycle::clean_rr(rr);
        assert_eq!(result, vec![800, 900, 1000]);
    }

    #[test]
    fn clean_rr_empty_input() {
        let result = SleepCycle::clean_rr(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn calculate_rmssd_basic() {
        // Constant RR -> all diffs = 0 -> RMSSD = 0
        let window = vec![800; 10];
        assert_eq!(SleepCycle::calculate_rmssd(&window), Some(0));
    }

    #[test]
    fn calculate_rmssd_with_variation() {
        // Alternating 800, 900 -> diff^2 = 10000 each -> mean = 10000 -> sqrt = 100
        let window: Vec<u64> = (0..10)
            .map(|i| if i % 2 == 0 { 800 } else { 900 })
            .collect();
        assert_eq!(SleepCycle::calculate_rmssd(&window), Some(100));
    }

    #[test]
    fn calculate_rmssd_single_element_returns_none() {
        assert!(SleepCycle::calculate_rmssd(&[800]).is_none());
    }

    #[test]
    fn rolling_hrv_needs_300_samples() {
        // Less than 300 samples -> no windows -> empty result
        let rr = vec![800; 299];
        assert!(SleepCycle::rolling_hrv(rr).is_empty());
    }

    #[test]
    fn rolling_hrv_exactly_300_samples() {
        let rr = vec![800; 300];
        let result = SleepCycle::rolling_hrv(rr);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0); // constant -> RMSSD = 0
    }

    #[test]
    fn from_event_computes_stats() {
        let base = dt(22, 0);
        let event = ActivityPeriod {
            activity: openwhoop_codec::Activity::Sleep,
            start: base,
            end: base + TimeDelta::hours(8),
            duration: TimeDelta::hours(8),
        };
        let history: Vec<ParsedHistoryReading> = (0..500)
            .map(|i| ParsedHistoryReading {
                time: base + TimeDelta::seconds(i * 60),
                bpm: 60,
                rr: vec![1000],
                imu_data: None,
                gravity: None,
            })
            .collect();
        let cycle = SleepCycle::from_event(event, &history).unwrap();
        assert_eq!(cycle.min_bpm, 60);
        assert_eq!(cycle.max_bpm, 60);
        assert_eq!(cycle.avg_bpm, 60);
        assert_eq!(cycle.score, 100.0);
    }
}
