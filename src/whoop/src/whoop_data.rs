use crate::{
    WhoopError, WhoopPacket,
    constants::{CommandNumber, MetadataType, PacketType},
    helpers::BufferReader,
};

mod history;
pub use history::{Activity, HistoryReading, ImuSample, ParsedHistoryReading};

#[derive(Debug, PartialEq)]
pub enum WhoopData {
    HistoryReading(HistoryReading),
    HistoryMetadata {
        unix: u32,
        data: u32,
        cmd: MetadataType,
    },
    ConsoleLog {
        unix: u32,
        log: String,
    },
    RunAlarm {
        unix: u32,
    },
    Event {
        unix: u32,
        event: CommandNumber,
    },
    UnknownEvent {
        unix: u32,
        event: u8,
    },
    VersionInfo {
        harvard: String,
        boylston: String,
    },
}

impl WhoopData {
    pub fn from_packet(packet: WhoopPacket) -> Result<Self, WhoopError> {
        match packet.packet_type {
            PacketType::HistoricalData => Self::parse_historical_packet(packet.data),
            PacketType::Metadata => Self::parse_metadata(packet),
            PacketType::ConsoleLogs => Self::parse_console_log(packet.data),
            PacketType::Event => Self::parse_event(packet),
            PacketType::CommandResponse => {
                let command = CommandNumber::from_u8(packet.cmd)
                    .ok_or(WhoopError::InvalidCommandType(packet.cmd))?;

                match command {
                    CommandNumber::ReportVersionInfo => {
                        Self::parse_report_version_info(packet.data)
                    }
                    _ => Err(WhoopError::Unimplemented),
                }
            }
            _ => Err(WhoopError::Unimplemented),
        }
    }

    fn parse_event(mut packet: WhoopPacket) -> Result<Self, WhoopError> {
        let command = CommandNumber::from_u8(packet.cmd).ok_or(packet.cmd);

        let _ = packet.data.pop_front()?;
        let unix = packet.data.read_u32_le()?;

        match command {
            Ok(CommandNumber::RunAlarm) => Ok(Self::RunAlarm { unix }),
            Ok(CommandNumber::SendR10R11Realtime)
            | Ok(CommandNumber::ToggleRealtimeHr)
            | Ok(CommandNumber::GetClock)
            | Ok(CommandNumber::RebootStrap)
            | Ok(CommandNumber::ToggleR7DataCollection)
            | Ok(CommandNumber::ToggleGenericHrProfile) => Ok(Self::Event {
                unix,
                event: command.expect("We check above that it is `Ok`"),
            }),
            Err(unknown) => Ok(Self::UnknownEvent {
                unix,
                event: unknown,
            }),
            _ => Err(WhoopError::Unimplemented),
        }
    }

    fn parse_console_log(mut packet: Vec<u8>) -> Result<Self, WhoopError> {
        let _ = packet.pop_front()?;
        let unix = packet.read_u32_le()?;

        let _ = packet.read::<2>();

        let mut result = Vec::new();

        let mut iter = packet.iter();
        let lookahead = packet.windows(3);

        for window in lookahead {
            if window != [0x34, 0x00, 0x01] {
                result.push(iter.next().copied().unwrap_or_default());
            } else {
                iter.nth(2);
            }
        }

        result.extend(iter);
        // not sure why this happens but sometimes Whoop gives logs
        // where part of logs is invalid, but some info can be still gained from them
        let lossy = String::from_utf8_lossy(&result).to_string();
        let log = match String::from_utf8(result) {
            Ok(log) => log,
            Err(_) => lossy,
        };
        Ok(Self::ConsoleLog { unix, log })
    }

    fn parse_metadata(mut packet: WhoopPacket) -> Result<Self, WhoopError> {
        let cmd =
            MetadataType::from_u8(packet.cmd).ok_or(WhoopError::InvalidMetadataType(packet.cmd))?;

        let unix = packet.data.read_u32_le()?;
        let _padding = packet.data.read::<6>()?;
        let data = packet.data.read_u32_le()?;

        Ok(Self::HistoryMetadata { unix, data, cmd })
    }

    fn parse_historical_packet(mut packet: Vec<u8>) -> Result<Self, WhoopError> {
        const MIN_PACKET_LEN_FOR_IMU: usize = 1188;

        let has_imu_data = packet.len() >= MIN_PACKET_LEN_FOR_IMU;

        if has_imu_data {
            return Self::parse_historical_packet_with_imu(packet);
        }

        let _something = packet.read::<4>();
        let unix = u64::from(packet.read_u32_le()?) * 1000; // Convert to millis
        let _something = packet.read::<6>();
        let bpm = packet.pop_front()?;
        let rr_count = usize::from(packet.pop_front()?);
        let mut rr = Vec::new();
        for _ in 0..4 {
            let rr_ = packet.read_u16_le()?;
            if rr_ == 0 {
                continue;
            }
            rr.push(rr_);
        }

        if rr.len() != rr_count {
            return Err(WhoopError::InvalidRRCount);
        }

        let activity = packet.read_u32_le()?;

        Ok(Self::HistoryReading(HistoryReading {
            unix,
            bpm,
            rr,
            activity,
            imu_data: Vec::new(),
        }))
    }

    fn parse_historical_packet_with_imu(mut packet: Vec<u8>) -> Result<Self, WhoopError> {
        // Constants for IMU parsing
        const ACC_X_OFFSET: usize = 85;
        const ACC_Y_OFFSET: usize = 285;
        const ACC_Z_OFFSET: usize = 485;
        const GYR_X_OFFSET: usize = 688;
        const GYR_Y_OFFSET: usize = 888;
        const GYR_Z_OFFSET: usize = 1088;
        const N_SAMPLES_IMU: usize = 100;
        const ACC_SENS: f32 = 1875.0;
        const GYR_SENS: f32 = 15.0;

        // baseline offset before evaluating the rr data
        let mut header_offset = 20;
        let _something = packet.read::<4>()?;
        let unix_seconds = packet.read_u32_le()?;
        let _sub_second_millis = packet.read_u16_le()?; // Not sure about this?
        let _something_else = packet.read::<4>()?;
        let bpm = packet.pop_front()?;
        let rr_count = packet.pop_front()?;
        let mut rr = Vec::new();
        for _ in 0..rr_count {
            let rr_ = packet.read_u16_le()?;
            if rr_ == 0 {
                continue;
            }
            rr.push(rr_);
        }
        if rr.len() as u8 != rr_count {
            return Err(WhoopError::InvalidRRCount);
        }

        header_offset += rr.len() * 2;

        let activity = packet.read_u32_le()?;

        // Helper function to read N_SAMPLES_IMU of i16 from a given offset
        // This closure captures N_SAMPLES_IMU from the outer scope.
        let read_imu_axis_data =
            |packet_data: &[u8], offset: usize| -> Result<Vec<i16>, WhoopError> {
                let mut axis_data = Vec::with_capacity(N_SAMPLES_IMU);
                for i in 0..N_SAMPLES_IMU {
                    // calculate the start index of the i-th sample - note we have already read header_offset bytes
                    let start = offset - header_offset + i * 2;
                    let end = start + 2;
                    if end > packet_data.len() {
                        // Bounds check
                        return Err(WhoopError::InvalidData);
                    }
                    let bytes = packet_data[start..end]
                        .try_into()
                        .map_err(|_| WhoopError::InvalidData)?;
                    axis_data.push(i16::from_be_bytes(bytes));
                }
                Ok(axis_data)
            };

        let acc_x_raw = read_imu_axis_data(&packet, ACC_X_OFFSET)?;
        let acc_y_raw = read_imu_axis_data(&packet, ACC_Y_OFFSET)?;
        let acc_z_raw = read_imu_axis_data(&packet, ACC_Z_OFFSET)?;
        let gyr_x_raw = read_imu_axis_data(&packet, GYR_X_OFFSET)?;
        let gyr_y_raw = read_imu_axis_data(&packet, GYR_Y_OFFSET)?;
        let gyr_z_raw = read_imu_axis_data(&packet, GYR_Z_OFFSET)?;

        let mut imu_data: Vec<ImuSample> = Vec::with_capacity(N_SAMPLES_IMU);
        for i in 0..N_SAMPLES_IMU {
            imu_data.push(ImuSample {
                acc_x_g: f32::from(acc_x_raw[i]) / ACC_SENS,
                acc_y_g: f32::from(acc_y_raw[i]) / ACC_SENS,
                acc_z_g: f32::from(acc_z_raw[i]) / ACC_SENS,
                gyr_x_dps: f32::from(gyr_x_raw[i]) / GYR_SENS,
                gyr_y_dps: f32::from(gyr_y_raw[i]) / GYR_SENS,
                gyr_z_dps: f32::from(gyr_z_raw[i]) / GYR_SENS,
            });
        }

        let unix = u64::from(unix_seconds) * 1000;
        Ok(Self::HistoryReading(HistoryReading {
            unix,
            bpm,
            rr,
            activity,
            imu_data,
        }))
    }

