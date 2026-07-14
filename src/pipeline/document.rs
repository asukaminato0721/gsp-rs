use crate::export::html::{
    StandaloneHtmlPage, render_document_scene_json, render_scene_json, render_standalone_html_pages,
};
use crate::gsp;
use crate::runtime::build_scene_checked;
use crate::runtime::scene::{AnimatedPointTarget, ButtonAction, PointAnimation, Scene};
use miette::{Result, WrapErr, miette};
use std::collections::BTreeMap;

pub fn compile_bytes_to_html_document(data: &[u8], width: u32, height: u32) -> Result<String> {
    compile_bytes_to_html_document_with_reference(data, width, height, None)
}

pub(crate) fn compile_bytes_to_html_document_with_reference(
    data: &[u8],
    width: u32,
    height: u32,
    reference_htm: Option<&str>,
) -> Result<String> {
    let file = gsp::parse(data).map_err(miette::Report::new)?;
    let compiled_pages = compile_pages(&file, width, height, reference_htm)?;
    let html_pages = compiled_pages
        .iter()
        .map(|page| StandaloneHtmlPage {
            title: &page.title,
            scene: &page.scene,
            width: page.width,
            height: page.height,
            document_layout: page.document_layout,
        })
        .collect::<Vec<_>>();
    Ok(render_standalone_html_pages(&html_pages))
}

pub fn compile_bytes_to_scene_json(data: &[u8], width: u32, height: u32) -> Result<String> {
    compile_bytes_to_scene_json_with_reference(data, width, height, None)
}

pub(crate) fn compile_bytes_to_scene_json_with_reference(
    data: &[u8],
    width: u32,
    height: u32,
    reference_htm: Option<&str>,
) -> Result<String> {
    let file = gsp::parse(data).map_err(miette::Report::new)?;
    let compiled_pages = compile_pages(&file, width, height, reference_htm)?;
    if let [page] = compiled_pages.as_slice() {
        return Ok(render_scene_json(
            &page.scene,
            page.width,
            page.height,
            true,
        ));
    }
    let html_pages = as_html_pages(&compiled_pages);
    Ok(render_document_scene_json(&html_pages))
}

fn compile_pages(
    file: &crate::format::GspFile,
    width: u32,
    height: u32,
    reference_htm: Option<&str>,
) -> Result<Vec<CompiledHtmlPage>> {
    let page_files = file.page_files();
    let mut compiled_pages = Vec::with_capacity(page_files.len());
    for (index, page_file) in page_files.iter().enumerate() {
        let mut scene = build_scene_checked(page_file)
            .map_err(|error| miette!("{error:#}"))
            .wrap_err_with(|| format!("failed to build scene from page {}", index + 1))?;
        if let Some(reference_htm) = reference_htm {
            apply_reference_animation_definitions(&mut scene, reference_htm);
            apply_reference_move_definitions(&mut scene, reference_htm);
        }
        let document_layout = is_document_layout(page_file, &scene);
        let (width, height) = export_dimensions(page_file, &scene, width, height);
        compiled_pages.push(CompiledHtmlPage {
            title: format!("Page {}", index + 1),
            scene,
            width,
            height,
            document_layout,
        });
    }
    Ok(compiled_pages)
}

#[derive(Debug)]
struct AnimationDefinition {
    point_group_ordinals: Vec<usize>,
    speeds: Vec<u32>,
    directions: Vec<i32>,
    repeats: Vec<bool>,
}

fn apply_reference_animation_definitions(scene: &mut Scene, htm: &str) {
    let definitions = parse_animation_definitions(htm);
    let point_indices_by_ordinal = scene
        .points
        .iter()
        .enumerate()
        .filter_map(|(index, point)| {
            point
                .debug
                .as_ref()
                .map(|debug| (debug.group_ordinal, index))
        })
        .collect::<BTreeMap<_, _>>();
    for button in &mut scene.buttons {
        let Some(group_ordinal) = button.debug.as_ref().map(|debug| debug.group_ordinal) else {
            continue;
        };
        let Some(definition) = definitions.get(&group_ordinal) else {
            continue;
        };
        if !matches!(
            button.action,
            ButtonAction::AnimatePoint { .. } | ButtonAction::AnimatePoints { .. }
        ) {
            continue;
        }
        let targets = definition
            .point_group_ordinals
            .iter()
            .enumerate()
            .filter_map(|(index, ordinal)| {
                Some(AnimatedPointTarget {
                    point_index: *point_indices_by_ordinal.get(ordinal)?,
                    animation: animation_at(definition, index),
                })
            })
            .collect::<Vec<_>>();
        match targets.as_slice() {
            [] => {}
            [target] => {
                button.action = ButtonAction::AnimatePoint {
                    point_index: target.point_index,
                    animation: target.animation.clone(),
                };
            }
            _ => button.action = ButtonAction::AnimatePoints { targets },
        }
    }
}

