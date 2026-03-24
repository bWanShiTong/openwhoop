use chrono::Utc;

use crate::{
    WhoopPacket,
    constants::{CommandNumber, PacketType},
    error::WhoopError,
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

    pub fn set_time() -> Result<WhoopPacket, WhoopError> {
        let mut data = vec![];
        let current_time = u32::try_from(Utc::now().timestamp()).map_err(|_| WhoopError::Overflow)?;
        data.extend_from_slice(&current_time.to_le_bytes());
        data.append(&mut vec![0, 0, 0, 0, 0]); // padding
        Ok(WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::SetClock.as_u8(),
            data,
        ))
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

    pub fn get_alarm_time() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetAlarmTime.as_u8(),
            vec![0x00],
        )
    }

    pub fn toggle_imu_mode(value: bool) -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleImuMode.as_u8(),
            vec![u8::from(value)],
        )
    }

    pub fn toggle_imu_mode_historical(value: bool) -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleImuModeHistorical.as_u8(),
            vec![u8::from(value)],
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

    pub fn enable_optical_data(enable: bool) -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::EnableOpticalData.as_u8(),
            vec![0x01, u8::from(enable)],
        )
    }

    pub fn toggle_optical_mode(enable: bool) -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleOpticalMode.as_u8(),
            vec![0x01, u8::from(enable)],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_command_packet(packet: &WhoopPacket, expected_cmd: CommandNumber) {
        assert_eq!(packet.packet_type, PacketType::Command);
        assert_eq!(packet.cmd, expected_cmd.as_u8());
    }

    fn assert_roundtrip(packet: &WhoopPacket) {
        let framed = packet.framed_packet().unwrap();
        let parsed = WhoopPacket::from_data(framed).unwrap();
        assert_eq!(parsed.packet_type, packet.packet_type);
        assert_eq!(parsed.cmd, packet.cmd);
        assert_eq!(parsed.data, packet.data);
    }

    #[test]
    fn enter_high_freq_sync_packet() {
        let p = WhoopPacket::enter_high_freq_sync();
        assert_command_packet(&p, CommandNumber::EnterHighFreqSync);
        assert!(p.data.is_empty());
    }

    #[test]
    fn exit_high_freq_sync_packet() {
        let p = WhoopPacket::exit_high_freq_sync();
        assert_command_packet(&p, CommandNumber::ExitHighFreqSync);
        assert!(p.data.is_empty());
    }

    #[test]
    fn history_start_packet() {
        let p = WhoopPacket::history_start();
        assert_command_packet(&p, CommandNumber::SendHistoricalData);
        assert_eq!(p.data, vec![0x00]);
        assert_roundtrip(&p);
    }

    #[test]
    fn hello_harvard_packet() {
        let p = WhoopPacket::hello_harvard();
        assert_command_packet(&p, CommandNumber::GetHelloHarvard);
        assert_roundtrip(&p);
    }

    #[test]
    fn version_packet() {
        let p = WhoopPacket::version();
        assert_command_packet(&p, CommandNumber::ReportVersionInfo);
        assert_roundtrip(&p);
    }

    #[test]
    fn toggle_imu_mode_on_off() {
        let on = WhoopPacket::toggle_imu_mode(true);
        assert_eq!(on.data, vec![1]);
        assert_roundtrip(&on);

        let off = WhoopPacket::toggle_imu_mode(false);
        assert_eq!(off.data, vec![0]);
        assert_roundtrip(&off);
    }

    #[test]
    fn history_end_encodes_data() {
        let p = WhoopPacket::history_end(0x12345678);
        assert_command_packet(&p, CommandNumber::HistoricalDataResult);
        // First byte is 0x01, then LE u32
        assert_eq!(p.data[0], 0x01);
        assert_eq!(&p.data[1..5], &0x12345678_u32.to_le_bytes());
        assert_roundtrip(&p);
    }

    #[test]
    fn erase_packet() {
        let p = WhoopPacket::erase();
        assert_command_packet(&p, CommandNumber::ForceTrim);
        assert_eq!(p.data, vec![0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0x00]);
        assert_roundtrip(&p);
    }

    #[test]
    fn restart_packet() {
        let p = WhoopPacket::restart();
        assert_command_packet(&p, CommandNumber::RebootStrap);
        assert_roundtrip(&p);
    }

    #[test]
    fn set_time_packet() {
        let p = WhoopPacket::set_time().unwrap();
        assert_command_packet(&p, CommandNumber::SetClock);
        assert_eq!(p.data.len(), 9); // 4 bytes time + 5 bytes padding
        assert_roundtrip(&p);
    }

    #[test]
    fn get_name_packet() {
        let p = WhoopPacket::get_name();
        assert_command_packet(&p, CommandNumber::GetAdvertisingNameHarvard);
        assert_roundtrip(&p);
    }

    #[test]
    fn alarm_time_packet() {
        let p = WhoopPacket::alarm_time(1700000000);
        assert_command_packet(&p, CommandNumber::SetAlarmTime);
        assert_eq!(p.data[0], 0x01);
        assert_eq!(&p.data[1..5], &1700000000_u32.to_le_bytes());
        assert_roundtrip(&p);
    }

    #[test]
    fn get_alarm_time_packet() {
        let p = WhoopPacket::get_alarm_time();
        assert_command_packet(&p, CommandNumber::GetAlarmTime);
        assert_eq!(p.data, vec![0x00]);
        assert_roundtrip(&p);
    }

    #[test]
    fn enable_optical_data_on_off() {
        let on = WhoopPacket::enable_optical_data(true);
        assert_command_packet(&on, CommandNumber::EnableOpticalData);
        assert_eq!(on.data, vec![0x01, 0x01]);
        assert_roundtrip(&on);

        let off = WhoopPacket::enable_optical_data(false);
        assert_eq!(off.data, vec![0x01, 0x00]);
        assert_roundtrip(&off);
    }

    #[test]
    fn toggle_optical_mode_on_off() {
        let on = WhoopPacket::toggle_optical_mode(true);
        assert_command_packet(&on, CommandNumber::ToggleOpticalMode);
        assert_eq!(on.data, vec![0x01, 0x01]);
        assert_roundtrip(&on);

        let off = WhoopPacket::toggle_optical_mode(false);
        assert_eq!(off.data, vec![0x01, 0x00]);
        assert_roundtrip(&off);
    }
}