    fn parse_report_version_info(mut data: Vec<u8>) -> Result<Self, WhoopError> {
        let _ = data.read::<3>();
        let h_major = data.read_u32_le()?;
        let h_minor = data.read_u32_le()?;
        let h_patch = data.read_u32_le()?;
        let h_build = data.read_u32_le()?;
        let b_major = data.read_u32_le()?;
        let b_minor = data.read_u32_le()?;
        let b_patch = data.read_u32_le()?;
        let b_build = data.read_u32_le()?;
        Ok(Self::VersionInfo {
            harvard: format!("{}.{}.{}.{}", h_major, h_minor, h_patch, h_build),
            boylston: format!("{}.{}.{}.{}", b_major, b_minor, b_patch, b_build),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        WhoopPacket,
        constants::{MetadataType, PacketType},
        whoop_data::{
            WhoopData,
            history::{HistoryReading, ImuSample},
        },
    };

    #[test]
    fn parse_historical_packet() {
        let data = hex::decode(
            "aa8407f72f0a297020d700ec563568b860805418013e0145030000000000000020f12fff000000000000000000008033ac3c52c068bf1f25293eae57b63e0000224652c068bf1f25293eae57b63e38027302b5037602030166e0f0d4f001f11bf130f14ff123f1ddf0daf0eff010f134f158f16af139f10ff119f136f153f16af175f14ef122f1e5f0b9f0a2f08bf082f080f080f08ff0b5f0c0f097f06bf054f086f0a8f0a3f0a0f0bbf0c2f0bbf0bcf0f0f019f11df10ff10cf101f1fff0e9f0c1f09ef085f0a4f0d8f0fff041f14df159f159f13ef11bf116f1f3f0d6f0e0f0e1f0c5f0c9f0d8f0f9f013f119f11af10bf1edf0dbf0d6f0d5f0d1f0caf0def0faf0f5f0dcf0dcf0e6f0e6f0f8f0f4f0ecf0f4f0e7f002f10cf1faf0dbf0c8f08a03fa030b043304b5033803de020503810304044b0461045f047704590423042a041704dc03dc03fe032a04380430041d040b0416042d042f040a04ca03b403bd03e003cf036f03f902b6028f024d021202fb01ee012202d9027c0385039f0383039803a8039803620310031f032d031b030403130319031403e402f90241038b033f033b02aa01d801dd01c601cb01d501da01da01eb01e6018e014d017801bc01fe010202c001b601bf01be01bc01e3011402f701ce01a501dd01c0018a0163018a01b301e301d003c203bc0382045605ec0558063f061406fa05ba055b051505ad044304d20382034d033c034a0347033e033903360358039c03e6031c04420475049b0491046504440465048a045204440431040604ef03e003b5039c03ad0309043c046d046d048e04ad04c304ef04f7049b046d04ba04fe044d0574058f0567055c0563056a05730554053e051a0511050005e204f70414054a057e059205800556054a053905fb040505270523054605460546050f05ec0429050c05ef040b053b056605800586058d0572050501664f0243021802b5012a01d600d000f9002c013f011d01cf0070001700d6ffa3ff89ff8affa5ffd8ff150048005a0060007800a400d70001011801240128012b012001fa00b8004900b1ff26ffcefeb0fecdfe16ff80fff7ff5200710062004a0025000c00f0ffccffbaffc6ffe7ff13001f001100f5ffc3ff87ff34ffcafe3bfe87fdf5fcdffc42fddffd73fee6fe41ff7bff9bffa3ff8bff70ff6aff7effa1ffb2ffb6ffb6ffadffa1ff9bffacffd1fff8ff0b00feffe5ffe2ffecfff3fff8ff140040006c008f002bff25ff21ff14ff17ff3bff6cff99ffc0ffe3ff02001e00360048004c004c004900440038002a0021001a0011000500f7ffe8ffdfffdaffd4ffcdffcdffd6ffdaffd6ffc8ffaaff97ff93ff8dff82ff7eff76ff6bff62ff5dff5fff6aff74ff78ff77ff76ff78ff78ff77ff6eff64ff5bff59ff61ff6eff7cff8fffa4ffb4ffc3ffd1ffdbffdeffe0ffe5ffebfff2fff4fff0ffeaffe8ffeaffe9ffe5ffe1ffe2ffe6ffe4ffdfffe0ffe7ffedffecffecffeaffe1ffd3ffc9ffc1ffb6ffaeffadffacffa9ffacff1b00140008000800180030003b003a003100210012000a000c001200150018001f001f0015000e000500f8ffeaffe1ffd8ffc9ffb4ff9bff85ff72ff63ff56ff49ff3cff32ff31ff43ff59ff6aff76ff7fff87ff8cff91ff95ff91ff8fff98ffa2ffabffabffa6ff9cff90ff83ff71ff60ff56ff4eff4eff59ff69ff77ff86ff93ff9cffa2ffa2ff96ff87ff84ff8aff90ff95ff9cffa8ffb3ffbcffc5ffcbffceffd2ffd5ffdcffe7ffeffff2fff2fff4fff7fffbff0700110012000e001200170015000d000600000100000011f300000000000000000a000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000032040000050000010000032000000000000220000000000007000000ebffffff07000000f9fffffffbfffffff7fffffff8ffffff03000000f3ffffffd7fffffff7ffffff00000000fbffffff04000000e3ffffff11000000fafffffffdffffffecffffffdcffffff01000000fcffffffebfffffffaffffff0e000000f6fffffffbfffffffcffffff00000000f5ffffffedfffffffcffffff04000000f2fffffff6fffffffbfffffff6fffffffcffffffdbffffff07000000fefffffffffffffff8fffffff4fffffffaffffff01000000f9ffffffdfffffffeeffffff00000000f6fffffffaffffff01000000f7fffffffefffffff9fffffffbfffffffcfffffff3fffffffffffffffafffffffdffffffecffffff06000000f6fffffff7fffffff4fffffffafffffff9fffffffaffffff04000000fcffffff03000000fbfffffffafffffff3ffffff01000000fbfffffffcfffffffbfffffffffffffffffffffff1fffffff6fffffffbfffffff9fffffff9fffffffffffffff3fffffffcfffffffcfffffffdfffffffcfffffffbfffffff6ffffff03000000f9ffffff03000000ffffffff03000000312b0100f243aeb5",
        ).expect("msgpack error");
        let packet = WhoopPacket::from_data(data).expect("Invalid packet data");
        let data = WhoopData::from_packet(packet).expect("Invalid packet");
        assert_eq!(
            data,
            WhoopData::HistoryReading(HistoryReading {
                unix: 1748326124000,
                bpm: 62,
                rr: vec![837],
                activity: 0,
                imu_data: [
                    ImuSample {
                        acc_x_g: -2.184,
                        acc_y_g: 0.41546667,
                        acc_z_g: 0.50986665,
                        gyr_x_dps: 35.733334,
                        gyr_y_dps: -14.866667,
                        gyr_z_dps: 0.53333336
                    },
                    ImuSample {
                        acc_x_g: -2.0336,
                        acc_y_g: 0.5733333,
                        acc_z_g: 0.47893333,
                        gyr_x_dps: 46.2,
                        gyr_y_dps: -15.733334,
                        gyr_z_dps: 0.53333336
                    },
                    ImuSample {
                        acc_x_g: -2.0224,
                        acc_y_g: 0.64266664,
                        acc_z_g: 0.592,
                        gyr_x_dps: 19.866667,
                        gyr_y_dps: -15.533334,
                        gyr_z_dps: 1.6
                    },
                    ImuSample {
                        acc_x_g: -2.0058668,
                        acc_y_g: 0.43946666,
                        acc_z_g: 0.8085333,
                        gyr_x_dps: 31.333334,
                        gyr_y_dps: -13.133333,
                        gyr_z_dps: 3.2
                    },
                    ImuSample {
                        acc_x_g: -2.0293334,
                        acc_y_g: 0.528,
                        acc_z_g: 0.7296,
                        gyr_x_dps: 13.866667,
                        gyr_y_dps: -9.866667,
                        gyr_z_dps: 3.9333334
                    },
                    ImuSample {
                        acc_x_g: -1.9301333,
                        acc_y_g: 0.27573332,
                        acc_z_g: 0.8528,
                        gyr_x_dps: 16.6,
                        gyr_y_dps: -6.866667,
                        gyr_z_dps: 3.8666666
                    },
                    ImuSample {
                        acc_x_g: -2.0682666,
                        acc_y_g: 0.4784,
                        acc_z_g: 0.82986665,
                        gyr_x_dps: 2.9333334,
                        gyr_y_dps: -4.266667,
                        gyr_z_dps: 3.2666667
                    },
                    ImuSample {
                        acc_x_g: -2.0570667,
                        acc_y_g: 0.41173333,
                        acc_z_g: 0.9525333,
                        gyr_x_dps: 21.266666,
                        gyr_y_dps: -1.9333333,
                        gyr_z_dps: 2.2
                    },
                    ImuSample {
                        acc_x_g: -2.176,
                        acc_y_g: 0.58613336,
                        acc_z_g: 0.78186667,
                        gyr_x_dps: 19.0,
                        gyr_y_dps: -16.933332,
                        gyr_z_dps: 1.2
                    },
                    ImuSample {
                        acc_x_g: -2.0202668,
                        acc_y_g: 0.59786665,
                        acc_z_g: 0.7312,
                        gyr_x_dps: 30.866667,
                        gyr_y_dps: 2.0,
                        gyr_z_dps: 0.6666667
                    },
                    ImuSample {
                        acc_x_g: -2.0010667,
                        acc_y_g: 0.5968,
                        acc_z_g: 0.69386667,
                        gyr_x_dps: 7.4666667,
                        gyr_y_dps: 3.6,
                        gyr_z_dps: 0.8
                    },
                    ImuSample {
                        acc_x_g: -1.9914666,
                        acc_y_g: 0.6096,
                        acc_z_g: 0.77493334,
                        gyr_x_dps: 1.5333333,
                        gyr_y_dps: 4.8,
                        gyr_z_dps: 1.2
                    },
                    ImuSample {
                        acc_x_g: -2.0176,
                        acc_y_g: 0.5936,
                        acc_z_g: 0.5818667,
                        gyr_x_dps: 14.266666,
                        gyr_y_dps: 5.0666666,
                        gyr_z_dps: 1.4
                    },
                    ImuSample {
                        acc_x_g: -2.04,
                        acc_y_g: 0.5648,
                        acc_z_g: 0.6581333,
                        gyr_x_dps: -6.2,
                        gyr_y_dps: 5.0666666,
                        gyr_z_dps: 1.6
                    },
                    ImuSample {
                        acc_x_g: -2.0346668,
                        acc_y_g: 0.56853336,
                        acc_z_g: 0.47893333,
                        gyr_x_dps: -7.9333334,
                        gyr_y_dps: 4.866667,
                        gyr_z_dps: 2.0666666
                    },
                    ImuSample {
                        acc_x_g: -2.0192,
                        acc_y_g: 0.5584,
                        acc_z_g: 0.45066667,
                        gyr_x_dps: -7.866667,
                        gyr_y_dps: 4.5333333,
                        gyr_z_dps: 2.0666666
                    },
                    ImuSample {
                        acc_x_g: -2.0037334,
                        acc_y_g: 0.6634667,
                        acc_z_g: 0.4416,
                        gyr_x_dps: -6.0666666,
                        gyr_y_dps: 3.7333333,
                        gyr_z_dps: 1.4
                    },
                    ImuSample {
                        acc_x_g: -1.9914666,
                        acc_y_g: 0.5269333,
                        acc_z_g: 0.44906667,
                        gyr_x_dps: -2.6666667,
                        gyr_y_dps: 2.8,
                        gyr_z_dps: 0.93333334
                    },
                    ImuSample {
                        acc_x_g: -1.9856,
                        acc_y_g: 0.54506665,
                        acc_z_g: 0.44746667,
                        gyr_x_dps: -15.666667,
                        gyr_y_dps: 2.2,
                        gyr_z_dps: 0.33333334
                    },
                    ImuSample {
                        acc_x_g: -2.0064,
                        acc_y_g: 0.432,
                        acc_z_g: 0.44266668,
                        gyr_x_dps: 4.8,
                        gyr_y_dps: 1.7333333,
                        gyr_z_dps: 16.533333
                    },
                    ImuSample {
                        acc_x_g: -2.0298667,
                        acc_y_g: 0.576,
                        acc_z_g: 0.44,
                        gyr_x_dps: 6.0,
                        gyr_y_dps: 1.1333333,
                        gyr_z_dps: -1.4666667
                    },
                    ImuSample {
                        acc_x_g: -1.9258667,
                        acc_y_g: 0.57173336,
                        acc_z_g: 0.4384,
                        gyr_x_dps: 6.4,
                        gyr_y_dps: 0.33333334,
                        gyr_z_dps: -2.0666666
                    },
                    ImuSample {
                        acc_x_g: -2.0858667,
                        acc_y_g: 0.5616,
                        acc_z_g: 0.45653334,
                        gyr_x_dps: 8.0,
                        gyr_y_dps: 16.466667,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -2.0981333,
                        acc_y_g: 0.552,
                        acc_z_g: 0.4928,
                        gyr_x_dps: 10.933333,
                        gyr_y_dps: -1.6,
                        gyr_z_dps: -3.6666667
                    },
                    ImuSample {
                        acc_x_g: -2.1104,
                        acc_y_g: 0.5578667,
                        acc_z_g: 0.5322667,
                        gyr_x_dps: 14.333333,
                        gyr_y_dps: -2.2,
                        gyr_z_dps: -5.0666666
                    },
                    ImuSample {
                        acc_x_g: -2.1152,
                        acc_y_g: 0.5701333,
                        acc_z_g: 0.42453334,
                        gyr_x_dps: 0.06666667,
                        gyr_y_dps: -2.5333333,
                        gyr_z_dps: -6.733333
                    },
                    ImuSample {
                        acc_x_g: -2.1162667,
                        acc_y_g: 0.5712,
                        acc_z_g: 0.58133334,
                        gyr_x_dps: 18.666666,
                        gyr_y_dps: -2.9333334,
                        gyr_z_dps: -8.2
                    },
                    ImuSample {
                        acc_x_g: -2.1162667,
                        acc_y_g: 0.55146664,
                        acc_z_g: 0.6085333,
                        gyr_x_dps: 19.466667,
                        gyr_y_dps: -3.4,
                        gyr_z_dps: -9.466666
                    },
                    ImuSample {
                        acc_x_g: -2.1082666,
                        acc_y_g: 0.65386665,
                        acc_z_g: 0.6288,
                        gyr_x_dps: 19.733334,
                        gyr_y_dps: -3.4,
                        gyr_z_dps: -10.466666
                    },
                    ImuSample {
                        acc_x_g: -2.088,
                        acc_y_g: 0.5056,
                        acc_z_g: 0.6234667,
                        gyr_x_dps: 19.933332,
                        gyr_y_dps: -2.8,
                        gyr_z_dps: -11.333333
                    },
                    ImuSample {
                        acc_x_g: -2.0821333,
                        acc_y_g: 0.5104,
                        acc_z_g: 0.6,
                        gyr_x_dps: 19.2,
                        gyr_y_dps: -2.5333333,
                        gyr_z_dps: -12.2
                    },
                    ImuSample {
                        acc_x_g: -2.104,
                        acc_y_g: 0.5290667,
                        acc_z_g: 0.5824,
                        gyr_x_dps: 33.733334,
                        gyr_y_dps: -2.8,
                        gyr_z_dps: -13.066667
                    },
                    ImuSample {
                        acc_x_g: -2.1274667,
                        acc_y_g: 0.52,
                        acc_z_g: 0.6,
                        gyr_x_dps: 12.266666,
                        gyr_y_dps: -3.7333333,
                        gyr_z_dps: -13.733334
                    },
                    ImuSample {
                        acc_x_g: -2.1397333,
                        acc_y_g: 0.4688,
                        acc_z_g: 0.61973333,
                        gyr_x_dps: 4.866667,
                        gyr_y_dps: -5.733333,
                        gyr_z_dps: -13.8
                    },
                    ImuSample {
                        acc_x_g: -2.1130667,
                        acc_y_g: 0.5424,
                        acc_z_g: 0.58986664,
                        gyr_x_dps: 11.8,
                        gyr_y_dps: -7.0,
                        gyr_z_dps: -12.6
                    },
                    ImuSample {
                        acc_x_g: -2.0949333,
                        acc_y_g: 0.37013334,
                        acc_z_g: 0.5824,
                        gyr_x_dps: -14.533334,
                        gyr_y_dps: -7.266667,
                        gyr_z_dps: -11.133333
                    },
                    ImuSample {
                        acc_x_g: -2.0976,
                        acc_y_g: 0.34933335,
                        acc_z_g: 0.57226664,
                        gyr_x_dps: -3.3333333,
                        gyr_y_dps: -7.6666665,
                        gyr_z_dps: -10.0
                    },
                    ImuSample {
                        acc_x_g: -2.0992,
                        acc_y_g: 0.31413335,
                        acc_z_g: 0.54933333,
                        gyr_x_dps: -22.4,
                        gyr_y_dps: -8.4,
                        gyr_z_dps: -9.2
                    },
                    ImuSample {
                        acc_x_g: -2.0848,
                        acc_y_g: 0.28266665,
                        acc_z_g: 0.6736,
                        gyr_x_dps: -20.466667,
                        gyr_y_dps: -8.666667,
                        gyr_z_dps: -8.6
                    },
                    ImuSample {
                        acc_x_g: -2.0810666,
                        acc_y_g: 0.40693334,
                        acc_z_g: 0.5290667,
                        gyr_x_dps: -32.666668,
                        gyr_y_dps: -9.2,
                        gyr_z_dps: -8.066667
                    },
                    ImuSample {
                        acc_x_g: -2.0848,
                        acc_y_g: 0.26346666,
                        acc_z_g: 0.5061333,
                        gyr_x_dps: -8.533334,
                        gyr_y_dps: -9.933333,
                        gyr_z_dps: -7.733333
                    },
                    ImuSample {
                        acc_x_g: -2.0842667,
                        acc_y_g: 0.15466666,
                        acc_z_g: 0.4928,
                        gyr_x_dps: -0.6,
                        gyr_y_dps: -10.533334,
                        gyr_z_dps: -7.4
                    },
                    ImuSample {
                        acc_x_g: -2.0565333,
                        acc_y_g: 0.3888,
                        acc_z_g: 0.50186664,
                        gyr_x_dps: -11.6,
                        gyr_y_dps: -10.866667,
                        gyr_z_dps: -7.133333
                    },
                    ImuSample {
                        acc_x_g: -2.1712,
                        acc_y_g: 0.3392,
                        acc_z_g: 0.4144,
                        gyr_x_dps: 7.5333333,
                        gyr_y_dps: -10.733334,
                        gyr_z_dps: -7.4
                    },
                    ImuSample {
                        acc_x_g: -2.0325334,
                        acc_y_g: 0.48053333,
                        acc_z_g: 0.57813334,
                        gyr_x_dps: 6.5333333,
                        gyr_y_dps: -10.0,
                        gyr_z_dps: -7.5333333
                    },
                    ImuSample {
                        acc_x_g: -2.04,
                        acc_y_g: 0.4944,
                        acc_z_g: 0.60426664,
                        gyr_x_dps: 4.9333334,
                        gyr_y_dps: -9.333333,
                        gyr_z_dps: -6.9333334
                    },
                    ImuSample {
                        acc_x_g: -2.0416,
                        acc_y_g: 0.47946668,
                        acc_z_g: 0.60426664,
                        gyr_x_dps: 2.4666667,
                        gyr_y_dps: -9.066667,
                        gyr_z_dps: -6.266667
                    },
                    ImuSample {
                        acc_x_g: -2.0474668,
                        acc_y_g: 0.49066666,
                        acc_z_g: 0.62186664,
                        gyr_x_dps: 0.8,
                        gyr_y_dps: -9.133333,
                        gyr_z_dps: -5.6666665
                    },
                    ImuSample {
                        acc_x_g: -1.912,
                        acc_y_g: 0.4992,
                        acc_z_g: 0.6384,
                        gyr_x_dps: 16.0,
                        gyr_y_dps: -9.2,
                        gyr_z_dps: -5.6666665
                    },
                    ImuSample {
                        acc_x_g: -2.0602667,
                        acc_y_g: 0.49066666,
                        acc_z_g: 0.6501333,
                        gyr_x_dps: -3.4666667,
                        gyr_y_dps: -9.066667,
                        gyr_z_dps: -6.0
                    },
                    ImuSample {
                        acc_x_g: -2.0816,
                        acc_y_g: 0.46186668,
                        acc_z_g: 0.6736,
                        gyr_x_dps: -4.6666665,
                        gyr_y_dps: -9.066667,
                        gyr_z_dps: -6.6666665
                    },
                    ImuSample {
                        acc_x_g: -2.1002667,
                        acc_y_g: 0.41813335,
                        acc_z_g: 0.67786664,
                        gyr_x_dps: -3.8666666,
                        gyr_y_dps: -9.133333,
                        gyr_z_dps: -7.4666667
                    },
                    ImuSample {
                        acc_x_g: -2.1136,
                        acc_y_g: 0.42613333,
                        acc_z_g: 0.6288,
                        gyr_x_dps: -1.6666666,
                        gyr_y_dps: -9.733334,
                        gyr_z_dps: -8.333333
                    },
                    ImuSample {
                        acc_x_g: -2.0970666,
                        acc_y_g: 0.4336,
                        acc_z_g: 0.60426664,
                        gyr_x_dps: -15.8,
                        gyr_y_dps: -10.4,
                        gyr_z_dps: -9.533334
                    },
                    ImuSample {
                        acc_x_g: -2.0693333,
                        acc_y_g: 0.424,
                        acc_z_g: 0.64533335,
                        gyr_x_dps: 2.0666666,
                        gyr_y_dps: -11.0,
                        gyr_z_dps: -10.666667
                    },
                    ImuSample {
                        acc_x_g: -2.0485334,
                        acc_y_g: 0.41173333,
                        acc_z_g: 0.6816,
                        gyr_x_dps: 1.1333333,
                        gyr_y_dps: -11.133333,
                        gyr_z_dps: -11.333333
                    },
                    ImuSample {
                        acc_x_g: -2.1498666,
                        acc_y_g: 0.41973335,
                        acc_z_g: 0.5872,
                        gyr_x_dps: 16.333334,
                        gyr_y_dps: -10.6,
                        gyr_z_dps: -11.866667
                    },
                    ImuSample {
                        acc_x_g: -2.0069335,
                        acc_y_g: 0.42293334,
                        acc_z_g: 0.74453336,
                        gyr_x_dps: -4.0666666,
                        gyr_y_dps: -9.733334,
                        gyr_z_dps: -11.866667
                    },
                    ImuSample {
                        acc_x_g: -2.0005333,
                        acc_y_g: 0.42026666,
                        acc_z_g: 0.7589333,
                        gyr_x_dps: -8.066667,
                        gyr_y_dps: -8.8,
                        gyr_z_dps: -11.133333
                    },
                    ImuSample {
                        acc_x_g: -2.0005333,
                        acc_y_g: 0.5312,
                        acc_z_g: 0.7376,
                        gyr_x_dps: -13.6,
                        gyr_y_dps: -7.5333333,
                        gyr_z_dps: -10.066667
                    },
                    ImuSample {
                        acc_x_g: -2.0149333,
                        acc_y_g: 0.40586665,
                        acc_z_g: 0.7317333,
                        gyr_x_dps: -3.6,
                        gyr_y_dps: -6.133333,
                        gyr_z_dps: -9.133333
                    },
                    ImuSample {
                        acc_x_g: -2.0336,
                        acc_y_g: 0.30773333,
                        acc_z_g: 0.73546666,
                        gyr_x_dps: -30.2,
                        gyr_y_dps: -5.0666666,
                        gyr_z_dps: -8.133333
                    },
                    ImuSample {
                        acc_x_g: -2.0362666,
                        acc_y_g: 0.48373333,
                        acc_z_g: 0.7392,
                        gyr_x_dps: -25.133333,
                        gyr_y_dps: -4.0666666,
                        gyr_z_dps: -7.266667
                    },
                    ImuSample {
                        acc_x_g: -1.9184,
                        acc_y_g: 0.4432,
                        acc_z_g: 0.744,
                        gyr_x_dps: -34.866665,
                        gyr_y_dps: -3.1333334,
                        gyr_z_dps: -6.6666665
                    },
                    ImuSample {
                        acc_x_g: -2.0704,
                        acc_y_g: 0.44106665,
                        acc_z_g: 0.72746664,
                        gyr_x_dps: -53.4,
                        gyr_y_dps: -2.4666667,
                        gyr_z_dps: -6.266667
                    },
                    ImuSample {
                        acc_x_g: -2.0650666,
                        acc_y_g: 0.36373332,
                        acc_z_g: 0.71573335,
                        gyr_x_dps: -63.866665,
                        gyr_y_dps: -2.2666667,
                        gyr_z_dps: -6.266667
                    },
                    ImuSample {
                        acc_x_g: -2.0645332,
                        acc_y_g: 0.25173333,
                        acc_z_g: 0.6965333,
                        gyr_x_dps: -36.333332,
                        gyr_y_dps: -2.1333334,
                        gyr_z_dps: -7.0666666
                    },
                    ImuSample {
                        acc_x_g: -2.0794666,
                        acc_y_g: 0.2544,
                        acc_z_g: 0.69173336,
                        gyr_x_dps: -43.533333,
                        gyr_y_dps: -1.8,
                        gyr_z_dps: -8.066667
                    },
                    ImuSample {
                        acc_x_g: -2.0773335,
                        acc_y_g: 0.24213333,
                        acc_z_g: 0.68266666,
                        gyr_x_dps: -18.8,
                        gyr_y_dps: -1.4,
                        gyr_z_dps: -8.266666
                    },
                    ImuSample {
                        acc_x_g: -2.0693333,
                        acc_y_g: 0.2448,
                        acc_z_g: 0.8032,
                        gyr_x_dps: -29.8,
                        gyr_y_dps: -0.93333334,
                        gyr_z_dps: -7.866667
                    },
                    ImuSample {
                        acc_x_g: -2.0517333,
                        acc_y_g: 0.25013334,
                        acc_z_g: 0.67786664,
                        gyr_x_dps: -8.866667,
                        gyr_y_dps: -0.8,
                        gyr_z_dps: -7.4666667
                    },
                    ImuSample {
                        acc_x_g: -2.1744,
                        acc_y_g: 0.2528,
                        acc_z_g: 0.5568,
                        gyr_x_dps: -6.733333,
                        gyr_y_dps: -1.0666667,
                        gyr_z_dps: -7.133333
                    },
                    ImuSample {
                        acc_x_g: -2.0346668,
                        acc_y_g: 0.2528,
                        acc_z_g: 0.72213334,
                        gyr_x_dps: -6.2,
                        gyr_y_dps: -1.4666667,
                        gyr_z_dps: -6.6666665
                    },
                    ImuSample {
                        acc_x_g: -2.0341334,
                        acc_y_g: 0.26186666,
                        acc_z_g: 0.74986666,
                        gyr_x_dps: -7.8,
                        gyr_y_dps: -1.6,
                        gyr_z_dps: -5.866667
                    },
                    ImuSample {
                        acc_x_g: -2.0421333,
                        acc_y_g: 0.2592,
                        acc_z_g: 0.76053333,
                        gyr_x_dps: -9.6,
                        gyr_y_dps: -1.4666667,
                        gyr_z_dps: -5.133333
                    },
                    ImuSample {
                        acc_x_g: -1.9216,
                        acc_y_g: 0.21226667,
                        acc_z_g: 0.75093335,
                        gyr_x_dps: -10.0,
                        gyr_y_dps: -1.5333333,
                        gyr_z_dps: -4.5333333
                    },
                    ImuSample {
                        acc_x_g: -2.0677333,
                        acc_y_g: 0.1776,
                        acc_z_g: 0.7285333,
                        gyr_x_dps: -8.666667,
                        gyr_y_dps: -1.8,
                        gyr_z_dps: -3.9333334
                    },
                    ImuSample {
                        acc_x_g: -2.0704,
                        acc_y_g: 0.20053333,
                        acc_z_g: 0.72213334,
                        gyr_x_dps: -6.3333335,
                        gyr_y_dps: -2.0666666,
                        gyr_z_dps: -3.5333333
                    },
                    ImuSample {
                        acc_x_g: -2.0709333,
                        acc_y_g: 0.2368,
                        acc_z_g: 0.71306664,
                        gyr_x_dps: -5.2,
                        gyr_y_dps: -2.0,
                        gyr_z_dps: -3.3333333
                    },
                    ImuSample {
                        acc_x_g: -2.0730667,
                        acc_y_g: 0.272,
                        acc_z_g: 0.8165333,
                        gyr_x_dps: -4.9333334,
                        gyr_y_dps: -1.7333333,
                        gyr_z_dps: -3.0666666
                    },
                    ImuSample {
                        acc_x_g: -2.0768,
                        acc_y_g: 0.1376,
                        acc_z_g: 0.5488,
                        gyr_x_dps: -4.9333334,
                        gyr_y_dps: -1.8666667,
                        gyr_z_dps: -2.8666666
                    },
                    ImuSample {
                        acc_x_g: -2.0661333,
                        acc_y_g: 0.37546667,
                        acc_z_g: 0.70346665,
                        gyr_x_dps: -5.5333333,
                        gyr_y_dps: -2.2,
                        gyr_z_dps: -2.4
                    },
                    ImuSample {
                        acc_x_g: -2.0512,
                        acc_y_g: 0.2336,
                        acc_z_g: 0.70133334,
                        gyr_x_dps: -6.3333335,
                        gyr_y_dps: -2.1333334,
                        gyr_z_dps: -1.6666666
                    },
                    ImuSample {
                        acc_x_g: -2.0538666,
                        acc_y_g: 0.2384,
                        acc_z_g: 0.72,
                        gyr_x_dps: -6.733333,
                        gyr_y_dps: -1.6666666,
                        gyr_z_dps: -1.1333333
                    },
                    ImuSample {
                        acc_x_g: -2.0672,
                        acc_y_g: 0.23786667,
                        acc_z_g: 0.72,
                        gyr_x_dps: -5.6,
                        gyr_y_dps: -1.2666667,
                        gyr_z_dps: -0.93333334
                    },
                    ImuSample {
                        acc_x_g: -2.0672,
                        acc_y_g: 0.2368,
                        acc_z_g: 0.72,
                        gyr_x_dps: -3.1333334,
                        gyr_y_dps: -1.3333334,
                        gyr_z_dps: -0.93333334
                    },
                    ImuSample {
                        acc_x_g: -2.0618668,
                        acc_y_g: 0.2576,
                        acc_z_g: 0.6906667,
                        gyr_x_dps: -0.53333336,
                        gyr_y_dps: -1.3333334,
                        gyr_z_dps: -0.8
                    },
                    ImuSample {
                        acc_x_g: -2.0618668,
                        acc_y_g: 0.1472,
                        acc_z_g: 0.8085333,
                        gyr_x_dps: -16.333334,
                        gyr_y_dps: -1.4666667,
                        gyr_z_dps: -0.6
                    },
                    ImuSample {
                        acc_x_g: -2.0522666,
                        acc_y_g: 0.4048,
                        acc_z_g: 0.568,
                        gyr_x_dps: 16.933332,
                        gyr_y_dps: -2.0666666,
                        gyr_z_dps: -0.33333334
                    },
                    ImuSample {
                        acc_x_g: -2.0544,
                        acc_y_g: 0.2464,
                        acc_z_g: 0.68906665,
                        gyr_x_dps: -1.8,
                        gyr_y_dps: -3.0,
                        gyr_z_dps: -16.6
                    },
                    ImuSample {
                        acc_x_g: -2.0586667,
                        acc_y_g: 0.22453333,
                        acc_z_g: 0.81013334,
                        gyr_x_dps: -2.0,
                        gyr_y_dps: -3.6666667,
                        gyr_z_dps: 1.1333333
                    },
                    ImuSample {
                        acc_x_g: -2.0544,
                        acc_y_g: 0.2544,
                        acc_z_g: 0.552,
                        gyr_x_dps: -1.3333334,
                        gyr_y_dps: -4.2,
                        gyr_z_dps: 1.2
                    },
                    ImuSample {
                        acc_x_g: -2.0613334,
                        acc_y_g: 0.23893334,
                        acc_z_g: 0.7141333,
                        gyr_x_dps: -0.8666667,
                        gyr_y_dps: -4.9333334,
                        gyr_z_dps: 0.93333334
                    },
                    ImuSample {
                        acc_x_g: -2.1834667,
                        acc_y_g: 0.21013333,
                        acc_z_g: 0.7370667,
                        gyr_x_dps: -0.53333336,
                        gyr_y_dps: -5.4666667,
                        gyr_z_dps: 1.2
                    },
                    ImuSample {
                        acc_x_g: -2.0416,
                        acc_y_g: 0.18933333,
                        acc_z_g: 0.75093335,
                        gyr_x_dps: -15.733334,
                        gyr_y_dps: -5.5333333,
                        gyr_z_dps: 1.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.9146667,
                        acc_y_g: 0.21013333,
                        acc_z_g: 0.75413334,
                        gyr_x_dps: 4.266667,
                        gyr_y_dps: -5.6,
                        gyr_z_dps: 1.4
                    },
                    ImuSample {
                        acc_x_g: -2.0677333,
                        acc_y_g: 0.232,
                        acc_z_g: 0.7578667,
                        gyr_x_dps: 7.2,
                        gyr_y_dps: -5.8,
                        gyr_z_dps: 0.8666667
                    },
                    ImuSample {
                        acc_x_g: -2.0778666,
                        acc_y_g: 0.2576,
                        acc_z_g: 0.7434667,
                        gyr_x_dps: 9.533334,
                        gyr_y_dps: -5.6,
                        gyr_z_dps: 0.4
                    },
                    ImuSample {
                        acc_x_g: -2.1109333,
                        acc_y_g: 0.24746667,
                        acc_z_g: 0.6853333,
                        gyr_x_dps: 2.8666666,
                        gyr_y_dps: -15.266666,
                        gyr_z_dps: 0.0
                    },
                    ImuSample {
                        acc_x_g: 0.54293334,
                        acc_y_g: 0.51306665,
                        acc_z_g: 0.19093333,
                        gyr_x_dps: -14.6,
                        gyr_y_dps: 1.3333334,
                        gyr_z_dps: 17.066668
                    }
                ]
                .to_vec()
            })
        );

        let data = hex::decode("aa5c00f02f0c050f0008029e7e2868906380542c01400000000000000000000021436dff904d893dec19fb3e5ccf9b3d0a03773f00000000ec19fb3e5ccf9b3d0a03773fe0015702eb02590239019004010c020c310000000000000115f49cd0").expect("Invalid hex data");
        let packet = WhoopPacket::from_data(data).expect("Invalid packet data");
        let data = WhoopData::from_packet(packet).expect("Invalid packet");

        assert_eq!(
            data,
            WhoopData::HistoryReading(HistoryReading {
                unix: 1747484318000,
                bpm: 64,
                rr: vec![],
                activity: 1833115904,
                imu_data: Vec::new()
            })
        );

        let data = hex::decode("aa5c00f02f0c053f940900da106966280080545401360195040000000000000000a34cff0050bf3b144efb3da4a4463f299c0dbf00004c42144efb3da4a4463f299c0dbff40155023b03530255016004010c020c2000000000000002e8c17c8d").expect("Invalid hex data");
        let packet = WhoopPacket::from_data(data).expect("Invalid packet data");
        let data = WhoopData::from_packet(packet).expect("Invalid packet");

        assert_eq!(
            data,
            WhoopData::HistoryReading(HistoryReading {
                unix: 1718161626000,
                bpm: 54,
                rr: vec![1173],
                activity: 1285750784,
                imu_data: Vec::new()
            })
        );

        let data = hex::decode("aa6400a12f1805cb6cc100f7715c67300b805454015700000000000000000000005161cda013a03dcdcc1cbbd723133ee146873f00028a46cdcc1cbbd723133ee146873f28026d029c03700257019004010c020c3000000000000001b9120000000000000a9c4cac").expect("Invalid hex data");
        let packet = WhoopPacket::from_data(data).expect("Invalid packet data");
        let data = WhoopData::from_packet(packet).expect("Invalid packet");

        assert_eq!(
            data,
            WhoopData::HistoryReading(HistoryReading {
                unix: 1734111735000,
                bpm: 87,
                rr: Vec::new(),
                activity: 1632698368,
                imu_data: Vec::new()
            })
        );

        let data = hex::decode("aa8407f72f0a29eb21d70059583568c00b805418013c0000000000000000000000e62dff00000000000000000000c0ba163c00fc4ebf00a0e8bd9a21173f0000f4c600fc4ebf00a0e8bd9a21173f40027b02e9037b020301657df27ef29ef28df28ff28bf287f2a1f299f2a3f294f297f29bf291f295f2a4f295f28bf286f294f294f29df2a0f297f28cf27df281f291f28ef297f290f290f2a0f2a5f2a6f2a1f296f287f293f28ff296f29bf297f284f286f28df286f283f294f29bf296f293f28cf290f29ef2a6f2b2f2c0f2b7f2b3f2abf2a6f29ff29ef293f294f299f29bf296f27df283f276f274f26ef260f27af279f280f298f299f27cf263f27af273f279f25df25bf258f25ff27ff27af25ff256f250f25bf24bf249f241f264f27ff268fe68fe80fe80fe83fe8ffe96fe8bfe94fea4fe97fe7cfe79fe86fe92fe86fe72fe84fe80fe98fe9dfe93fe7ffe79fe79fe7ffe71fe6efe6bfe6cfe79fe8dfe8efe8bfe81fe7ffe7afe83fe76fe59fe56fe5bfe59fe5cfe56fe4ffe49fe48fe62fe79fe76fe64fe5dfe62fe73fe7efe89fe9afea1fe9bfe95fe9afe8bfe7afe6dfe6dfe8dfe9bfe98fe96fe80fe76fe84fe85fe82fe7dfe6bfe71fe70fe90fe85fe7efe89fe83fe8afe8dfe87fe6efe63fe6ffe67fe4afe41fe49fe42fe44fe2dfe41fe2cfe49fe6b08700878089608a208a208aa08a208a0089e08ac089a0892089a08960892088f0883088f0878086b086e0866086d0862087b088b0884088e08920895089808940899089c08a108aa08a0089c0899089c0898088e0881087e0884087f08750877087f08800883088808900899089b08a108a908a908a908a9089808960894089c089d08810878086e085e085e085a084d08560856085c086d086808670878086d086e0875086a0863085e0847084d083a0828081e080c0804081d082608440853086908580860080501651a0019001600130012000e000d000b000a0008000c0009000b000f000d0008000500060009000b0009000a000b00110016001c0021002300240026002700270022001e001e001f0021002200230022002200240025002500250028002b002e0030002f002d002d00320038003c003d003b00370031002e002e00300031003200350038003b003a0034002e002d002e0030002f002a00260025002400230021001d001800140010000a00080003000100010002000100fefffcfffafff3ffedffe6ffe4ffe6ffe9fffffffdfffbfffbfffbfffcffffff01000400060009000b000c000f000f0010000f000f000d000a0008000600040003000000fdfffbfffafff9fff8fff9fff9fff9fff9fffbfffdffffff0000010001000300040004000400050005000400040003000000ffff0000fffffffffefffeffffff01000400070009000b000c000d000e000e000e000d000d000d000c000b000b000d000e000e000f000f0010001200120012001100140018001b001b001a001a001a001a001800150013000f000f000d000b000a000d00f2fff4fff6fff8fff9fff7fff7fff6fff6fff5fff4fff4fff3fff3fff2fff3fff3fff2fff0ffefffedffebffe9ffe8ffe6ffe5ffe6ffe7ffe7ffe6ffe4ffe2ffe1ffdfffdcffdaffd9ffd7ffd6ffd6ffd6ffd7ffd7ffd8ffd8ffd9ffdaffdbffdbffdbffdcffddffdfffdfffdeffdfffdfffdfffdeffdcffdaffd8ffd7ffd6ffd6ffd7ffd7ffd6ffd5ffd3ffd1ffd0ffd1ffd2ffd4ffd6ffd7ffd8ffdaffd9ffd7ffd8ffd8ffdaffd8ffd5ffd7ffdaffddffe0ffdfffdfffe0ffe3ffe8ffebffebffeffff5fffaff000100000011f300000000000000000a0000000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320400000500000100000320000000000002200000000000e6fffffffaffffff02000000fefffffff4fffffffefffffffafffffffeffffff03000000f2ffffff00000000f8fffffff5ffffff01000000f3fffffff9ffffff1000000001000000f9fffffffdfffffff1ffffffe0fffffff4fffffffffffffff1fffffff0fffffffffffffffbffffff02000000ddfffffffeffffffe8fffffffaffffff06000000faffffff03000000faffffffebffffff00000000f6fffffff6fffffffcfffffff5fffffff3ffffff08000000fdfffffff9fffffff5fffffffdfffffffafffffffcffffff02000000fdfffffffffffffffffffffff5ffffff1b0000000800000003000000feffffffeffffffff8fffffff7ffffff2700000001000000feffffff040000000200000000000000fdfffffffffffffff1ffffff0100000006000000fbfffffffaffffff01000000f9ffffffffffffff070000000a000000fdffffff030000000b000000fffffffffcffffffffffffff0a000000fcffffff01000000000000000200000001000000f7fffffffbffffff0a000000fefffffffefffffff9fffffff8ffffff31280100a57c006f").expect("Invalid bytes");
        let packet = WhoopPacket::from_data(data).expect("Invalid packet data");
        dbg!(packet.size, packet.partial);
        let data = WhoopData::from_packet(packet).expect("Invalid packet");

        assert_eq!(
            data,
            WhoopData::HistoryReading(HistoryReading {
                unix: 1748326489000,
                bpm: 60,
                rr: Vec::new(),
                activity: 0,
                imu_data: [
                    ImuSample {
                        acc_x_g: -1.8272,
                        acc_y_g: -0.2048,
                        acc_z_g: 1.1562667,
                        gyr_x_dps: 1.4666667,
                        gyr_y_dps: -0.33333334,
                        gyr_z_dps: -0.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8362666,
                        acc_y_g: -0.2048,
                        acc_z_g: 1.1722667,
                        gyr_x_dps: 1.2666667,
                        gyr_y_dps: -0.33333334,
                        gyr_z_dps: -0.53333336
                    },
                    ImuSample {
                        acc_x_g: -1.8352,
                        acc_y_g: -0.2032,
                        acc_z_g: 1.1786667,
                        gyr_x_dps: 1.2,
                        gyr_y_dps: -0.33333334,
                        gyr_z_dps: -0.46666667
                    },
                    ImuSample {
                        acc_x_g: -1.8373333,
                        acc_y_g: -0.1968,
                        acc_z_g: 1.1786667,
                        gyr_x_dps: 0.93333334,
                        gyr_y_dps: -0.26666668,
                        gyr_z_dps: -0.6
                    },
                    ImuSample {
                        acc_x_g: -1.8394667,
                        acc_y_g: -0.19306667,
                        acc_z_g: 1.1829333,
                        gyr_x_dps: 0.8666667,
                        gyr_y_dps: -0.06666667,
                        gyr_z_dps: -0.6
                    },
                    ImuSample {
                        acc_x_g: -1.8256,
                        acc_y_g: -0.19893333,
                        acc_z_g: 1.1786667,
                        gyr_x_dps: 0.73333335,
                        gyr_y_dps: -17.0,
                        gyr_z_dps: -0.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8298666,
                        acc_y_g: -0.19413333,
                        acc_z_g: 1.1776,
                        gyr_x_dps: 0.6666667,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -0.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8245333,
                        acc_y_g: -0.1856,
                        acc_z_g: 1.1765333,
                        gyr_x_dps: 0.53333336,
                        gyr_y_dps: 0.4,
                        gyr_z_dps: -0.73333335
                    },
                    ImuSample {
                        acc_x_g: -1.8325334,
                        acc_y_g: -0.19253333,
                        acc_z_g: 1.184,
                        gyr_x_dps: 0.8,
                        gyr_y_dps: 0.6,
                        gyr_z_dps: -0.8
                    },
                    ImuSample {
                        acc_x_g: -1.8309333,
                        acc_y_g: -0.20693333,
                        acc_z_g: 1.1744,
                        gyr_x_dps: 0.6,
                        gyr_y_dps: 0.73333335,
                        gyr_z_dps: -0.8
                    },
                    ImuSample {
                        acc_x_g: -1.8288,
                        acc_y_g: -0.20853333,
                        acc_z_g: 1.1701334,
                        gyr_x_dps: 0.73333335,
                        gyr_y_dps: 0.8,
                        gyr_z_dps: -0.8666667
                    },
                    ImuSample {
                        acc_x_g: -1.8341334,
                        acc_y_g: -0.2016,
                        acc_z_g: 1.1744,
                        gyr_x_dps: 1.0,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -0.8666667
                    },
                    ImuSample {
                        acc_x_g: -1.832,
                        acc_y_g: -0.1952,
                        acc_z_g: 1.1722667,
                        gyr_x_dps: 0.8666667,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -0.93333334
                    },
                    ImuSample {
                        acc_x_g: -1.824,
                        acc_y_g: -0.2016,
                        acc_z_g: 1.1701334,
                        gyr_x_dps: 0.53333336,
                        gyr_y_dps: 1.0666667,
                        gyr_z_dps: -0.8666667
                    },
                    ImuSample {
                        acc_x_g: -1.832,
                        acc_y_g: -0.21226667,
                        acc_z_g: 1.1685333,
                        gyr_x_dps: 0.33333334,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -0.8666667
                    },
                    ImuSample {
                        acc_x_g: -1.8373333,
                        acc_y_g: -0.20266667,
                        acc_z_g: 1.1621333,
                        gyr_x_dps: 0.4,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -0.93333334
                    },
                    ImuSample {
                        acc_x_g: -1.84,
                        acc_y_g: -0.2048,
                        acc_z_g: 1.1685333,
                        gyr_x_dps: 0.6,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -1.0666667
                    },
                    ImuSample {
                        acc_x_g: -1.8325334,
                        acc_y_g: -0.192,
                        acc_z_g: 1.1562667,
                        gyr_x_dps: 0.73333335,
                        gyr_y_dps: 0.6666667,
                        gyr_z_dps: -1.1333333
                    },
                    ImuSample {
                        acc_x_g: -1.8325334,
                        acc_y_g: -0.18933333,
                        acc_z_g: 1.1493334,
                        gyr_x_dps: 0.6,
                        gyr_y_dps: 0.53333336,
                        gyr_z_dps: -1.2666667
                    },
                    ImuSample {
                        acc_x_g: -1.8277333,
                        acc_y_g: -0.19466667,
                        acc_z_g: 1.1509334,
                        gyr_x_dps: 0.6666667,
                        gyr_y_dps: 0.4,
                        gyr_z_dps: -1.4
                    },
                    ImuSample {
                        acc_x_g: -1.8261334,
                        acc_y_g: -0.20533334,
                        acc_z_g: 1.1466666,
                        gyr_x_dps: 0.73333335,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -1.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.8309333,
                        acc_y_g: -0.20853333,
                        acc_z_g: 1.1504,
                        gyr_x_dps: 1.1333333,
                        gyr_y_dps: 0.2,
                        gyr_z_dps: -1.6
                    },
                    ImuSample {
                        acc_x_g: -1.8368,
                        acc_y_g: -0.20853333,
                        acc_z_g: 1.1445333,
                        gyr_x_dps: 1.4666667,
                        gyr_y_dps: 0.0,
                        gyr_z_dps: -1.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8448,
                        acc_y_g: -0.20533334,
                        acc_z_g: 1.1578667,
                        gyr_x_dps: 1.8666667,
                        gyr_y_dps: 16.866667,
                        gyr_z_dps: -1.8
                    },
                    ImuSample {
                        acc_x_g: -1.8426666,
                        acc_y_g: -0.2128,
                        acc_z_g: 1.1664,
                        gyr_x_dps: 2.2,
                        gyr_y_dps: -0.33333334,
                        gyr_z_dps: -1.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8341334,
                        acc_y_g: -0.2144,
                        acc_z_g: 1.1626667,
                        gyr_x_dps: 2.3333333,
                        gyr_y_dps: -0.4,
                        gyr_z_dps: -1.6666666
                    },
                    ImuSample {
                        acc_x_g: -1.8357333,
                        acc_y_g: -0.216,
                        acc_z_g: 1.168,
                        gyr_x_dps: 2.4,
                        gyr_y_dps: -0.46666667,
                        gyr_z_dps: -1.6666666
                    },
                    ImuSample {
                        acc_x_g: -1.8309333,
                        acc_y_g: -0.21546666,
                        acc_z_g: 1.1701334,
                        gyr_x_dps: 2.5333333,
                        gyr_y_dps: -0.53333336,
                        gyr_z_dps: -1.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8346666,
                        acc_y_g: -0.20853333,
                        acc_z_g: 1.1717334,
                        gyr_x_dps: 2.6,
                        gyr_y_dps: -0.46666667,
                        gyr_z_dps: -1.8666667
                    },
                    ImuSample {
                        acc_x_g: -1.8346666,
                        acc_y_g: -0.19786666,
                        acc_z_g: 1.1733333,
                        gyr_x_dps: 2.6,
                        gyr_y_dps: -0.46666667,
                        gyr_z_dps: -2.0
                    },
                    ImuSample {
                        acc_x_g: -1.8261334,
                        acc_y_g: -0.19733334,
                        acc_z_g: 1.1712,
                        gyr_x_dps: 2.2666667,
                        gyr_y_dps: -0.46666667,
                        gyr_z_dps: -2.0666666
                    },
                    ImuSample {
                        acc_x_g: -1.8234667,
                        acc_y_g: -0.19893333,
                        acc_z_g: 1.1738666,
                        gyr_x_dps: 2.0,
                        gyr_y_dps: -0.46666667,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8229333,
                        acc_y_g: -0.20426667,
                        acc_z_g: 1.1754667,
                        gyr_x_dps: 2.0,
                        gyr_y_dps: -0.33333334,
                        gyr_z_dps: -2.4
                    },
                    ImuSample {
                        acc_x_g: -1.8256,
                        acc_y_g: -0.20533334,
                        acc_z_g: 1.1781334,
                        gyr_x_dps: 2.0666666,
                        gyr_y_dps: -0.2,
                        gyr_z_dps: -2.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.8314667,
                        acc_y_g: -0.208,
                        acc_z_g: 1.1829333,
                        gyr_x_dps: 2.2,
                        gyr_y_dps: -0.06666667,
                        gyr_z_dps: -2.6
                    },
                    ImuSample {
                        acc_x_g: -1.8394667,
                        acc_y_g: -0.2032,
                        acc_z_g: 1.1776,
                        gyr_x_dps: 2.2666667,
                        gyr_y_dps: -17.066668,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8330667,
                        acc_y_g: -0.21013333,
                        acc_z_g: 1.1754667,
                        gyr_x_dps: 2.3333333,
                        gyr_y_dps: 0.06666667,
                        gyr_z_dps: -2.8
                    },
                    ImuSample {
                        acc_x_g: -1.8352,
                        acc_y_g: -0.2256,
                        acc_z_g: 1.1738666,
                        gyr_x_dps: 2.2666667,
                        gyr_y_dps: 0.06666667,
                        gyr_z_dps: -2.8
                    },
                    ImuSample {
                        acc_x_g: -1.8314667,
                        acc_y_g: -0.2272,
                        acc_z_g: 1.1754667,
                        gyr_x_dps: 2.2666667,
                        gyr_y_dps: 0.2,
                        gyr_z_dps: -2.8
                    },
                    ImuSample {
                        acc_x_g: -1.8288,
                        acc_y_g: -0.22453333,
                        acc_z_g: 1.1733333,
                        gyr_x_dps: 2.4,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8309333,
                        acc_y_g: -0.2256,
                        acc_z_g: 1.168,
                        gyr_x_dps: 2.4666667,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8410667,
                        acc_y_g: -0.224,
                        acc_z_g: 1.1610667,
                        gyr_x_dps: 2.4666667,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.84,
                        acc_y_g: -0.2272,
                        acc_z_g: 1.1594666,
                        gyr_x_dps: 2.4666667,
                        gyr_y_dps: 0.33333334,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8362666,
                        acc_y_g: -0.23093334,
                        acc_z_g: 1.1626667,
                        gyr_x_dps: 2.6666667,
                        gyr_y_dps: 0.33333334,
                        gyr_z_dps: -2.6
                    },
                    ImuSample {
                        acc_x_g: -1.84,
                        acc_y_g: -0.23413333,
                        acc_z_g: 1.16,
                        gyr_x_dps: 2.8666666,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -2.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.8416,
                        acc_y_g: -0.23466666,
                        acc_z_g: 1.1546667,
                        gyr_x_dps: 3.0666666,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -2.4666667
                    },
                    ImuSample {
                        acc_x_g: -1.8325334,
                        acc_y_g: -0.2208,
                        acc_z_g: 1.1557333,
                        gyr_x_dps: 3.2,
                        gyr_y_dps: 0.2,
                        gyr_z_dps: -2.4666667
                    },
                    ImuSample {
                        acc_x_g: -1.8288,
                        acc_y_g: -0.20853333,
                        acc_z_g: 1.16,
                        gyr_x_dps: 3.1333334,
                        gyr_y_dps: 0.0,
                        gyr_z_dps: -2.4666667
                    },
                    ImuSample {
                        acc_x_g: -1.8314667,
                        acc_y_g: -0.21013333,
                        acc_z_g: 1.1605333,
                        gyr_x_dps: 3.0,
                        gyr_y_dps: 17.0,
                        gyr_z_dps: -2.4
                    },
                    ImuSample {
                        acc_x_g: -1.8330667,
                        acc_y_g: -0.21973333,
                        acc_z_g: 1.1621333,
                        gyr_x_dps: 3.0,
                        gyr_y_dps: -17.066668,
                        gyr_z_dps: -2.3333333
                    },
                    ImuSample {
                        acc_x_g: -1.8368,
                        acc_y_g: -0.22346666,
                        acc_z_g: 1.1648,
                        gyr_x_dps: 3.3333333,
                        gyr_y_dps: 17.0,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8346666,
                        acc_y_g: -0.2208,
                        acc_z_g: 1.1690667,
                        gyr_x_dps: 3.7333333,
                        gyr_y_dps: -0.06666667,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8272,
                        acc_y_g: -0.21173333,
                        acc_z_g: 1.1738666,
                        gyr_x_dps: 4.0,
                        gyr_y_dps: -0.13333334,
                        gyr_z_dps: -2.2666667
                    },
                    ImuSample {
                        acc_x_g: -1.8229333,
                        acc_y_g: -0.20586666,
                        acc_z_g: 1.1749333,
                        gyr_x_dps: 4.0666666,
                        gyr_y_dps: -0.13333334,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8165333,
                        acc_y_g: -0.2,
                        acc_z_g: 1.1781334,
                        gyr_x_dps: 3.9333334,
                        gyr_y_dps: -0.06666667,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8090667,
                        acc_y_g: -0.19093333,
                        acc_z_g: 1.1824,
                        gyr_x_dps: 3.6666667,
                        gyr_y_dps: -17.0,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8138666,
                        acc_y_g: -0.1872,
                        acc_z_g: 1.1824,
                        gyr_x_dps: 3.2666667,
                        gyr_y_dps: 0.26666668,
                        gyr_z_dps: -2.2666667
                    },
                    ImuSample {
                        acc_x_g: -1.816,
                        acc_y_g: -0.1904,
                        acc_z_g: 1.1824,
                        gyr_x_dps: 3.0666666,
                        gyr_y_dps: 0.46666667,
                        gyr_z_dps: -2.4
                    },
                    ImuSample {
                        acc_x_g: -1.8202667,
                        acc_y_g: -0.1936,
                        acc_z_g: 1.1824,
                        gyr_x_dps: 3.0666666,
                        gyr_y_dps: 0.6,
                        gyr_z_dps: -2.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.8229333,
                        acc_y_g: -0.19093333,
                        acc_z_g: 1.1733333,
                        gyr_x_dps: 3.2,
                        gyr_y_dps: 0.73333335,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8266667,
                        acc_y_g: -0.19893333,
                        acc_z_g: 1.1722667,
                        gyr_x_dps: 3.2666667,
                        gyr_y_dps: 0.8,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8272,
                        acc_y_g: -0.208,
                        acc_z_g: 1.1712,
                        gyr_x_dps: 3.3333333,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -2.8
                    },
                    ImuSample {
                        acc_x_g: -1.8330667,
                        acc_y_g: -0.21493334,
                        acc_z_g: 1.1754667,
                        gyr_x_dps: 3.5333333,
                        gyr_y_dps: 0.93333334,
                        gyr_z_dps: -2.8
                    },
                    ImuSample {
                        acc_x_g: -1.8325334,
                        acc_y_g: -0.21493334,
                        acc_z_g: 1.176,
                        gyr_x_dps: 3.7333333,
                        gyr_y_dps: 0.93333334,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8298666,
                        acc_y_g: -0.19786666,
                        acc_z_g: 1.1610667,
                        gyr_x_dps: 3.9333334,
                        gyr_y_dps: 0.93333334,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8288,
                        acc_y_g: -0.1904,
                        acc_z_g: 1.1562667,
                        gyr_x_dps: 3.8666666,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -2.8
                    },
                    ImuSample {
                        acc_x_g: -1.8314667,
                        acc_y_g: -0.192,
                        acc_z_g: 1.1509334,
                        gyr_x_dps: 3.4666667,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -2.8666666
                    },
                    ImuSample {
                        acc_x_g: -1.8448,
                        acc_y_g: -0.19306667,
                        acc_z_g: 1.1424,
                        gyr_x_dps: 3.0666666,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -3.0
                    },
                    ImuSample {
                        acc_x_g: -1.8416,
                        acc_y_g: -0.2048,
                        acc_z_g: 1.1424,
                        gyr_x_dps: 3.0,
                        gyr_y_dps: 0.8,
                        gyr_z_dps: -3.1333334
                    },
                    ImuSample {
                        acc_x_g: -1.8485334,
                        acc_y_g: -0.21013333,
                        acc_z_g: 1.1402667,
                        gyr_x_dps: 3.0666666,
                        gyr_y_dps: 0.73333335,
                        gyr_z_dps: -3.2
                    },
                    ImuSample {
                        acc_x_g: -1.8496,
                        acc_y_g: -0.20266667,
                        acc_z_g: 1.1333333,
                        gyr_x_dps: 3.2,
                        gyr_y_dps: 0.73333335,
                        gyr_z_dps: -3.1333334
                    },
                    ImuSample {
                        acc_x_g: -1.8528,
                        acc_y_g: -0.20213333,
                        acc_z_g: 1.1381333,
                        gyr_x_dps: 3.1333334,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -3.0666666
                    },
                    ImuSample {
                        acc_x_g: -1.8602667,
                        acc_y_g: -0.20373334,
                        acc_z_g: 1.1381333,
                        gyr_x_dps: 2.8,
                        gyr_y_dps: 0.93333334,
                        gyr_z_dps: -2.9333334
                    },
                    ImuSample {
                        acc_x_g: -1.8464,
                        acc_y_g: -0.2064,
                        acc_z_g: 1.1413333,
                        gyr_x_dps: 2.5333333,
                        gyr_y_dps: 0.93333334,
                        gyr_z_dps: -2.8
                    },
                    ImuSample {
                        acc_x_g: -1.8469334,
                        acc_y_g: -0.216,
                        acc_z_g: 1.1504,
                        gyr_x_dps: 2.4666667,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8432,
                        acc_y_g: -0.2128,
                        acc_z_g: 1.1477333,
                        gyr_x_dps: 2.4,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8304,
                        acc_y_g: -0.21333334,
                        acc_z_g: 1.1472,
                        gyr_x_dps: 2.3333333,
                        gyr_y_dps: 1.0666667,
                        gyr_z_dps: -2.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.8298666,
                        acc_y_g: -0.19626667,
                        acc_z_g: 1.1562667,
                        gyr_x_dps: 2.2,
                        gyr_y_dps: 1.2,
                        gyr_z_dps: -2.6
                    },
                    ImuSample {
                        acc_x_g: -1.8453333,
                        acc_y_g: -0.20213333,
                        acc_z_g: 1.1504,
                        gyr_x_dps: 1.9333333,
                        gyr_y_dps: 1.2,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8586667,
                        acc_y_g: -0.20586666,
                        acc_z_g: 1.1509334,
                        gyr_x_dps: 1.6,
                        gyr_y_dps: 1.2,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8464,
                        acc_y_g: -0.2,
                        acc_z_g: 1.1546667,
                        gyr_x_dps: 1.3333334,
                        gyr_y_dps: 1.1333333,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8501333,
                        acc_y_g: -0.2032,
                        acc_z_g: 1.1488,
                        gyr_x_dps: 1.0666667,
                        gyr_y_dps: 1.3333334,
                        gyr_z_dps: -2.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.8469334,
                        acc_y_g: -0.19946666,
                        acc_z_g: 1.1450666,
                        gyr_x_dps: 0.6666667,
                        gyr_y_dps: 1.6,
                        gyr_z_dps: -2.6666667
                    },
                    ImuSample {
                        acc_x_g: -1.8618667,
                        acc_y_g: -0.19786666,
                        acc_z_g: 1.1424,
                        gyr_x_dps: 0.53333336,
                        gyr_y_dps: 1.8,
                        gyr_z_dps: -2.8666666
                    },
                    ImuSample {
                        acc_x_g: -1.8629333,
                        acc_y_g: -0.20106667,
                        acc_z_g: 1.1301334,
                        gyr_x_dps: 0.2,
                        gyr_y_dps: 1.8,
                        gyr_z_dps: -2.7333333
                    },
                    ImuSample {
                        acc_x_g: -1.8645333,
                        acc_y_g: -0.2144,
                        acc_z_g: 1.1333333,
                        gyr_x_dps: 0.06666667,
                        gyr_y_dps: 1.7333333,
                        gyr_z_dps: -2.5333333
                    },
                    ImuSample {
                        acc_x_g: -1.8608,
                        acc_y_g: -0.22026667,
                        acc_z_g: 1.1232,
                        gyr_x_dps: 0.06666667,
                        gyr_y_dps: 1.7333333,
                        gyr_z_dps: -2.3333333
                    },
                    ImuSample {
                        acc_x_g: -1.8437333,
                        acc_y_g: -0.21386667,
                        acc_z_g: 1.1136,
                        gyr_x_dps: 0.13333334,
                        gyr_y_dps: 1.7333333,
                        gyr_z_dps: -2.1333334
                    },
                    ImuSample {
                        acc_x_g: -1.8464,
                        acc_y_g: -0.21813333,
                        acc_z_g: 1.1082667,
                        gyr_x_dps: 0.06666667,
                        gyr_y_dps: 1.7333333,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8608,
                        acc_y_g: -0.2336,
                        acc_z_g: 1.0986667,
                        gyr_x_dps: 16.933332,
                        gyr_y_dps: 1.6,
                        gyr_z_dps: -2.2
                    },
                    ImuSample {
                        acc_x_g: -1.8656,
                        acc_y_g: -0.2384,
                        acc_z_g: 1.0944,
                        gyr_x_dps: -0.26666668,
                        gyr_y_dps: 1.4,
                        gyr_z_dps: -2.1333334
                    },
                    ImuSample {
                        acc_x_g: -1.8688,
                        acc_y_g: -0.23413333,
                        acc_z_g: 1.1077334,
                        gyr_x_dps: -0.4,
                        gyr_y_dps: 1.2666667,
                        gyr_z_dps: -1.9333333
                    },
                    ImuSample {
                        acc_x_g: -1.8629333,
                        acc_y_g: -0.23786667,
                        acc_z_g: 1.1125333,
                        gyr_x_dps: -0.8666667,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -1.6
                    },
                    ImuSample {
                        acc_x_g: -1.8714666,
                        acc_y_g: -0.2368,
                        acc_z_g: 1.1285334,
                        gyr_x_dps: -1.2666667,
                        gyr_y_dps: 1.0,
                        gyr_z_dps: -1.4
                    },
                    ImuSample {
                        acc_x_g: -1.8725333,
                        acc_y_g: -0.24906667,
                        acc_z_g: 1.1365334,
                        gyr_x_dps: -1.7333333,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -1.4
                    },
                    ImuSample {
                        acc_x_g: -1.8768,
                        acc_y_g: -0.2384,
                        acc_z_g: 1.1482667,
                        gyr_x_dps: -1.8666667,
                        gyr_y_dps: 0.73333335,
                        gyr_z_dps: -1.1333333
                    },
                    ImuSample {
                        acc_x_g: -1.8581333,
                        acc_y_g: -0.2496,
                        acc_z_g: 1.1392,
                        gyr_x_dps: -1.7333333,
                        gyr_y_dps: 0.6666667,
                        gyr_z_dps: -0.73333335
                    },
                    ImuSample {
                        acc_x_g: -1.8437333,
                        acc_y_g: -0.23413333,
                        acc_z_g: 1.1434667,
                        gyr_x_dps: -1.5333333,
                        gyr_y_dps: 0.8666667,
                        gyr_z_dps: -0.4
                    },
                    ImuSample {
                        acc_x_g: -1.856,
                        acc_y_g: -0.216,
                        acc_z_g: 1.0949334,
                        gyr_x_dps: -0.06666667,
                        gyr_y_dps: 16.133333,
                        gyr_z_dps: -17.066668
                    },
                    ImuSample {
                        acc_x_g: -0.2176,
                        acc_y_g: 1.152,
                        acc_z_g: 0.1904,
                        gyr_x_dps: -0.2,
                        gyr_y_dps: -0.8,
                        gyr_z_dps: 17.066668
                    }
                ]
                .to_vec()
            })
        );

        let data = hex::decode("aa1c00ab31370268ae7667702d32000000c7b6000010000000000000e01eba47")
            .expect("Invalid hex data");

        let packet = WhoopPacket::from_data(data).expect("Invalid packet data");
        let data = WhoopData::from_packet(packet).expect("Invalid packet");

        assert_eq!(
            data,
            WhoopData::HistoryMetadata {
                unix: 1735831144,
                data: 46791,
                cmd: MetadataType::HistoryEnd
            }
        );
    }

    #[test]
    fn parse_console_logs() {
        let packet = WhoopPacket{
            packet_type: PacketType::ConsoleLogs,
            seq: 0,
            cmd: 2,
            data: hex::decode("007e0b6d67907b340001205472696d3a20307830303030303030303a30303031623665662028303a313132333637290a3231312c203131323633313400").expect("Invalid hex data"),
            size: 0,
            partial: false
        };

        let data = WhoopData::from_packet(packet).expect("Invalid data");
        assert_eq!(
            data,
            WhoopData::ConsoleLog {
                unix: 1735199614,
                log: " Trim: 0x00000000:0001b6ef (0:112367)\n211, 1126314\0".to_owned()
            }
        );
    }

    #[test]
    fn parse_event() {
        let packet = WhoopPacket {
            packet_type: PacketType::Event,
            seq: 0,
            cmd: 68,
            data: hex::decode("00b70c5467000c04000101ff00").expect("Invalid hex data"),
            size: 0,
            partial: false,
        };

        let data = WhoopData::from_packet(packet).expect("Invalid data");

        assert_eq!(data, WhoopData::RunAlarm { unix: 1733561527 });
    }

    #[test]
    fn parse_metadata() {
        let bytes = hex::decode("aa1c00ab311002a9fc8367205337000000257e00000a0000000000007ac020f8")
            .expect("invalid bytes");
        let packet = WhoopPacket::from_data(bytes).expect("Invalid packet");
        let data = WhoopData::from_packet(packet).expect("invalid packet");
        assert_eq!(
            data,
            WhoopData::HistoryMetadata {
                unix: 1736703145,
                data: 32293,
                cmd: MetadataType::HistoryEnd
            }
        );

        let bytes = hex::decode("aa2c005231010146fb8367404c0600000010000000020000002900000010000000030000000000000008020055fd251d").expect("invalid bytes");
        let packet = WhoopPacket::from_data(bytes).expect("Invalid packet");
        let data = WhoopData::from_packet(packet).expect("invalid packet");
        assert_eq!(
            data,
            WhoopData::HistoryMetadata {
                unix: 1736702790,
                data: 16,
                cmd: MetadataType::HistoryStart,
            }
        );
    }

    #[test]
    fn parse_version_response() {
        let response = hex::decode("aa50000c2477070a01012900000011000000020000000000000011000000020000000200000000000000030000000400000000000000000000000300000006000000000000000000000008050100000074b95569").expect("invalid data");
        let packet = WhoopPacket::from_data(response).expect("invalid packet");
        let data = WhoopData::from_packet(packet).expect("invalid packet");
        assert_eq!(
            data,
            WhoopData::VersionInfo {
                harvard: String::from("41.17.2.0"),
                boylston: String::from("17.2.2.0")
            }
        )
    }
}
