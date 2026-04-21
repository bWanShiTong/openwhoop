use chrono::Utc;

use crate::{
    WhoopPacket,
    constants::{CommandNumber, PacketType, WhoopGeneration},
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

    /// WHOOP 5.0 historical transfer start uses an empty payload.
    pub fn history_start_gen5() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::SendHistoricalData.as_u8(),
            vec![],
        )
    }

    pub fn get_data_range() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetDataRange.as_u8(),
            vec![0x00],
        )
    }

    /// WHOOP 5.0 GetDataRange request uses an empty payload.
    pub fn get_data_range_gen5() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetDataRange.as_u8(),
            vec![],
        )
    }

    pub fn get_battery_pack_info() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetBatteryPackInfo.as_u8(),
            vec![0x01],
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

    /// Handshake packet for WHOOP 5.0 (Maverick).
    pub fn hello() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetHello.as_u8(),
            vec![0x01],
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

    /// Get advertising name for WHOOP 5.0 (Maverick).
    /// NOTE: revision byte is unknown - 0x00 gets "unsupported revision:0", 0x01 causes reboot.
    pub fn get_maverick_name() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::GetAdvertisingName.as_u8(),
            vec![0x01],
        )
    }

    pub fn set_time() -> Result<WhoopPacket, WhoopError> {
        let mut data = vec![];
        let current_time =
            u32::try_from(Utc::now().timestamp()).map_err(|_| WhoopError::Overflow)?;
        data.extend_from_slice(&current_time.to_le_bytes());
        data.append(&mut vec![0, 0, 0, 0, 0]); // padding
        Ok(WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::SetClock.as_u8(),
            data,
        ))
    }

    pub fn history_end(end_data: [u8; 8]) -> WhoopPacket {
        let mut packet_data = vec![0x01];
        packet_data.extend_from_slice(&end_data);

        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::HistoricalDataResult.as_u8(),
            packet_data,
        )
    }

    pub fn history_end_failure() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::HistoricalDataResult.as_u8(),
            vec![0x00],
        )
    }

    pub fn abort_historical_transmits() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::AbortHistoricalTransmits.as_u8(),
            vec![],
        )
    }

    pub fn run_alarm_now() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::RunAlarm.as_u8(),
            vec![0x00],
        )
    }

    /// Ring the device immediately on WHOOP 5.0 (Gen5).
    /// Uses WSBLE_CMD_HAPTICS_RUN_NTF (cmd=19) with revision 0x01.
    /// Payload:
    /// - revision=0x01
    /// - effects=[0x2f, 0x98, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    /// - loop_ctrl=0x00
    /// - overall_loop=0x01
    pub fn run_haptic_pattern_gen5() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::RunHapticPatternMaverick.as_u8(),
            vec![
                0x01, 0x2f, 0x98, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            ],
        )
    }

    /// Set the alarm time.
    ///
    /// Gen4 format:  [rev=0x01][unix:4][padding:4]
    ///
    /// Maverick format (21 bytes, from pcapng):
    ///   [rev=0x04][alarm_id=0x01][unix:4][0x00 0x00][haptic_pattern:4][0x00*4][0x00 0x07 0x1e 0x00]
    ///   - alarm_id: 1-6 (per firmware debug menu)
    ///   - haptic_pattern 0x0000982f: same pattern ref as RunHapticPatternMaverick
    ///   - trailing bytes match official app pcapng capture
    pub fn alarm_time(unix: u32, generation: WhoopGeneration) -> WhoopPacket {
        let data = match generation {
            WhoopGeneration::Gen5 => {
                let mut d = vec![0x04, 0x01]; // revision=4, alarm_id=1
                d.extend_from_slice(&unix.to_le_bytes());
                d.extend_from_slice(&[0x00, 0x00]); // unknown
                d.extend_from_slice(&[0x2f, 0x98, 0x00, 0x00]); // haptic pattern ref
                d.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // zeros
                d.extend_from_slice(&[0x00, 0x00, 0x07, 0x1e, 0x00]); // trailing (timeout?)
                d
            }
            WhoopGeneration::Gen4 => {
                let mut d = vec![0x01]; // revision=1
                d.extend_from_slice(&unix.to_le_bytes());
                d.extend_from_slice(&[0, 0, 0, 0]); // padding
                d
            }
            WhoopGeneration::Placeholder => {
                panic!("WhoopGeneration::Placeholder cannot be used to build alarm packets")
            }
        };
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

    pub fn toggle_generic_hr_profile() -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleGenericHrProfile.as_u8(),
            vec![0x01],
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

    /// Enable or disable realtime HR streaming. Payload: [0x01=enable / 0x00=disable].
    /// No revision byte - same command for Gen4 and Maverick.
    pub fn toggle_realtime_hr(enable: bool) -> WhoopPacket {
        WhoopPacket::new(
            PacketType::Command,
            0,
            CommandNumber::ToggleRealtimeHr.as_u8(),
            vec![u8::from(enable)],
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
    fn history_start_gen5_packet() {
        let p = WhoopPacket::history_start_gen5();
        assert_command_packet(&p, CommandNumber::SendHistoricalData);
        assert!(p.data.is_empty());
    }

    #[test]
    fn get_data_range_gen5_packet() {
        let p = WhoopPacket::get_data_range_gen5();
        assert_command_packet(&p, CommandNumber::GetDataRange);
        assert!(p.data.is_empty());
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
        let end_data: [u8; 8] = [0x78, 0x56, 0x34, 0x12, 0xEF, 0xBE, 0xAD, 0xDE];
        let p = WhoopPacket::history_end(end_data);
        assert_command_packet(&p, CommandNumber::HistoricalDataResult);
        assert_eq!(p.data[0], 0x01);
        assert_eq!(&p.data[1..9], &end_data);
        assert_roundtrip(&p);
    }

    #[test]
    fn history_end_failure_packet() {
        let p = WhoopPacket::history_end_failure();
        assert_command_packet(&p, CommandNumber::HistoricalDataResult);
        assert_eq!(p.data, vec![0x00]);
        assert_roundtrip(&p);
    }

    #[test]
    fn abort_historical_transmits_packet() {
        let p = WhoopPacket::abort_historical_transmits();
        assert_command_packet(&p, CommandNumber::AbortHistoricalTransmits);
        assert!(p.data.is_empty());
    }

    #[test]
    fn erase_packet() {
        let p = WhoopPacket::erase();
        assert_command_packet(&p, CommandNumber::ForceTrim);
        assert_eq!(
            p.data,
            vec![0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0x00]
        );
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
        let p = WhoopPacket::alarm_time(1700000000, WhoopGeneration::Gen4);
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

    #[test]
    fn alarm_gen5() -> Result<(), WhoopError> {
        let packet = WhoopPacket::alarm_time(1772710140, WhoopGeneration::Gen5).with_seq(56);
        let data = packet.framed_packet_maverick()?;
        let expected =
            hex::decode("aa011c000001e3812338420401fc68a96900002f980000000000000000071e0089335b59")
                .unwrap();

        assert_eq!(data, expected);

        Ok(())
    }

    #[test]
    fn run_haptic_pattern_gen5_packet() -> Result<(), WhoopError> {
        let packet = WhoopPacket::run_haptic_pattern_gen5().with_seq(2);
        let data = packet.framed_packet_maverick()?;
        let expected =
            hex::decode("aa0114000001e1e1230213012f9800000000000000000100a090e5ad").unwrap();
        assert_eq!(data, expected);
        Ok(())
    }
}
