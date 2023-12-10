#![cfg_attr(not(test), no_std)]
use crate::date_time::UtcDateTime;
pub mod date_time;

// use chrono::{DateTime as ChronoDateTime, TimeZone, Utc};
use serde_derive::{Deserialize, Serialize};

// we could use new-type pattern here but let's keep it simple
pub type Id = u32;
pub type DevId = u32;
pub type Parameter = u32;

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub enum Command {
    Set(Id, Message, DevId),
    Get(Id, Parameter, DevId),
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub enum Message {
    SetTimeReference(UtcDateTime),
    TurnBlinkerOff,
    TurnBlinkerOnNow(u64, u64),
    TurnBlinkerOnAfterDelay(u64, u64, u64),
    TurnRgbLedOff,
    TurnRgbLedOn,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub enum Response {
    Data(Id, Parameter, u32, DevId),
    SetOk,
    ParseError,
    NotOK,
    Recovered,
    Illegal,
    SerializationError,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    nanoseconds: u32,
}

pub const CKSUM: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_CKSUM);

#[derive(Debug, Serialize, Deserialize)]
pub enum SerializationError {
    Serialization,
    CrcSerialization,
    CorncobsEncoding,
    BufferOverflow,
    CorncobsDecoding,
    Deserialization,
    CrcDeserialization,
    CrcMismatch,
}

// Serialize T into cobs encoded out_buf with crc
pub fn serialize_crc_cobs<'a, T: serde::Serialize, const N: usize>(
    t: &'a T,
    out_buf: &'a mut [u8; N],
) -> Result<&'a [u8], SerializationError> {
    let n_ser = ssmarshal::serialize(out_buf, t).map_err(|_| SerializationError::Serialization)?;
    let crc = CKSUM.checksum(&out_buf[0..n_ser]);
    let n_crc = ssmarshal::serialize(&mut out_buf[n_ser..], &crc)
        .map_err(|_| SerializationError::CrcSerialization)?;

    // overflow
    if n_ser + n_crc > out_buf.len() {
        return Err(SerializationError::BufferOverflow);
    }

    let buf_copy = *out_buf;
    let n = corncobs::encode_buf(&buf_copy[0..n_ser + n_crc], out_buf);

    if n > buf_copy.len() {
        return Err(SerializationError::CorncobsEncoding);
    }
    Ok(&out_buf[0..n])
}

// deserialize T from cobs in_buf with crc check
pub fn deserialize_crc_cobs<T>(in_buf: &mut [u8]) -> Result<T, SerializationError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let n = corncobs::decode_in_place(in_buf).map_err(|_| SerializationError::CorncobsDecoding)?;
    let (t, resp_used) = ssmarshal::deserialize::<T>(&in_buf[0..n])
        .map_err(|_| SerializationError::Deserialization)?;
    let crc_buf = &in_buf[resp_used..];
    let (crc, _) = ssmarshal::deserialize::<u32>(crc_buf)
        .map_err(|_| SerializationError::CrcDeserialization)?;
    let pkg_crc = CKSUM.checksum(&in_buf[0..resp_used]);
    if crc != pkg_crc {
        return Err(SerializationError::CrcMismatch);
    }
    Ok(t)
}
