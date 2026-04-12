use super::{IndexedPathRecord, ObjectGroupHeader, PointRecord};
use binrw::{BinRead, BinReaderExt};
use std::io::Cursor;

#[derive(BinRead)]
#[br(little)]
struct RawPointRecord {
    x: f64,
    y: f64,
}

#[derive(BinRead)]
#[br(little)]
struct RawIndexedPath {
    count: u32,
    #[br(count = count)]
    refs: Vec<u32>,
}

#[derive(BinRead)]
#[br(little)]
struct RawObjectGroupHeader16 {
    class_id: u32,
    flags: u32,
    style_a: u32,
    style_b: u32,
}

#[derive(BinRead)]
#[br(little)]
struct RawObjectGroupHeader28 {
    class_id: u32,
    flags: u32,
    style_a: u32,
    style_b: u32,
    style_c: u32,
    #[br(count = 8)]
    _reserved: Vec<u8>,
}

pub fn decode_point_record(payload: &[u8]) -> Option<PointRecord> {
    if payload.len() != 16 {
        return None;
    }

    let mut cursor = Cursor::new(payload);
    let raw = cursor.read_le::<RawPointRecord>().ok()?;
    Some(PointRecord { x: raw.x, y: raw.y })
}

pub fn decode_indexed_path(record_type: u32, payload: &[u8]) -> Option<IndexedPathRecord> {
    if payload.len() < 4 || !payload.len().is_multiple_of(4) {
        return None;
    }

    let mut cursor = Cursor::new(payload);
    let raw = cursor.read_le::<RawIndexedPath>().ok()?;
    let count = raw.count as usize;
    if payload.len() != (count + 1) * 4 {
        return None;
    }

    Some(IndexedPathRecord {
        record_type,
        refs: raw.refs.into_iter().map(|value| value as usize).collect(),
    })
}

pub fn decode_object_group_header(payload: &[u8]) -> Option<ObjectGroupHeader> {
    match payload.len() {
        0x10 => {
            let mut cursor = Cursor::new(payload);
            let raw = cursor.read_le::<RawObjectGroupHeader16>().ok()?;
            Some(ObjectGroupHeader {
                class_id: raw.class_id,
                flags: raw.flags,
                style_a: raw.style_a,
                style_b: raw.style_b,
                style_c: 0,
            })
        }
        0x1c => {
            let mut cursor = Cursor::new(payload);
            let raw = cursor.read_le::<RawObjectGroupHeader28>().ok()?;
            Some(ObjectGroupHeader {
                class_id: raw.class_id,
                flags: raw.flags,
                style_a: raw.style_a,
                style_b: raw.style_b,
                style_c: raw.style_c,
            })
        }
        _ => None,
    }
}

pub fn read_u16(data: &[u8], offset: usize) -> u16 {
    let bytes = data
        .get(offset..offset + 2)
        .expect("read_u16 caller must validate bounds");
    u16::from_le_bytes([bytes[0], bytes[1]])
}

pub fn read_i16(data: &[u8], offset: usize) -> i16 {
    let bytes = data
        .get(offset..offset + 2)
        .expect("read_i16 caller must validate bounds");
    i16::from_le_bytes([bytes[0], bytes[1]])
}

pub fn read_u32(data: &[u8], offset: usize) -> u32 {
    let bytes = data
        .get(offset..offset + 4)
        .expect("read_u32 caller must validate bounds");
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

pub fn read_f64(data: &[u8], offset: usize) -> f64 {
    let bytes = data
        .get(offset..offset + 8)
        .expect("read_f64 caller must validate bounds");
    f64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}
