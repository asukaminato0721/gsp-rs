use super::{
    GspFile, ObjectGroup, TransformBinding, TransformBindingKind,
    decode_angle_parameter_value_for_group,
};
use crate::runtime::extract::find_indexed_path;
use crate::runtime::extract::points::editable_non_graph_parameter_name_for_group;

pub(crate) fn decode_transform_binding(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<TransformBinding> {
    let kind = group.header.class_id & 0xffff;
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let center_group_index = path.refs.get(1)?.checked_sub(1)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;

    let kind = match kind {
        27 => {
            let angle_degrees = if payload.len() >= 28 {
                let angle = super::read_f64(payload, 20);
                if angle.is_finite() {
                    angle
                } else {
                    return None;
                }
            } else {
                let cos = super::read_f64(payload, 4);
                let sin = super::read_f64(payload, 12);
                sin.atan2(cos).to_degrees()
            };
            TransformBindingKind::Rotate {
                angle_degrees,
                parameter_name: None,
            }
        }
        30 => {
            if payload.len() < 12 {
                return None;
            }
            let factor = super::read_f64(payload, 4);
            if !factor.is_finite() {
                return None;
            }
            TransformBindingKind::Scale { factor }
        }
        _ => return None,
    };

    Some(TransformBinding {
        source_group_index,
        center_group_index,
        kind,
    })
}

pub(crate) fn decode_parameter_rotation_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<TransformBinding> {
    if (group.header.class_id & 0xffff) != 29 {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let center_group_index = path.refs.get(1)?.checked_sub(1)?;
    let angle_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
    if (angle_group.header.class_id & 0xffff) != 0 {
        return None;
    }
    let angle_degrees = decode_angle_parameter_value_for_group(file, angle_group)?;
    if !angle_degrees.is_finite() {
        return None;
    }
    let parameter_name = editable_non_graph_parameter_name_for_group(file, angle_group);

    Some(TransformBinding {
        source_group_index,
        center_group_index,
        kind: TransformBindingKind::Rotate {
            angle_degrees,
            parameter_name,
        },
    })
}
