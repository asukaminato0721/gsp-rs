use super::{
    ObjectGroup, ParseError, Record, decode_object_aux_u16, decode_object_aux_words,
    decode_object_group_header,
};

pub(super) fn validate_object_groups(records: &[Record], data: &[u8]) -> Result<(), ParseError> {
    let mut index = 0;
    while index < records.len() {
        let record = &records[index];
        if record.record_type != 0x07d0 {
            index += 1;
            continue;
        }
        if decode_object_group_header(record.payload(data)).is_none() {
            return Err(ParseError::InvalidObjectGroupHeader {
                offset: record.offset,
                length: record.length,
            });
        }
        let Some(relative_end) = records[index + 1..]
            .iter()
            .position(|record| matches!(record.record_type, 0x07d7 | 0x07d0 | 0x0387))
        else {
            return Err(ParseError::UnterminatedObjectGroup {
                offset: record.offset,
            });
        };
        if records[index + relative_end + 1].record_type != 0x07d7 {
            return Err(ParseError::UnterminatedObjectGroup {
                offset: record.offset,
            });
        }
        index += relative_end + 2;
    }
    Ok(())
}

pub(super) fn collect_object_groups(records: &[Record], data: &[u8]) -> Vec<ObjectGroup> {
    let mut groups = Vec::new();
    let mut index = 0usize;

    while index < records.len() {
        let record = &records[index];
        if record.record_type != 0x07d0 {
            index += 1;
            continue;
        }

        let header = decode_object_group_header(record.payload(data))
            .expect("GspFile::parse validates every object-group header");

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

        let object_aux_u16 = group_records
            .iter()
            .find(|record| record.record_type == 0x07d8)
            .and_then(|record| decode_object_aux_u16(record.payload(data)));
        let object_aux_0x07d6_words = group_records
            .iter()
            .find(|record| record.record_type == 0x07d6)
            .and_then(|record| decode_object_aux_words(record.payload(data)));

        groups.push(ObjectGroup {
            ordinal: groups.len() + 1,
            start_offset,
            end_offset,
            header,
            object_aux_0x07d6_words,
            object_aux_u16,
            records: group_records,
        });
        index = cursor;
    }

    groups
}
