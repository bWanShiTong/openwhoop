use chrono::Utc;

use crate::{
    constants::{CommandNumber, PacketType},
    WhoopPacket,
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

    pub fn reboot() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::RebootStrap.as_u8(),
            vec![0x00],
        )
    }
}