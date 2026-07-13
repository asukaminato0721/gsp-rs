use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error("file is too small to be a GSP file: {len} bytes")]
    #[diagnostic(
        code(gsp_rs::format::file_too_small),
        help("expected at least the 4-byte GSP magic and one 8-byte record header")
    )]
    FileTooSmall { len: usize },

    #[error("unexpected magic {found:?}, expected \"GSP4\"")]
    #[diagnostic(
        code(gsp_rs::format::invalid_magic),
        help("this compiler only supports GSP4 payloads")
    )]
    InvalidMagic { found: String },

    #[error("truncated record header at 0x{offset:x}: {trailing} trailing byte(s)")]
    #[diagnostic(
        code(gsp_rs::format::truncated_record_header),
        help("the payload ended before an 8-byte record header could be read")
    )]
    TruncatedRecordHeader { offset: usize, trailing: usize },

    #[error("record at 0x{offset:x} overflows usize")]
    #[diagnostic(
        code(gsp_rs::format::record_overflow),
        help("the record length field is too large for this platform")
    )]
    RecordOverflowsUsize { offset: usize },

    #[error(
        "record at 0x{offset:x} extends past EOF: len=0x{length:x}, end=0x{end:x}, file=0x{file_len:x}"
    )]
    #[diagnostic(
        code(gsp_rs::format::record_past_eof),
        help("the record length field points beyond the available payload bytes")
    )]
    RecordPastEof {
        offset: usize,
        length: u32,
        end: usize,
        file_len: usize,
    },

    #[error("invalid object-group header at 0x{offset:x}: payload length is 0x{length:x}")]
    #[diagnostic(
        code(gsp_rs::format::invalid_object_group_header),
        help("object-group headers must use one of the payload layouts defined by GSP4")
    )]
    InvalidObjectGroupHeader { offset: usize, length: u32 },

    #[error("object group starting at 0x{offset:x} has no 0x07d7 terminator")]
    #[diagnostic(
        code(gsp_rs::format::unterminated_object_group),
        help("the payload ended or a document section began before the object group was closed")
    )]
    UnterminatedObjectGroup { offset: usize },
}
