use chrono::Utc;

use crate::{
    WhoopPacket,
    constants::{CommandNumber, PacketType},
};

impl WhoopPacket {
    pub fn enter_high_freq_sync() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::EnterHighFreqSync.as_u8(),
            vec![],
        )
    }

    pub fn exit_high_freq_sync() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ExitHighFreqSync.as_u8(),
            vec![],
        )
    }

    pub fn history_start() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::SendHistoricalData.as_u8(),
            vec![0x00],
        )
    }

    pub fn hello_harvard() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetHelloHarvard.as_u8(),
            vec![0x00],
        )
    }

    pub fn get_name() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetAdvertisingNameHarvard.as_u8(),
            vec![0x00],
        )
    }

    pub fn set_time() -> WhoopPacket {
        let mut data = vec![];
        let current_time = Utc::now().timestamp() as u32;
        data.extend_from_slice(&current_time.to_le_bytes());
        data.append(&mut vec![0, 0, 0, 0, 0]); // padding
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::SetClock.as_u8(),
            data,
        )
    }

    pub fn history_end(data: u32) -> WhoopPacket {
        let mut packet_data = vec![0x01];
        packet_data.extend_from_slice(&data.to_le_bytes());
        packet_data.append(&mut vec![0, 0, 0, 0]); // padding

        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::HistoricalDataResult.as_u8(),
            packet_data,
        )
    }

    pub fn alarm_time(unix: u32) -> WhoopPacket {
        let mut data = vec![0x01];
        data.extend_from_slice(&unix.to_le_bytes());
        data.append(&mut vec![0, 0, 0, 0]); // padding
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::SetAlarmTime.as_u8(),
            data,
        )
    }

    pub fn toggle_imu_mode(value: bool) -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleImuMode.as_u8(),
            vec![value as u8],
        )
    }

    pub fn toggle_imu_mode_historical(value: bool) -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleImuModeHistorical.as_u8(),
            vec![value as u8],
        )
    }

    pub fn toggle_r7_data_collection() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleR7DataCollection.as_u8(),
            vec![0x01],
        )
    }

    pub fn restart() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::RebootStrap.as_u8(),
            vec![0x00],
        )
    }

    pub fn erase() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ForceTrim.as_u8(),
            vec![0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0x00],
        )
    }

    pub fn version() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ReportVersionInfo.as_u8(),
            vec![0x00],
        )
    }
}

#[test]
fn view_bytes() {
    let packet = WhoopPacket::erase();
    let bytes = packet.framed_packet();
    println!("SendHistoricalData");
    println!("aa10005723cf19fefefefefefefefe002f8744f6");
    println!("{}", hex::encode(bytes));

    // let packet = WhoopPacket::hello_harvard();
    // let bytes = packet.framed_packet();
    // println!("Hello Harvard");
    // println!("{:?}", bytes);
    // println!("{}", hex::encode(bytes));
}
