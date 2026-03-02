use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReading {
    pub unix: u64,
    pub bpm: u8,
    pub rr: Vec<u16>,
    pub imu_data: Vec<ImuSample>,
    pub sensor_data: Option<SensorData>,
}

/// DSP sensor fields from V12/V24 historical data packets.
/// These are raw ADC values - the WHOOP app uploads them to the server
/// for server-side digital signal processing (not parsed client-side).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorData {
    /// PPG green LED photodiode (channel 1)
    pub ppg_green: u16,
    /// PPG red/IR LED photodiode (channel 2)
    pub ppg_red_ir: u16,
    /// SpO2 red LED raw ADC reading
    pub spo2_red: u16,
    /// SpO2 infrared LED raw ADC reading
    pub spo2_ir: u16,
    /// Skin temperature thermistor raw ADC
    pub skin_temp_raw: u16,
    /// Ambient light photodiode raw ADC
    pub ambient_light: u16,
    /// LED drive current 1
    pub led_drive_1: u16,
    /// LED drive current 2
    pub led_drive_2: u16,
    /// Respiratory rate raw value
    pub resp_rate_raw: u16,
    /// Signal quality index
    pub signal_quality: u16,
    /// Skin contact indicator (0 = off-wrist)
    pub skin_contact: u8,
    /// Accelerometer gravity vector [x, y, z] (magnitude ~= 1.0g)
    pub accel_gravity: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImuSample {
    pub acc_x_g: f32,
    pub acc_y_g: f32,
    pub acc_z_g: f32,
    pub gyr_x_dps: f32,
    pub gyr_y_dps: f32,
    pub gyr_z_dps: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedHistoryReading {
    pub time: NaiveDateTime,
    pub bpm: u8,
    pub rr: Vec<u16>,
    pub imu_data: Option<Vec<ImuSample>>,
    /// Gravity vector [x, y, z] from V12/V24 packets. `None` for older packet formats.
    pub gravity: Option<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum Activity {
    #[default]
    Unknown,
    Active,
    Inactive,
    Sleep,
    Awake,
}

impl HistoryReading {
    pub fn is_valid(&self) -> bool {
        self.bpm > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_reading_valid_when_bpm_positive() {
        let reading = HistoryReading {
            unix: 1000,
            bpm: 70,
            rr: vec![800],
            imu_data: vec![],
            sensor_data: None,
        };
        assert!(reading.is_valid());
    }

    #[test]
    fn history_reading_invalid_when_bpm_zero() {
        let reading = HistoryReading {
            unix: 1000,
            bpm: 0,
            rr: vec![],
            imu_data: vec![],
            sensor_data: None,
        };
        assert!(!reading.is_valid());
    }
}
