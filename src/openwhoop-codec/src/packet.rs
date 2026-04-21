use std::fmt;

use crate::{constants::PacketType, error::WhoopError, helpers::BufferReader};

#[derive(Debug)]
pub struct WhoopPacket {
    pub packet_type: PacketType,
    pub seq: u8,
    pub cmd: u8,
    pub data: Vec<u8>,
    pub partial: bool,
    pub size: usize,
}

impl WhoopPacket {
    const SOF: u8 = 0xAA;

    pub fn with_seq(self, seq: u8) -> WhoopPacket {
        WhoopPacket { seq, ..self }
    }

    pub fn new(packet_type: PacketType, seq: u8, cmd: u8, data: Vec<u8>) -> Self {
        Self {
            packet_type,
            seq,
            cmd,
            size: data.len(),
            data,
            partial: false,
        }
    }

    pub fn from_data(mut data: Vec<u8>) -> Result<Self, WhoopError> {
        if data.len() < 8 {
            return Err(WhoopError::PacketTooShort);
        }

        let sof = data.pop_front()?;
        if sof != Self::SOF {
            return Err(WhoopError::InvalidSof);
        }

        // Verify header CRC8
        let length_buffer = data.read::<2>()?;
        let expected_crc8 = data.pop_front()?;
        let calculated_crc8 = Self::crc8(&length_buffer);

        if calculated_crc8 != expected_crc8 {
            return Err(WhoopError::InvalidHeaderCrc8);
        }

        let length = usize::from(u16::from_le_bytes(length_buffer));
        let partial = data.len() < length;
        if length < 8 {
            return Err(WhoopError::InvalidPacketLength);
        }

        // Verify data CRC32
        if !partial {
            let expected_crc32 = u32::from_le_bytes(data.read_end()?);
            let calculated_crc32 = Self::crc32(&data);
            if calculated_crc32 != expected_crc32 {
                return Err(WhoopError::InvalidDataCrc32);
            }
        }

        Ok(Self {
            packet_type: {
                let packet_type = data.pop_front()?;
                PacketType::from_u8(packet_type)
                    .ok_or(WhoopError::InvalidPacketType(packet_type))?
            },
            seq: data.pop_front()?,
            cmd: data.pop_front()?,
            data,
            partial,
            size: length,
        })
    }

    fn create_packet(&self) -> Vec<u8> {
        let mut packet = Vec::with_capacity(3 + self.data.len());
        packet.push(self.packet_type.as_u8());
        packet.push(self.seq);
        packet.push(self.cmd);
        packet.extend_from_slice(&self.data);
        packet
    }

