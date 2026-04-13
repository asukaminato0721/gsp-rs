use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u16, read_u32};
use crate::runtime::geometry::GraphTransform;
use crate::runtime::scene::SceneImage;

use super::payload_debug_source;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub(super) fn collect_scene_images(
    file: &GspFile,
    groups: &[ObjectGroup],
    graph: &Option<GraphTransform>,
) -> Vec<SceneImage> {
    let png_blobs = collect_embedded_pngs(&file.data);

    groups
        .iter()
        .filter_map(|group| decode_image_group(file, groups, group, graph, &png_blobs))
        .collect()
}

fn decode_image_group(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    _graph: &Option<GraphTransform>,
    png_blobs: &[Vec<u8>],
) -> Option<SceneImage> {
    if (group.header.kind()) != crate::format::GroupKind::Point {
        return None;
    }

    let size_payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x090c)
        .map(|record| record.payload(&file.data))?;
    let transform_payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x08a8)
        .map(|record| record.payload(&file.data))?;
    let resource_payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x1f44)
        .map(|record| record.payload(&file.data))?;
    if size_payload.len() < 8 || transform_payload.len() < 48 || resource_payload.len() < 2 {
        return None;
    }

    let width = read_u32(size_payload, 0) as f64;
    let height = read_u32(size_payload, 4) as f64;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    let scale_x = read_f64(transform_payload, 0);
    let shear_x = read_f64(transform_payload, 8);
    let translate_x = read_f64(transform_payload, 16);
    let shear_y = read_f64(transform_payload, 24);
    let scale_y = read_f64(transform_payload, 32);
    let translate_y = read_f64(transform_payload, 40);
    if !scale_x.is_finite()
        || !shear_x.is_finite()
        || !translate_x.is_finite()
        || !shear_y.is_finite()
        || !scale_y.is_finite()
        || !translate_y.is_finite()
    {
        return None;
    }

    // Legacy image payloads in the fixtures use an axis-aligned affine matrix.
    if shear_x.abs() > 1e-6 || shear_y.abs() > 1e-6 {
        return None;
    }

    let resource_index = read_u16(resource_payload, 0) as usize;
    let png = png_blobs.get(resource_index)?;
    let src = format!("data:image/png;base64,{}", crate::util::base64_encode(png));

    let raw_top_left = PointRecord {
        x: translate_x,
        y: translate_y,
    };
    let raw_bottom_right = PointRecord {
        x: translate_x + width * scale_x,
        y: translate_y + height * scale_y,
    };

    Some(SceneImage {
        top_left: raw_top_left,
        bottom_right: raw_bottom_right,
        src,
        screen_space: true,
        debug: Some(payload_debug_source(group)),
    })
}

fn collect_embedded_pngs(data: &[u8]) -> Vec<Vec<u8>> {
    let mut pngs = Vec::new();
    let mut search_from = 0usize;

    while let Some(start) = find_subsequence(&data[search_from..], PNG_SIGNATURE) {
        let absolute_start = search_from + start;
        let Some(end) = png_end(data, absolute_start) else {
            break;
        };
        pngs.push(data[absolute_start..end].to_vec());
        search_from = end;
    }

    pngs
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn png_end(data: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start.checked_add(PNG_SIGNATURE.len())?;
    while cursor.checked_add(8)? <= data.len() {
        let length = u32::from_be_bytes(data.get(cursor..cursor + 4)?.try_into().ok()?) as usize;
        let chunk_start = cursor.checked_add(8)?;
        let chunk_end = chunk_start.checked_add(length)?;
        let crc_end = chunk_end.checked_add(4)?;
        if crc_end > data.len() {
            return None;
        }
        let chunk_type = &data[cursor + 4..cursor + 8];
        cursor = crc_end;
        if chunk_type == b"IEND" {
            return Some(cursor);
        }
    }
    None
}
