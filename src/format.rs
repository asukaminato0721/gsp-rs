use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Debug)]
pub struct GspFile {
    pub magic: String,
    pub data: Vec<u8>,
    pub records: Vec<Record>,
}

impl GspFile {
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 12 {
            return Err(format!(
                "file is too small to be a GSP file: {} bytes",
                data.len()
            ));
        }

        let magic = String::from_utf8_lossy(&data[..4]).to_string();
        if magic != "GSP4" {
            return Err(format!("unexpected magic {magic:?}, expected \"GSP4\""));
        }

        let records = parse_records(data)?;
        Ok(Self {
            magic,
            data: data.to_vec(),
            records,
        })
    }

    pub fn record_type_counts(&self) -> Vec<RecordTypeCount> {
        let mut counts = BTreeMap::<u32, usize>::new();
        for record in &self.records {
            *counts.entry(record.record_type).or_default() += 1;
        }

        let mut entries = counts
            .into_iter()
            .map(|(record_type, count)| RecordTypeCount { record_type, count })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.record_type.cmp(&right.record_type))
        });
        entries
    }

    pub fn point_records(&self) -> Vec<PointRecord> {
        self.records
            .iter()
            .filter_map(|record| {
                if record.record_type == 0x0899 {
                    decode_point_record(record.payload(&self.data))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn indexed_paths(&self) -> Vec<IndexedPathRecord> {
        self.records
            .iter()
            .filter_map(|record| match record.record_type {
                0x07d2 | 0x07d3 => {
                    decode_indexed_path(record.record_type, record.payload(&self.data))
                }
                _ => None,
            })
            .collect()
    }

    pub fn object_groups(&self) -> Vec<ObjectGroup> {
        collect_object_groups(&self.records, &self.data)
    }
}

#[derive(Debug, Clone)]
pub struct Record {
    pub offset: usize,
    pub length: u32,
    pub record_type: u32,
    pub payload_range: Range<usize>,
}

impl Record {
    pub fn payload<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        &data[self.payload_range.clone()]
    }
}

#[derive(Debug)]
pub struct RecordTypeCount {
    pub record_type: u32,
    pub count: usize,
}

#[derive(Debug)]
pub struct HeaderRecord {
    pub words_u16: Vec<u16>,
    pub words_u32: Vec<u32>,
}

#[derive(Debug)]
pub struct SymbolSlotRecord {
    pub slot_index: u32,
    pub value: u16,
    pub flag: u16,
    pub reserved: u16,
}

#[derive(Debug)]
pub struct PaletteEntryRecord {
    pub slot_index: u16,
    pub rgba: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct PointRecord {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct IndexedPathRecord {
    #[allow(dead_code)]
    pub record_type: u32,
    pub refs: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct ObjectGroupHeader {
    pub class_id: u32,
    pub flags: u32,
    pub style_a: u32,
    pub style_b: u32,
    pub style_c: u32,
}

#[derive(Debug, Clone)]
pub struct ObjectGroup {
    #[allow(dead_code)]
    pub ordinal: usize,
    #[allow(dead_code)]
    pub start_offset: usize,
    #[allow(dead_code)]
    pub end_offset: usize,
    pub header: ObjectGroupHeader,
    pub records: Vec<Record>,
}

#[derive(Debug, Clone)]
pub struct ExtractedString {
    pub offset: usize,
    pub byte_len: usize,
    pub text: String,
    pub prefix_len16: Option<u16>,
    pub prefix_len32: Option<u32>,
}

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

        offset = payload_end;
    }

    Ok(records)
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
    if payload.len() != 0x1c {
        return None;
    }

    Some(ObjectGroupHeader {
        class_id: read_u32(payload, 0),
        flags: read_u32(payload, 4),
        style_a: read_u32(payload, 8),
        style_b: read_u32(payload, 12),
        style_c: read_u32(payload, 16),
    })
}

pub fn decode_c_string(payload: &[u8]) -> Option<String> {
    let nul = payload.iter().position(|byte| *byte == 0)?;
    let bytes = &payload[..nul];
    if bytes.len() < 4 {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(ToString::to_string)
}

pub fn collect_strings(data: &[u8]) -> Vec<ExtractedString> {
    let mut strings = Vec::new();
    let mut offset = 0usize;

    while offset < data.len() {
        let Some(end) = data[offset..].iter().position(|byte| *byte == 0) else {
            break;
        };
        let end = offset + end;
        let bytes = &data[offset..end];

        if is_useful_string(bytes) {
            let text = String::from_utf8_lossy(bytes).to_string();
            strings.push(ExtractedString {
                offset,
                byte_len: bytes.len(),
                prefix_len16: prefix_len16(data, offset, bytes.len()),
                prefix_len32: prefix_len32(data, offset, bytes.len()),
                text,
            });
        }

        offset = end + 1;
    }

    strings
}

pub fn read_u16(data: &[u8], offset: usize) -> u16 {
    let bytes = data
        .get(offset..offset + 2)
        .expect("read_u16 caller must validate bounds");
    u16::from_le_bytes([bytes[0], bytes[1]])
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

fn collect_object_groups(records: &[Record], data: &[u8]) -> Vec<ObjectGroup> {
    let mut groups = Vec::new();
    let mut index = 0usize;

    while index < records.len() {
        let record = &records[index];
        if record.record_type != 0x07d0 {
            index += 1;
            continue;
        }

        let Some(header) = decode_object_group_header(record.payload(data)) else {
            index += 1;
            continue;
        };

        let start_offset = record.offset;
        let mut group_records = Vec::new();
        let mut cursor = index + 1;
        let mut end_offset = record.offset + 8 + record.length as usize;

        while cursor < records.len() {
            let current = &records[cursor];
            group_records.push(current.clone());
            end_offset = current.offset + 8 + current.length as usize;
            cursor += 1;
            if current.record_type == 0x07d7 {
                break;
            }
        }

        groups.push(ObjectGroup {
            ordinal: groups.len() + 1,
            start_offset,
            end_offset,
            header,
            records: group_records,
        });
        index = cursor;
    }

    groups
}

fn is_useful_string(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }

    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };

    let mut useful = 0usize;
    for ch in text.chars() {
        if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' {
            return false;
        }
        if ch.is_alphanumeric() || !ch.is_ascii() {
            useful += 1;
        }
    }

    useful >= 3
}

fn prefix_len16(data: &[u8], offset: usize, len: usize) -> Option<u16> {
    if offset < 2 {
        return None;
    }
    let value = read_u16(data, offset - 2);
    if usize::from(value) == len || usize::from(value) == len + 1 {
        Some(value)
    } else {
        None
    }
}

fn prefix_len32(data: &[u8], offset: usize, len: usize) -> Option<u32> {
    if offset < 4 {
        return None;
    }
    let value = read_u32(data, offset - 4);
    if value as usize == len || value as usize == len + 1 {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_record_stream() {
        let mut data = Vec::new();
        data.extend_from_slice(b"GSP4");
        data.extend_from_slice(&4_u32.to_le_bytes());
        data.extend_from_slice(&0x1111_u32.to_le_bytes());
        data.extend_from_slice(&[1, 2, 3, 4]);
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0x2222_u32.to_le_bytes());

        let file = GspFile::parse(&data).expect("valid file");
        assert_eq!(file.records.len(), 2);
        assert_eq!(file.records[0].record_type, 0x1111);
        assert_eq!(file.records[0].payload(&file.data), &[1, 2, 3, 4]);
        assert_eq!(file.records[1].record_type, 0x2222);
        assert!(file.records[1].payload(&file.data).is_empty());
    }

    #[test]
    fn decodes_symbol_slot_record() {
        let payload = [0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let slot = decode_symbol_slot(0x0960, &payload).expect("slot");
        assert_eq!(slot.slot_index, 1);
        assert_eq!(slot.value, 0);
        assert_eq!(slot.flag, 1);
        assert_eq!(slot.reserved, 0);
    }

    #[test]
    fn decodes_point_record() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&239.0_f64.to_le_bytes());
        payload.extend_from_slice(&205.0_f64.to_le_bytes());
        let point = decode_point_record(&payload).expect("point");
        assert_eq!(point.x, 239.0);
        assert_eq!(point.y, 205.0);
    }
}
