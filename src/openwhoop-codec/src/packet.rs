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
}
