use chrono::{Duration, NaiveDateTime, TimeDelta};
use openwhoop_codec::{Activity, ParsedHistoryReading};

const ACTIVITY_CHANGE_THRESHOLD: Duration = Duration::minutes(15);
const MIN_SLEEP_DURATION: Duration = Duration::minutes(60);
pub const MAX_SLEEP_PAUSE: Duration = Duration::minutes(60);

// Gravity-based detection thresholds (from notebook analysis)
const GRAVITY_STILL_THRESHOLD: f32 = 0.01; // g - max delta to be considered "still"
const GRAVITY_WINDOW_MINUTES: i64 = 15; // rolling window size in minutes
const GRAVITY_STILL_FRACTION: f32 = 0.70; // fraction of still readings to classify as sleep
const GRAVITY_MAX_GAP: Duration = Duration::minutes(20); // break runs on data gaps larger than this

#[derive(Clone, Copy, Debug)]
pub struct ActivityPeriod {
    pub activity: Activity,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub duration: TimeDelta,
}

#[derive(Clone, Copy, Debug)]
struct TempActivity {
    activity: Activity,
    start: NaiveDateTime,
    end: NaiveDateTime,
}

impl ActivityPeriod {
    pub fn is_active(&self) -> bool {
        matches!(self.activity, Activity::Active)
    }

    pub fn find_sleep(events: &mut Vec<ActivityPeriod>) -> Option<ActivityPeriod> {
        let mut next = || {
            if events.is_empty() {
                None
            } else {
                Some(events.remove(0))
            }
        };

        while let Some(event) = next() {
            if matches!(event.activity, Activity::Sleep) && event.duration > MIN_SLEEP_DURATION {
                return Some(event);
            }
        }

        None
    }

    /// Detect sleep/active periods using the gravity vector.
    pub fn detect_from_gravity(history: &[ParsedHistoryReading]) -> Vec<ActivityPeriod> {
        if history.len() < 2 {
            return Vec::new();
        }

        let deltas = std::iter::once(0.0_f32)
            .chain(
                history
                    .windows(2)
                    .map(|w| match (w[0].gravity, w[1].gravity) {
                        (Some(a), Some(b)) => {
                            let dx = a[0] - b[0];
                            let dy = a[1] - b[1];
                            let dz = a[2] - b[2];
                            (dx * dx + dy * dy + dz * dz).sqrt()
                        }
                        _ => f32::MAX, // no gravity data -> treat as active (moving)
                    }),
            )
            .collect::<Vec<_>>();

        let mut diffs = history
            .windows(2)
            .map(|w| (w[1].time - w[0].time).num_seconds())
            .filter(|&d| d > 0 && d < 300)
            .collect::<Vec<_>>();

        diffs.sort_unstable();
        let avg_interval_secs = diffs.get(diffs.len() / 2).copied().unwrap_or(60).max(1);

        let window_size = ((GRAVITY_WINDOW_MINUTES * 60) / avg_interval_secs) as usize;
        let window_size = window_size.max(3);
        let n = deltas.len();

        let still_frac = (0..n)
            .map(|i| {
                let half = window_size / 2;
                let start = i.saturating_sub(half);
                let end = (i + half + 1).min(n);
                let window = &deltas[start..end];
                let still = window
                    .iter()
                    .filter(|&&d| d < GRAVITY_STILL_THRESHOLD)
                    .count();

                still as f32 / window.len() as f32
            })
            .collect::<Vec<_>>();

        let is_sleep = still_frac
            .iter()
            .map(|&f| f >= GRAVITY_STILL_FRACTION)
            .collect::<Vec<_>>();

        let mut periods = Vec::new();
        let mut run_start = 0_usize;

        for i in 1..=n {
            let end_of_data = i == n;
            let class_change = !end_of_data && is_sleep[i] != is_sleep[run_start];
            let gap_break =
                !end_of_data && (history[i].time - history[i - 1].time) > GRAVITY_MAX_GAP;

            if end_of_data || class_change || gap_break {
                let activity = if is_sleep[run_start] {
                    Activity::Sleep
                } else {
                    Activity::Active
                };
                periods.push(TempActivity {
                    activity,
                    start: history[run_start].time,
                    end: history[i - 1].time,
                });
                if !end_of_data {
                    run_start = i;
                }
            }
        }

        Self::filter_merge(periods)
            .into_iter()
            .map(|a| ActivityPeriod {
                activity: a.activity,
                start: a.start,
                end: a.end,
                duration: a.end - a.start,
            })
            .collect()
    }

