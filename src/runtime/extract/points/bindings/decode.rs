use super::{
    GspFile, ObjectGroup, TransformBinding, TransformBindingKind,
    decode_angle_parameter_value_for_group,
};
use crate::runtime::extract::points::editable_non_graph_parameter_name_for_group;
use crate::runtime::extract::{find_indexed_path, try_find_indexed_path};
use crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum TransformBindingDecodeError {
    #[error("missing indexed path for transform binding")]
    MissingPath,
    #[error("transform path is missing required references")]
    MissingPathRefs,
    #[error("missing 0x07d3 transform payload record")]
    MissingPayloadRecord,
    #[error("unsupported transform kind {0:?}")]
    UnsupportedKind(crate::format::GroupKind),
    #[error("rotation payload contains non-finite angle")]
    NonFiniteRotationAngle,
    #[error("scale payload too short ({0} bytes)")]
    ScalePayloadTooShort(usize),
    #[error("scale payload contains non-finite factor")]
    NonFiniteScaleFactor,
}

pub(crate) fn decode_transform_binding(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<TransformBinding> {
    try_decode_transform_binding(file, group).ok()
}

pub(crate) fn try_decode_transform_binding(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<TransformBinding, TransformBindingDecodeError> {
    let kind = group.header.kind();
    let path = try_find_indexed_path(file, group)
        .map_err(|_| TransformBindingDecodeError::MissingPath)?
        .ok_or(TransformBindingDecodeError::MissingPath)?;
    let source_group_index = path
        .refs
        .first()
        .and_then(|value| value.checked_sub(1))
        .ok_or(TransformBindingDecodeError::MissingPathRefs)?;
    let center_group_index = path
        .refs
        .get(1)
        .and_then(|value| value.checked_sub(1))
        .ok_or(TransformBindingDecodeError::MissingPathRefs)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .ok_or(TransformBindingDecodeError::MissingPayloadRecord)?;

    let kind = match kind {
        crate::format::GroupKind::Rotation => {
            let angle_degrees = if payload.len() >= 28 {
                let angle = super::read_f64(payload, 20);
                if angle.is_finite() {
                    angle
                } else {
                    return Err(TransformBindingDecodeError::NonFiniteRotationAngle);
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
        crate::format::GroupKind::Scale => {
            if payload.len() < 12 {
                return Err(TransformBindingDecodeError::ScalePayloadTooShort(
                    payload.len(),
                ));
            }
            let factor = super::read_f64(payload, 4);
            if !factor.is_finite() {
                return Err(TransformBindingDecodeError::NonFiniteScaleFactor);
            }
            TransformBindingKind::Scale { factor }
        }
        _ => return Err(TransformBindingDecodeError::UnsupportedKind(kind)),
    };

    Ok(TransformBinding {
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
    if (group.header.kind()) != crate::format::GroupKind::ParameterRotation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let center_group_index = path.refs.get(1)?.checked_sub(1)?;
    let angle_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
    if (angle_group.header.kind()) != crate::format::GroupKind::Point {
        return None;
    }
    let angle_degrees = decode_angle_parameter_value_for_group(file, angle_group)?;
    if !angle_degrees.is_finite() {
        return None;
    }
    let parameter_name = editable_non_graph_parameter_name_for_group(file, groups, angle_group);

    Some(TransformBinding {
        source_group_index,
        center_group_index,
        kind: TransformBindingKind::Rotate {
            angle_degrees,
            parameter_name,
        },
    })
}