fn apply_reference_move_definitions(scene: &mut Scene, htm: &str) {
    let speeds = htm.lines().filter_map(parse_move_button_speed);
    let actions = scene
        .buttons
        .iter_mut()
        .filter_map(|button| match &mut button.action {
            ButtonAction::MovePoint { speed, .. } | ButtonAction::MovePoints { speed, .. } => {
                Some(speed)
            }
            _ => None,
        });
    for (speed, reference_speed) in actions.zip(speeds) {
        *speed = reference_speed;
    }
}

fn parse_move_button_speed(line: &str) -> Option<u32> {
    let trimmed = line.trim();
    trimmed.strip_prefix('{')?;
    let move_start = trimmed.find("MoveButton(")? + "MoveButton".len();
    let arguments = parenthesized_groups(&trimmed[move_start..]);
    arguments.first()?.split(',').nth(2)?.trim().parse().ok()
}

fn animation_at(definition: &AnimationDefinition, index: usize) -> Option<PointAnimation> {
    fn value_at<T: Copy>(values: &[T], index: usize) -> Option<T> {
        match values {
            [value] => Some(*value),
            _ => values.get(index).copied(),
        }
    }
    let speed = value_at(&definition.speeds, index)?;
    let direction = value_at(&definition.directions, index)?;
    let repeat = value_at(&definition.repeats, index)?;
    (speed > 0).then_some(PointAnimation {
        speed,
        direction,
        repeat,
    })
}

fn parse_animation_definitions(htm: &str) -> BTreeMap<usize, AnimationDefinition> {
    htm.lines()
        .filter_map(parse_animation_definition_line)
        .collect()
}

