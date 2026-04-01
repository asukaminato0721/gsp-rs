use super::{
    HeaderRecord, IndexedPathRecord, ObjectGroupHeader, PaletteEntryRecord, PointRecord,
    SymbolSlotRecord,
};

pub fn decode_header_record(payload: &[u8]) -> Option<HeaderRecord> {
    if payload.len() != 0x1c {
        return None;
    }

    let words_u16 = (0..payload.len())
        .step_by(2)
        .map(|offset| read_u16(payload, offset))
        .collect::<Vec<_>>();
    let words_u32 = (0..payload.len())
        .step_by(4)
        .map(|offset| read_u32(payload, offset))
        .collect::<Vec<_>>();

    Some(HeaderRecord {
        words_u16,
        words_u32,
    })
}

pub fn decode_symbol_slot(record_type: u32, payload: &[u8]) -> Option<SymbolSlotRecord> {
    if payload.len() != 6 {
        return None;
    }

    Some(SymbolSlotRecord {
        slot_index: record_type - 0x095f,
        value: read_u16(payload, 0),
        flag: read_u16(payload, 2),
        reserved: read_u16(payload, 4),
    })
}

pub fn decode_palette_entry(payload: &[u8]) -> Option<PaletteEntryRecord> {
    if payload.len() != 6 {
        return None;
    }

    Some(PaletteEntryRecord {
        slot_index: read_u16(payload, 0),
        rgba: [payload[2], payload[3], payload[4], payload[5]],
    })
}

pub fn decode_point_record(payload: &[u8]) -> Option<PointRecord> {
    if payload.len() != 16 {
        return None;
    }

    Some(PointRecord {
        x: read_f64(payload, 0),
        y: read_f64(payload, 8),
    })
}

pub fn decode_indexed_path(record_type: u32, payload: &[u8]) -> Option<IndexedPathRecord> {
    if payload.len() < 4 || !payload.len().is_multiple_of(4) {
        return None;
    }

    let count = read_u32(payload, 0) as usize;
    if payload.len() != (count + 1) * 4 {
        return None;
    }

    let refs = (0..count)
        .map(|index| read_u32(payload, 4 + index * 4) as usize)
        .collect::<Vec<_>>();

    Some(IndexedPathRecord { record_type, refs })
}

pub fn decode_object_group_header(payload: &[u8]) -> Option<ObjectGroupHeader> {
    match payload.len() {
        0x10 => Some(ObjectGroupHeader {
            class_id: read_u32(payload, 0),
            flags: read_u32(payload, 4),
            style_a: read_u32(payload, 8),
            style_b: read_u32(payload, 12),
            style_c: 0,
        }),
        0x1c => Some(ObjectGroupHeader {
            class_id: read_u32(payload, 0),
            flags: read_u32(payload, 4),
            style_a: read_u32(payload, 8),
            style_b: read_u32(payload, 12),
            style_c: read_u32(payload, 16),
        }),
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
