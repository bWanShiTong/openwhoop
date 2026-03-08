use std::array::TryFromSliceError;

use thiserror::Error;

#[derive(Debug, Error)]
#[error("{self:?}")]
pub enum WhoopError {
    PacketTooShort,
    InvalidSof,
    InvalidHeaderCrc8,
    InvalidHeaderCrc16,
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
    Overflow,
    InvalidTime,
    InvalidSliceError,
}

impl From<TryFromSliceError> for WhoopError {
    fn from(_value: TryFromSliceError) -> Self {
        Self::InvalidSliceError
    }
}