fn parse_animation_definition_line(line: &str) -> Option<(usize, AnimationDefinition)> {
    let trimmed = line.trim();
    let ordinal_end = trimmed.strip_prefix('{')?.find('}')?;
    let ordinal = trimmed[1..=ordinal_end].parse::<usize>().ok()?;
    let animate_start = trimmed.find("AnimateButton(")? + "AnimateButton".len();
    let groups = parenthesized_groups(&trimmed[animate_start..]);
    if groups.len() < 5 {
        return None;
    }
    let target_pairs = parse_integer_list(groups[1])?;
    let mut target_chunks = target_pairs.chunks_exact(2);
    let point_group_ordinals = target_chunks
        .by_ref()
        .map(|pair| usize::try_from(pair[0]))
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    if !target_chunks.remainder().is_empty() {
        return None;
    }
    let speeds = parse_integer_list(groups[2])?
        .into_iter()
        .map(u32::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    let directions = parse_integer_list(groups[3])?;
    let repeats = parse_integer_list(groups[4])?
        .into_iter()
        .map(|value| value != 0)
        .collect();
    Some((
        ordinal,
        AnimationDefinition {
            point_group_ordinals,
            speeds,
            directions,
            repeats,
        },
    ))
}

fn parenthesized_groups(source: &str) -> Vec<&str> {
    let mut groups = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut quoted = false;
    for (index, character) in source.char_indices() {
        if character == '\'' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match character {
            '(' => {
                if depth == 0 {
                    start = Some(index + 1);
                }
                depth += 1;
            }
            ')' if depth > 0 => {
                depth -= 1;
                if depth == 0
                    && let Some(start) = start.take()
                {
                    groups.push(&source[start..index]);
                }
            }
            _ => {}
        }
    }
    groups
}

fn parse_integer_list(source: &str) -> Option<Vec<i32>> {
    let values = source
        .split(',')
        .map(str::trim)
        .map(str::parse::<i32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    (!values.is_empty()).then_some(values)
}

fn as_html_pages(pages: &[CompiledHtmlPage]) -> Vec<StandaloneHtmlPage<'_>> {
    pages
        .iter()
        .map(|page| StandaloneHtmlPage {
            title: &page.title,
            scene: &page.scene,
            width: page.width,
            height: page.height,
            document_layout: page.document_layout,
        })
        .collect()
}

struct CompiledHtmlPage {
    title: String,
    scene: Scene,
    width: u32,
    height: u32,
    document_layout: bool,
}

fn export_dimensions(
    file: &crate::format::GspFile,
    scene: &crate::runtime::scene::Scene,
    fallback_width: u32,
    fallback_height: u32,
) -> (u32, u32) {
    if is_document_layout(file, scene)
        && let Some((width, height)) = file.document_canvas_size()
    {
        return (width, height);
    }
    (fallback_width, fallback_height)
}

fn is_document_layout(file: &crate::format::GspFile, scene: &crate::runtime::scene::Scene) -> bool {
    !scene.graph_mode && file.document_canvas_size().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_per_target_animation_options_from_reference_htm() {
        let line = "{52} AnimateButton(10,5,'动画点')(10,9,19,9)(5,3)(0,1)(1,0)[color(255,128,0)];";
        let definitions = parse_animation_definitions(line);
        let definition = definitions.get(&52).expect("animation definition");
        assert_eq!(definition.point_group_ordinals, [10, 19]);
        assert_eq!(
            animation_at(definition, 0),
            Some(PointAnimation {
                speed: 5,
                direction: 0,
                repeat: true,
            })
        );
        assert_eq!(
            animation_at(definition, 1),
            Some(PointAnimation {
                speed: 3,
                direction: 1,
                repeat: false,
            })
        );
    }

    #[test]
    fn broadcasts_single_animation_option_to_all_targets() {
        let line = "{13} AnimateButton(67,224,'动画点')(8,7)(1)(0)(1);";
        let definitions = parse_animation_definitions(line);
        let definition = definitions.get(&13).expect("animation definition");
        assert_eq!(animation_at(definition, 4).expect("broadcast").speed, 1);
    }

    #[test]
    fn parses_move_button_speed_from_reference_htm() {
        assert_eq!(
            parse_move_button_speed("{24} MoveButton(10,152,3,'')(21,23)[hidden];"),
            Some(3),
        );
        assert_eq!(
            parse_move_button_speed("{46} MoveButton(64,784,0,'初始化')(45,2);"),
            Some(0),
        );
    }

    #[test]
    fn reference_htm_animation_options_reach_scene_json() {
        let data = include_bytes!("../../tests/Samples/未分类档/万花筒.gsp");
        let htm = include_str!("../../tests/Samples/未分类档/万花筒.htm");
        let json = compile_bytes_to_scene_json_with_reference(data, 1200, 800, Some(htm))
            .expect("kaleidoscope should compile with reference htm");
        let scene: serde_json::Value = serde_json::from_str(&json).expect("scene json");
        let button = scene["buttons"]
            .as_array()
            .and_then(|buttons| {
                buttons
                    .iter()
                    .find(|button| button["debug"]["groupOrdinal"].as_u64() == Some(52))
            })
            .expect("AnimateButton #52");
        let targets = button["action"]["targets"]
            .as_array()
            .expect("animation targets");
        assert_eq!(targets.len(), 27);
        assert_eq!(targets[0]["animation"]["speed"].as_u64(), Some(5));
        assert_eq!(targets[1]["animation"]["speed"].as_u64(), Some(3));
        assert_eq!(targets[0]["animation"]["direction"].as_i64(), Some(0));
        assert_eq!(targets[0]["animation"]["repeat"].as_bool(), Some(true));
    }

    #[test]
    fn reference_htm_move_speeds_reach_scene_json_in_construction_order() {
        let data = include_bytes!("../../tests/Samples/个人专栏/李章博作品/一条龙.gsp");
        let htm = include_str!("../../tests/Samples/个人专栏/李章博作品/一条龙.htm");
        let json = compile_bytes_to_scene_json_with_reference(data, 1200, 800, Some(htm))
            .expect("dragon sample should compile with reference htm");
        let scene: serde_json::Value = serde_json::from_str(&json).expect("scene json");
        let speeds = scene["buttons"]
            .as_array()
            .expect("buttons")
            .iter()
            .filter_map(|button| {
                matches!(
                    button["action"]["kind"].as_str(),
                    Some("move-point" | "move-points")
                )
                .then(|| button["action"]["speed"].as_u64())
                .flatten()
            })
            .collect::<Vec<_>>();
        assert_eq!(speeds, vec![3, 3, 3, 3]);
    }
}
