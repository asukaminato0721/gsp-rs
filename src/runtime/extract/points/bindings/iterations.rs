use super::{
    GspFile, ObjectGroup, RawPointIterationFamily, iteration_depth, iteration_state_count,
};
use crate::runtime::extract::find_indexed_path;
use crate::runtime::extract::points::editable_non_graph_parameter_name_for_group;
use crate::runtime::scene::{IterationStateKind, IterationStatePair};

fn mapped_point_index(group_to_point_index: &[Option<usize>], group_index: usize) -> Option<usize> {
    group_to_point_index.get(group_index).copied().flatten()
}

/// Decode point iterations from the payload's state table.
///
/// `ITERATION_DEFINITION` supplies the number of state columns. The indexed path is laid out as
/// `[depth?, sources..., first_images..., carried_objects...]`, where the optional depth driver is
/// present for regular-polygon iteration records. Each `(source, first_image)` pair defines one
/// input/output slot of the operation that is interpreted for every subsequent image.
pub(crate) fn collect_point_iteration_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_to_point_index: &[Option<usize>],
) -> Vec<RawPointIterationFamily> {
    groups
        .iter()
        .filter(|group| group.header.kind() == crate::format::GroupKind::IterationBinding)
        .filter_map(|binding_group| {
            let binding_path = find_indexed_path(file, binding_group)?;
            let output_group_index = binding_path.refs.first()?.checked_sub(1)?;
            let point_index = mapped_point_index(group_to_point_index, output_group_index)?;
            let iteration_group = groups.get(binding_path.refs.get(1)?.checked_sub(1)?)?;
            let state_count = iteration_state_count(file, iteration_group)?;
            if state_count == 0 {
                return None;
            }

            let iteration_path = find_indexed_path(file, iteration_group)?;
            let state_start = usize::from(
                iteration_group.header.kind() == crate::format::GroupKind::RegularPolygonIteration,
            );
            let image_start = state_start.checked_add(state_count)?;
            let carried_start = image_start.checked_add(state_count)?;
            if iteration_path.refs.len() < carried_start
                || !iteration_path.refs[carried_start..].contains(&(output_group_index + 1))
            {
                return None;
            }

            let states = (0..state_count)
                .map(|index| {
                    let source_group_ordinal = *iteration_path.refs.get(state_start + index)?;
                    let image_group_ordinal = *iteration_path.refs.get(image_start + index)?;
                    let source_group = groups.get(source_group_ordinal.checked_sub(1)?)?;
                    let image_group = groups.get(image_group_ordinal.checked_sub(1)?)?;
                    let kind = if image_group.header.kind()
                        == crate::format::GroupKind::FunctionExpr
                        || editable_non_graph_parameter_name_for_group(file, groups, source_group)
                            .is_some()
                    {
                        IterationStateKind::Scalar
                    } else {
                        IterationStateKind::Object
                    };
                    Some(IterationStatePair {
                        source_group_ordinal,
                        image_group_ordinal,
                        kind,
                    })
                })
                .collect::<Option<Vec<_>>>()?;

            let depth_parameter_name = (state_start == 1)
                .then(|| {
                    let ordinal = *iteration_path.refs.first()?;
                    editable_non_graph_parameter_name_for_group(
                        file,
                        groups,
                        groups.get(ordinal.checked_sub(1)?)?,
                    )
                })
                .flatten();

            Some(RawPointIterationFamily::Interpreted {
                point_index,
                states,
                depth_parameter_name,
                depth: iteration_depth(file, iteration_group, 3),
            })
        })
        .collect()
}
