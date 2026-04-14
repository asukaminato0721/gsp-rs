mod decode;
mod error;
mod group_kind;
mod groups;
mod records;
mod strings;

use std::collections::BTreeMap;
use std::ops::Range;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[allow(unused_imports)]
pub use decode::{
    decode_indexed_path, decode_object_group_header, decode_point_record, read_f64, read_i16,
    read_u16, read_u32,
};
pub use error::ParseError;
pub use group_kind::GroupKind;
#[allow(unused_imports)]
pub use records::{parse_records, record_name};
#[allow(unused_imports)]
pub use strings::{collect_strings, decode_c_string};

#[derive(Debug)]
pub struct GspFile {
    pub magic: String,
    pub data: Vec<u8>,
    pub records: Vec<Record>,
}

impl GspFile {
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() < 12 {
            return Err(ParseError::FileTooSmall { len: data.len() });
        }

        let magic = String::from_utf8_lossy(&data[..4]).to_string();
        if magic != "GSP4" {
            return Err(ParseError::InvalidMagic { found: magic });
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
        groups::collect_object_groups(&self.records, &self.data)
    }

    pub fn document_canvas_size(&self) -> Option<(u32, u32)> {
        let header = self
            .records
            .first()
            .filter(|record| record.record_type == 0x0384)?;
        let payload = header.payload(&self.data);
        if payload.len() < 22 {
            return None;
        }
        let width = u32::from(read_u16(payload, 18));
        let height = u32::from(read_u16(payload, 20));
        (width > 0 && height > 0).then_some((width, height))
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

#[derive(Debug, Clone, Default)]
pub struct PointRecord {
    pub x: f64,
    pub y: f64,
}

impl Add for PointRecord {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for PointRecord {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f64> for PointRecord {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl AddAssign for PointRecord {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl SubAssign for PointRecord {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
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

impl ObjectGroupHeader {
    pub fn kind(&self) -> GroupKind {
        GroupKind::from(self.class_id as u16)
    }

    pub fn kind_id(&self) -> u16 {
        self.kind().raw()
    }

    pub fn is_hidden(&self) -> bool {
        (self.class_id & 0x0001_0000) != 0
    }
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
    pub text: String,
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
    fn decodes_point_record() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&239.0_f64.to_le_bytes());
        payload.extend_from_slice(&205.0_f64.to_le_bytes());
        let point = decode_point_record(&payload).expect("point");
        assert_eq!(point.x, 239.0);
        assert_eq!(point.y, 205.0);
    }

    #[test]
    fn decodes_short_object_group_header() {
        let payload = [
            0x00, 0x00, 0x01, 0x00, //
            0x00, 0x00, 0x01, 0x00, //
            0x04, 0x00, 0x01, 0x00, //
            0xff, 0x00, 0x00, 0x20, //
        ];
        let header = decode_object_group_header(&payload).expect("header");
        assert_eq!(header.class_id, 0x0001_0000);
        assert_eq!(header.flags, 0x0001_0000);
        assert_eq!(header.style_a, 0x0001_0004);
        assert_eq!(header.style_b, 0x2000_00ff);
        assert_eq!(header.style_c, 0);
    }

    #[test]
    fn parses_odd_length_record_stream_with_padding() {
        let mut data = Vec::new();
        data.extend_from_slice(b"GSP4");
        data.extend_from_slice(&3_u32.to_le_bytes());
        data.extend_from_slice(&0x1111_u32.to_le_bytes());
        data.extend_from_slice(&[1, 2, 3]);
        data.push(0);
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0x2222_u32.to_le_bytes());

        let file = GspFile::parse(&data).expect("valid padded file");
        assert_eq!(file.records.len(), 2);
        assert_eq!(file.records[0].payload(&file.data), &[1, 2, 3]);
        assert_eq!(file.records[1].record_type, 0x2222);
    }
}