    // used in gen5 header
    fn crc16(data: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;
        for &byte in data {
            crc ^= u16::from(byte);
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xA001;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc
    }

    /// WHOOP 5.0 frame:
    /// [SOF=0xAA][Flags=0x01][Length u16 LE][DestRole=0x00][SrcRole=0x01][CRC16 of bytes 0-5][Payload][CRC32 of payload]
    /// The payload is zero-padded to the next 4-byte boundary before CRC32 is
    /// computed, matching the `_align4` step in the reference implementation.
    pub fn framed_packet_maverick(&self) -> Result<Vec<u8>, WhoopError> {
        let pkt = self.create_packet();
        // Zero-pad to next 4-byte boundary.
        let padding = pkt.len().wrapping_neg() % 4;
        let mut pkt_aligned = pkt;
        pkt_aligned.resize(pkt_aligned.len() + padding, 0u8);

        let length = u16::try_from(pkt_aligned.len() + 4).map_err(|_| WhoopError::Overflow)?;

        let mut header = vec![
            0xAA,                  // SOF
            0x01,                  // Flags
            (length & 0xFF) as u8, // Length LSB
            (length >> 8) as u8,   // Length MSB
            0x00,                  // DestRole (strap)
            0x01,                  // SrcRole (host/app)
        ];
        let crc16 = Self::crc16(&header);
        header.push((crc16 & 0xFF) as u8);
        header.push((crc16 >> 8) as u8);

        let crc32 = Self::crc32(&pkt_aligned).to_le_bytes();
        let mut framed = header;
        framed.extend_from_slice(&pkt_aligned);
        framed.extend_from_slice(&crc32);

        Ok(framed)
    }

    pub fn from_data_maverick(mut data: Vec<u8>) -> Result<Self, WhoopError> {
        if data.len() < 8 {
            return Err(WhoopError::PacketTooShort);
        }

        if data[0] != Self::SOF {
            return Err(WhoopError::InvalidSof);
        }

        // verify CRC16 of header bytes 0-5
        let stored_crc16 = u16::from_le_bytes([data[6], data[7]]);
        let computed_crc16 = Self::crc16(&data[0..6]);
        if computed_crc16 != stored_crc16 {
            return Err(WhoopError::InvalidHeaderCrc16);
        }

        let length = usize::from(u16::from_le_bytes([data[2], data[3]]));

        // remove 8-byte header, payload starts at index 8
        let payload_start = 8;
        let payload_data: Vec<u8> = data.drain(payload_start..).collect();
        let partial = payload_data.len() < length;

        let packet_len = if partial { 3 } else { 4 };
        if payload_data.len() < packet_len {
            return Err(WhoopError::InvalidPacketLength);
        }

        if !partial {
            let (payload_body, crc_bytes) = payload_data.split_at(payload_data.len() - 4);
            let stored_crc32 = u32::from_le_bytes(crc_bytes.try_into()?);
            let computed_crc32 = Self::crc32(payload_body);
            if computed_crc32 != stored_crc32 {
                return Err(WhoopError::InvalidDataCrc32);
            }

            let mut body = payload_body.to_vec();
            if body.len() < 3 {
                return Err(WhoopError::InvalidPacketLength);
            }

            let packet_type_byte = body.remove(0);
            let packet_type = PacketType::from_u8(packet_type_byte)
                .ok_or(WhoopError::InvalidPacketType(packet_type_byte))?;

            let seq = body.pop_front()?;
            let cmd = body.pop_front()?;
            Ok(Self {
                packet_type,
                seq,
                cmd,
                size: length,
                data: body,
                partial: false,
            })
        } else {
            let mut body = payload_data;
            let packet_type_byte = body.pop_front()?;
            let packet_type = PacketType::from_u8(packet_type_byte)
                .ok_or(WhoopError::InvalidPacketType(packet_type_byte))?;

            let seq = body.pop_front()?;
            let cmd = body.pop_front()?;
            Ok(Self {
                packet_type,
                seq,
                cmd,
                size: length,
                data: body,
                partial: true,
            })
        }
    }

    fn crc8(data: &[u8]) -> u8 {
        let mut crc: u8 = 0;
        for &byte in data {
            crc ^= byte;
            for _ in 0..8 {
                if (crc & 0x80) != 0 {
                    crc = (crc << 1) ^ 0x07;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if (crc & 1) != 0 {
                    (crc >> 1) ^ 0xEDB88320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    pub fn framed_packet(&self) -> Result<Vec<u8>, WhoopError> {
        let pkt = self.create_packet();
        let length = u16::try_from(pkt.len()).map_err(|_| WhoopError::Overflow)? + 4;
        let length_buffer = length.to_le_bytes();
        let crc8_value = Self::crc8(&length_buffer);

        let crc32_value = Self::crc32(&pkt);
        let crc32_buffer = crc32_value.to_le_bytes();

        let mut framed_packet = vec![Self::SOF];
        framed_packet.extend_from_slice(&length_buffer);
        framed_packet.push(crc8_value);
        framed_packet.extend_from_slice(&pkt);
        framed_packet.extend_from_slice(&crc32_buffer);

        Ok(framed_packet)
    }
}

impl fmt::Display for WhoopPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WhoopPacket {{\n\tType: {:?},\n\tSeq: {},\n\tCmd: {:?},\n\tPayload: {}\n}}",
            self.packet_type,
            self.seq,
            self.cmd,
            hex::encode(&self.data)
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::PacketType;

    use super::*;

    #[test]
    fn test_packet_creation() {
        let packet = WhoopPacket::new(PacketType::Command, 1, 5, vec![0x01, 0x02, 0x03]);
        let framed = packet.framed_packet().unwrap();
        assert!(framed.len() > 8);
        assert_eq!(framed[0], WhoopPacket::SOF);
    }

    #[test]
    fn test_packet_parsing() {
        let original_packet = WhoopPacket::new(PacketType::Command, 1, 5, vec![0x01, 0x02, 0x03]);
        let framed = original_packet.framed_packet().unwrap();
        let parsed = WhoopPacket::from_data(framed).unwrap();

        assert_eq!(parsed.packet_type, original_packet.packet_type);
        assert_eq!(parsed.seq, original_packet.seq);
        assert_eq!(parsed.cmd, original_packet.cmd);
        assert_eq!(parsed.data, original_packet.data);
    }

    #[test]
    fn packet_too_short() {
        let result = WhoopPacket::from_data(vec![0xAA, 0x01]);
        assert!(matches!(result, Err(WhoopError::PacketTooShort)));
    }

    #[test]
    fn invalid_sof() {
        let result = WhoopPacket::from_data(vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert!(matches!(result, Err(WhoopError::InvalidSof)));
    }

    #[test]
    fn invalid_header_crc8() {
        // SOF + length bytes + wrong CRC8
        let mut data = vec![0xAA, 0x0B, 0x00, 0xFF]; // 0xFF is wrong CRC
        data.extend_from_slice(&[0; 20]);
        let result = WhoopPacket::from_data(data);
        assert!(matches!(result, Err(WhoopError::InvalidHeaderCrc8)));
    }

    #[test]
    fn with_seq_changes_seq() {
        let packet = WhoopPacket::new(PacketType::Command, 0, 1, vec![]);
        let packet = packet.with_seq(42);
        assert_eq!(packet.seq, 42);
    }

    #[test]
    fn display_format() {
        let packet = WhoopPacket::new(PacketType::Command, 1, 5, vec![0xAB, 0xCD]);
        let display = format!("{}", packet);
        assert!(display.contains("Command"));
        assert!(display.contains("abcd"));
    }

    #[test]
    fn roundtrip_all_packet_types() {
        for pt in [
            PacketType::Command,
            PacketType::CommandResponse,
            PacketType::HistoricalData,
            PacketType::Event,
            PacketType::Metadata,
            PacketType::ConsoleLogs,
        ] {
            let original = WhoopPacket::new(pt, 7, 3, vec![0x01, 0x02]);
            let framed = original.framed_packet().unwrap();
            let parsed = WhoopPacket::from_data(framed).unwrap();
            assert_eq!(parsed.packet_type, pt);
            assert_eq!(parsed.seq, 7);
            assert_eq!(parsed.cmd, 3);
            assert_eq!(parsed.data, vec![0x01, 0x02]);
        }
    }

    #[test]
    fn empty_payload_creates_valid_frame() {
        let packet = WhoopPacket::new(PacketType::Command, 0, 0, vec![]);
        let framed = packet.framed_packet().unwrap();
        // SOF + 2 length + 1 CRC8 + 3 (type/seq/cmd) + 4 CRC32 = 11 bytes
        assert_eq!(framed[0], WhoopPacket::SOF);
        assert_eq!(framed.len(), 11);
    }

    #[test]
    fn maverick_3byte_command_aligns_to_4() {
        let p = WhoopPacket::new(crate::constants::PacketType::Command, 1, 34, vec![]);
        let framed = p.framed_packet_maverick().unwrap();
        assert_eq!(
            framed,
            hex::decode("aa0108000001e67123012200dbf3b335").unwrap(),
            "get_data_range_gen5 frame mismatch"
        );
        // Inner payload length field == 8 (4-byte aligned body + 4-byte CRC32).
        let length = u16::from_le_bytes([framed[2], framed[3]]) as usize;
        assert_eq!(length, 8);
    }

    #[test]
    fn maverick_4byte_aligned_command_unchanged() {
        // history_end([0;8]): data = [0x01, 0,0,0,0,0,0,0,0] -> pkt = 12 bytes (already aligned)
        let p = WhoopPacket::new(
            crate::constants::PacketType::Command,
            1,
            23,
            vec![0x01, 0, 0, 0, 0, 0, 0, 0, 0],
        );
        let framed = p.framed_packet_maverick().unwrap();
        // 12-byte aligned body + 4 CRC32 = 16; length field should be 16.
        let length = u16::from_le_bytes([framed[2], framed[3]]) as usize;
        assert_eq!(length, 16);
        // Should round-trip cleanly.
        let parsed = WhoopPacket::from_data_maverick(framed).unwrap();
        assert_eq!(parsed.cmd, 23);
    }
}
