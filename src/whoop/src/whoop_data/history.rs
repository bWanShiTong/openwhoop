use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReading {
    pub unix: u64,
    pub bpm: u8,
    pub rr: Vec<u16>,
    pub activity: u32,
    pub imu_data: Vec<ImuSample>,
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
    pub activity: Activity,
    pub imu_data: Option<Vec<ImuSample>>,
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

impl From<i64> for Activity {
    fn from(value: i64) -> Self {
        match value {
            0..500_000_000 => Self::Inactive,
            500_000_000..1_000_000_000 => Self::Active,
            1_000_000_000..1_500_000_000 => Self::Sleep,
            1_500_000_000..=i64::MAX => Self::Awake,
            _ => {
                println!("{}, {}", value, u64::from_le_bytes(value.to_le_bytes()));
                Self::Unknown
            }
        }
    }
}