    fn filter_merge(mut activities: Vec<TempActivity>) -> Vec<TempActivity> {
        if activities.is_empty() {
            return Vec::new();
        }

        let mut merged = Vec::new();
        let mut i = 0;

        while i < activities.len() {
            let current = &activities[i];
            let duration = current.end - current.start;

            if duration < ACTIVITY_CHANGE_THRESHOLD {
                if i > 0
                    && i + 1 < activities.len()
                    && activities[i - 1].activity == activities[i + 1].activity
                    && !merged.is_empty()
                {
                    // Merge with both previous and next activity
                    let prev: TempActivity = merged.pop().unwrap();
                    merged.push(TempActivity {
                        activity: prev.activity,
                        start: prev.start,
                        end: activities[i + 1].end,
                    });
                    i += 1; // Skip next since it's merged
                } else if i + 1 < activities.len() {
                    // Merge with next
                    activities[i + 1] = TempActivity {
                        activity: activities[i + 1].activity,
                        start: current.start,
                        end: activities[i + 1].end,
                    };
                } else if !merged.is_empty() {
                    // Merge with previous if at the end
                    let prev = merged.pop().unwrap();
                    merged.push(TempActivity {
                        activity: prev.activity,
                        start: prev.start,
                        end: current.end,
                    });
                }
            } else {
                merged.push(*current);
            }

            i += 1;
        }

        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn base() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    fn make_reading(minutes: i64, gravity: Option<[f32; 3]>) -> ParsedHistoryReading {
        ParsedHistoryReading {
            time: base() + Duration::minutes(minutes),
            bpm: 70,
            rr: vec![],
            imu_data: None,
            gravity,
        }
    }

    // -- detect_from_gravity ------------------------------------------------

    #[test]
    fn detect_from_gravity_empty() {
        assert!(ActivityPeriod::detect_from_gravity(&[]).is_empty());
    }

    #[test]
    fn detect_from_gravity_single_reading() {
        let history = vec![make_reading(0, Some([0.0, 0.0, 1.0]))];
        assert!(ActivityPeriod::detect_from_gravity(&history).is_empty());
    }

    #[test]
    fn detect_from_gravity_all_still_is_sleep() {
        // 120 readings at 1-min intervals, all with the same gravity vector -> delta = 0
        let history: Vec<_> = (0..120)
            .map(|m| make_reading(m, Some([0.0, 0.0, 1.0])))
            .collect();
        let periods = ActivityPeriod::detect_from_gravity(&history);
        assert!(!periods.is_empty());
        assert!(matches!(periods[0].activity, Activity::Sleep));
    }

    #[test]
    fn detect_from_gravity_all_moving_is_active() {
        // Large gravity delta each step -> never still
        let history: Vec<_> = (0..120)
            .map(|m| {
                let v = if m % 2 == 0 { 1.0_f32 } else { -1.0_f32 };
                make_reading(m, Some([v, 0.0, 0.0]))
            })
            .collect();
        let periods = ActivityPeriod::detect_from_gravity(&history);
        assert!(!periods.is_empty());
        assert!(matches!(periods[0].activity, Activity::Active));
    }

    #[test]
    fn detect_from_gravity_no_gravity_data_is_active() {
        // gravity: None -> delta = MAX -> classified as active
        let history: Vec<_> = (0..120).map(|m| make_reading(m, None)).collect();
        let periods = ActivityPeriod::detect_from_gravity(&history);
        assert!(!periods.is_empty());
        assert!(matches!(periods[0].activity, Activity::Active));
    }

    #[test]
    fn detect_from_gravity_gap_breaks_run() {
        // Two separate sleep blocks separated by a >20-min gap should not merge
        let mut history: Vec<_> = (0..60)
            .map(|m| make_reading(m, Some([0.0, 0.0, 1.0])))
            .collect();
        // Jump 60 minutes forward (> GRAVITY_MAX_GAP = 20 min)
        history.extend((120..180).map(|m| make_reading(m, Some([0.0, 0.0, 1.0]))));
        let periods = ActivityPeriod::detect_from_gravity(&history);
        // Both blocks are sleep (still), gap forces a break -> at least 2 periods
        // (though filter_merge may re-merge short ones; both are 60 min so they survive)
        assert!(periods.len() >= 2);
    }

    // -- find_sleep ---------------------------------------------------------

    #[test]
    fn find_sleep_returns_long_sleep() {
        let b = base();
        let mut events = vec![
            ActivityPeriod {
                activity: Activity::Active,
                start: b,
                end: b + Duration::minutes(30),
                duration: Duration::minutes(30),
            },
            ActivityPeriod {
                activity: Activity::Sleep,
                start: b + Duration::minutes(30),
                end: b + Duration::minutes(300),
                duration: Duration::minutes(270),
            },
        ];
        let sleep = ActivityPeriod::find_sleep(&mut events);
        assert!(sleep.is_some());
        assert!(matches!(sleep.unwrap().activity, Activity::Sleep));
    }

    #[test]
    fn find_sleep_ignores_short_sleep() {
        let b = base();
        let mut events = vec![ActivityPeriod {
            activity: Activity::Sleep,
            start: b,
            end: b + Duration::minutes(30),
            duration: Duration::minutes(30),
        }];
        assert!(ActivityPeriod::find_sleep(&mut events).is_none());
    }

    #[test]
    fn find_sleep_empty_returns_none() {
        assert!(ActivityPeriod::find_sleep(&mut vec![]).is_none());
    }

    // -- is_active ----------------------------------------------------------

    #[test]
    fn is_active_returns_true_for_active() {
        let b = base();
        let period = ActivityPeriod {
            activity: Activity::Active,
            start: b,
            end: b + Duration::hours(1),
            duration: Duration::hours(1),
        };
        assert!(period.is_active());
    }

    #[test]
    fn is_active_returns_false_for_sleep() {
        let b = base();
        let period = ActivityPeriod {
            activity: Activity::Sleep,
            start: b,
            end: b + Duration::hours(1),
            duration: Duration::hours(1),
        };
        assert!(!period.is_active());
    }
}
