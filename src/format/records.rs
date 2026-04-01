use super::{Record, read_u32};

pub fn parse_records(data: &[u8]) -> Result<Vec<Record>, String> {
    let mut records = Vec::new();
    let mut offset = 4usize;

    while offset < data.len() {
        if offset + 8 > data.len() {
            return Err(format!(
                "truncated record header at 0x{offset:x}: {} trailing byte(s)",
                data.len() - offset
            ));
        }

        let length = read_u32(data, offset);
        let record_type = read_u32(data, offset + 4);
        let payload_start = offset + 8;
        let payload_end = payload_start
            .checked_add(length as usize)
            .ok_or_else(|| format!("record at 0x{offset:x} overflows usize"))?;

        if payload_end > data.len() {
            return Err(format!(
                "record at 0x{offset:x} extends past EOF: len=0x{length:x}, end=0x{payload_end:x}, file=0x{:x}",
                data.len()
            ));
        }

        records.push(Record {
            offset,
            length,
            record_type,
            payload_range: payload_start..payload_end,
        });

        offset = align_record_end(payload_end);
    }

    Ok(records)
}

fn align_record_end(payload_end: usize) -> usize {
    if payload_end.is_multiple_of(2) {
        payload_end
    } else {
        payload_end + 1
    }
}

pub fn record_name(record_type: u32) -> &'static str {
    match record_type {
        0x0384 => "GSP_HEADER",
        0x0385 => "END_MARKER_0x385",
        0x0386 => "COMPATIBILITY_BUNDLE",
        0x0387 => "SECTION_MARKER_0x387",
        0x03e8 => "DOC_PREAMBLE_0x3e8",
        0x03e9 => "DOC_TRAILER_0x3e9",
        0x03ec => "OLE_OR_ITEM_REF_0x3ec",
        0x03ed => "OLE_OR_ITEM_REF_0x3ed",
        0x03ee => "RESOURCE_BLOCK_0x3ee",
        0x03ef => "RESOURCE_BLOCK_0x3ef",
        0x07d0 => "OBJECT_GROUP_BEGIN",
        0x07d2 => "OBJECT_INDEX_0x7d2",
        0x07d3 => "OBJECT_INDEX_0x7d3",
        0x07d5 => "OBJECT_AUX_0x7d5",
        0x07d6 => "OBJECT_AUX_0x7d6",
        0x07d7 => "OBJECT_MARKER_0x7d7",
        0x07d8 => "OBJECT_AUX_0x7d8",
        0x0899 => "POINT_F64_PAIR",
        0x2328 => "METADATA_0x2328",
        0x2329 => "METADATA_0x2329",
        0x232b => "DISPLAY_SLOT_0x232b",
        0x232c => "BUILD_INFO_STRING",
        0x232d => "METADATA_0x232d",
        0x232e => "METADATA_0x232e",
        0x232f => "TEXT_BLOB_0x232f",
        0x2330 => "BUILD_INFO_EXTRA",
        0x2331 => "METADATA_0x2331",
        0x2724 => "PALETTE_ENTRY",
        0x273c => "FONT_ENTRY",
        0x095f..=0x0973 => "SYMBOL_SLOT",
        _ => "UNKNOWN",
    }
}
