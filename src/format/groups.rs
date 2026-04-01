use super::{ObjectGroup, Record, decode_object_group_header};

pub(super) fn collect_object_groups(records: &[Record], data: &[u8]) -> Vec<ObjectGroup> {
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
