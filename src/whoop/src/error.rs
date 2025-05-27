use thiserror::Error;

#[derive(Debug, Error)]
#[error("{self:?}")]
pub enum WhoopError {
    PacketTooShort,
    InvalidSof,
    InvalidHeaderCrc8,
    InvalidPacketLength,
    InvalidDataCrc32,
    InvalidIndexError,
    InvalidPacketType(u8),
    InvalidData,
    InvalidMetadataType(u8),
    InvalidCommandType(u8),
    InvalidConsoleLog,
    Unimplemented,
    InvalidRRCount,
}
