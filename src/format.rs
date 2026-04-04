mod decode;
mod groups;
mod records;
mod strings;

use std::collections::BTreeMap;
use std::ops::Range;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[allow(unused_imports)]
pub use decode::{
    decode_header_record, decode_indexed_path, decode_object_group_header, decode_palette_entry,
    decode_point_record, decode_symbol_slot, read_f64, read_i16, read_u16, read_u32,
};
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
        groups::collect_object_groups(&self.records, &self.data)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupKind {
    Point,
    Midpoint,
    Segment,
    Circle,
    LineKind5,
    LineKind6,
    LineKind7,
    Polygon,
    PointConstraint,
    Translation,
    CartesianOffsetPoint,
    PolarOffsetPoint,
    Rotation,
    ParameterRotation,
    Scale,
    Reflection,
    PointTrace,
    GraphObject40,
    Kind51,
    FunctionExpr,
    GraphCalibrationX,
    GraphCalibrationY,
    MeasurementLine,
    ActionButton,
    Line,
    Ray,
    OffsetAnchor,
    CoordinatePoint,
    FunctionPlot,
    ButtonLabel,
    DerivedSegment24,
    AffineIteration,
    IterationBinding,
    DerivativeFunction,
    RegularPolygonIteration,
    LabelIterationSeed,
    ParameterAnchor,
    ParameterControlledPoint,
    ArcOnCircle,
    ThreePointArc,
    CoordinateTrace,
    AxisLine,
    DerivedSegment75,
    Unknown(u16),
}

impl From<u16> for GroupKind {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::Point,
            1 => Self::Midpoint,
            2 => Self::Segment,
            3 => Self::Circle,
            5 => Self::LineKind5,
            6 => Self::LineKind6,
            7 => Self::LineKind7,
            8 => Self::Polygon,
            15 => Self::PointConstraint,
            16 => Self::Translation,
            17 => Self::CartesianOffsetPoint,
            21 => Self::PolarOffsetPoint,
            24 => Self::DerivedSegment24,
            27 => Self::Rotation,
            29 => Self::ParameterRotation,
            30 => Self::Scale,
            34 => Self::Reflection,
            35 => Self::PointTrace,
            40 => Self::GraphObject40,
            51 => Self::Kind51,
            48 => Self::FunctionExpr,
            52 => Self::GraphCalibrationX,
            54 => Self::GraphCalibrationY,
            58 => Self::MeasurementLine,
            61 => Self::AxisLine,
            62 => Self::ActionButton,
            63 => Self::Line,
            64 => Self::Ray,
            67 => Self::OffsetAnchor,
            69 => Self::CoordinatePoint,
            72 => Self::FunctionPlot,
            73 => Self::ButtonLabel,
            75 => Self::DerivedSegment75,
            76 => Self::AffineIteration,
            77 => Self::IterationBinding,
            78 => Self::DerivativeFunction,
            89 => Self::RegularPolygonIteration,
            90 => Self::LabelIterationSeed,
            94 => Self::ParameterAnchor,
            95 => Self::ParameterControlledPoint,
            79 => Self::ArcOnCircle,
            81 => Self::ThreePointArc,
            97 => Self::CoordinateTrace,
            other => Self::Unknown(other),
        }
    }
}

impl GroupKind {
    pub fn raw(self) -> u16 {
        match self {
            Self::Point => 0,
            Self::Midpoint => 1,
            Self::Segment => 2,
            Self::Circle => 3,
            Self::LineKind5 => 5,
            Self::LineKind6 => 6,
            Self::LineKind7 => 7,
            Self::Polygon => 8,
            Self::PointConstraint => 15,
            Self::Translation => 16,
            Self::CartesianOffsetPoint => 17,
            Self::PolarOffsetPoint => 21,
            Self::DerivedSegment24 => 24,
            Self::Rotation => 27,
            Self::ParameterRotation => 29,
            Self::Scale => 30,
            Self::Reflection => 34,
            Self::PointTrace => 35,
            Self::GraphObject40 => 40,
            Self::Kind51 => 51,
            Self::FunctionExpr => 48,
            Self::GraphCalibrationX => 52,
            Self::GraphCalibrationY => 54,
            Self::MeasurementLine => 58,
            Self::AxisLine => 61,
            Self::ActionButton => 62,
            Self::Line => 63,
            Self::Ray => 64,
            Self::OffsetAnchor => 67,
            Self::CoordinatePoint => 69,
            Self::FunctionPlot => 72,
            Self::ButtonLabel => 73,
            Self::DerivedSegment75 => 75,
            Self::AffineIteration => 76,
            Self::IterationBinding => 77,
            Self::DerivativeFunction => 78,
            Self::RegularPolygonIteration => 89,
            Self::LabelIterationSeed => 90,
            Self::ParameterAnchor => 94,
            Self::ParameterControlledPoint => 95,
            Self::ArcOnCircle => 79,
            Self::ThreePointArc => 81,
            Self::CoordinateTrace => 97,
            Self::Unknown(other) => other,
        }
    }
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
