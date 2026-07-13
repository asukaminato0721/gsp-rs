fn payload_function_expr_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    fallback_expr_label: &str,
    visiting: &mut BTreeSet<usize>,
) -> String {
    if !visiting.insert(group.ordinal) {
        return fallback_expr_label.to_string();
    }
    let result = (|| {
        if let Some(display_label) = expanded_fallback_expr_label(
            file,
            groups,
            anchors,
            group,
            fallback_expr_label,
            visiting,
        ) {
            return Some(display_label);
        }
        if let Some(display_label) =
            payload_function_expr_display_label(file, groups, anchors, group, visiting)
        {
            return Some(display_label);
        }
        if let Some(label) = decode_label_name(file, group) {
            return Some(label);
        }
        let path = find_indexed_path(file, group)?;
        let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
        let source_label = if source_group.header.kind() == crate::format::GroupKind::FunctionExpr {
            payload_function_expr_label(
                file,
                groups,
                anchors,
                source_group,
                fallback_expr_label,
                visiting,
            )
        } else {
            parameter_anchor_display_label(file, groups, source_group)
                .or_else(|| decode_label_name(file, source_group))
                .or_else(|| {
                    if source_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
                        let anchor_path = find_indexed_path(file, source_group)?;
                        let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
                        decode_label_name(file, point_group)
                    } else {
                        None
                    }
                })?
        };
        let (parameter_name, _) =
            resolve_function_expr_parameter(file, groups, group, anchors, &mut BTreeSet::new())?;
        Some(fallback_expr_label.replace(&parameter_name, &source_label))
    })()
    .unwrap_or_else(|| fallback_expr_label.to_string());
    visiting.remove(&group.ordinal);
    trim_decimal_literal_zeros(&result)
}

fn trim_decimal_literal_zeros(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if !ch.is_ascii_digit() {
            output.push(ch);
            continue;
        }
        let mut end = start + ch.len_utf8();
        let mut number = String::from(ch);
        while let Some((next_index, next_ch)) = chars.peek().copied() {
            if next_ch.is_ascii_digit() || next_ch == '.' {
                chars.next();
                end = next_index + next_ch.len_utf8();
                number.push(next_ch);
            } else {
                break;
            }
        }
        if number.contains('.') {
            while number.ends_with('0') {
                number.pop();
            }
            if number.ends_with('.') {
                number.pop();
            }
        }
        output.push_str(&number);
        if end >= text.len() {
            break;
        }
    }
    output
}

fn expanded_fallback_expr_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    fallback_expr_label: &str,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let mut text = fallback_expr_label.to_string();
    let mut changed = false;
    for ordinal in path.refs.iter().copied() {
        let source_group = groups.get(ordinal.checked_sub(1)?)?;
        let source_name = if source_group.header.kind() == crate::format::GroupKind::ParameterAnchor
        {
            let anchor_path = find_indexed_path(file, source_group)?;
            let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
            decode_label_name(file, source_group).or_else(|| decode_label_name(file, point_group))
        } else {
            decode_label_name(file, source_group)
        };
        let Some(source_name) = source_name else {
            continue;
        };
        let Some(display_name) =
            display_group_reference_label(file, groups, anchors, source_group, visiting)
        else {
            continue;
        };
        if display_name == source_name {
            continue;
        }
        let next = replace_expr_identifier(&text, &source_name, &display_name);
        changed |= next != text;
        text = next;
    }
    changed.then_some(text)
}

fn replace_expr_identifier(text: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(relative_index) = text[cursor..].find(needle) {
        let start = cursor + relative_index;
        let end = start + needle.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        output.push_str(&text[cursor..start]);
        if expr_identifier_boundary(before) && expr_identifier_boundary(after) {
            output.push_str(replacement);
        } else {
            output.push_str(needle);
        }
        cursor = end;
    }
    output.push_str(&text[cursor..]);
    output
}

fn expr_identifier_boundary(value: Option<char>) -> bool {
    match value {
        Some(ch) => !(ch.is_alphanumeric() || matches!(ch, '_' | '.' | '[' | ']' | '₀'..='₉')),
        None => true,
    }
}

#[derive(Clone)]
struct DisplayExprLabel {
    text: String,
    precedence: u8,
}

impl DisplayExprLabel {
    fn atom(text: String) -> Self {
        Self {
            text,
            precedence: 4,
        }
    }

    fn parenthesized_for(&self, parent_precedence: u8) -> String {
        if self.precedence < parent_precedence {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }
}

fn payload_function_expr_display_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    if group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let start = function_expr_word_start(&words)?;
    payload_function_application_label(file, groups, anchors, group, &words[start..], visiting)
        .or_else(|| {
            postfix_display_label_from_words(
                file,
                groups,
                anchors,
                group,
                &words[start..],
                visiting,
            )
            .map(|label| label.text)
        })
        .or_else(|| {
            linear_display_label_from_words(file, groups, anchors, group, &words[start..], visiting)
                .map(|label| label.text)
        })
}

fn payload_function_expr_application_display_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    if group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let start = function_expr_word_start(&words)?;
    payload_function_application_label(file, groups, anchors, group, &words[start..], visiting)
}

fn function_expr_word_start(words: &[u16]) -> Option<usize> {
    if let Some(start) = words
        .windows(7)
        .enumerate()
        .find_map(|(index, window)| {
            (window[0] == RECORD_FUNCTION_EXPR_PAYLOAD as u16
                && window[1] == 0
                && window[4] == crate::format::GroupKind::FunctionExpr.raw()
                && window[5] == 0)
                .then_some(index + 6)
        })
        .and_then(|count_index| {
            let word_count = usize::from(*words.get(count_index)?);
            let start = words.len().checked_sub(word_count)?;
            (word_count > 0 && start > count_index && start < words.len()).then_some(start)
        })
    {
        return Some(start);
    }
    words
        .windows(2)
        .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B)
        .map(|marker_index| marker_index + 2)
        .or_else(|| (!words.is_empty()).then_some(0))
}

const EXPR_FUNCTION_REF_MASK: u16 = 0xfff0;
const EXPR_FUNCTION_REF_PREFIX: u16 = 0x7000;

fn payload_function_application_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    words: &[u16],
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    let (application_offset, application_word) = words
        .iter()
        .copied()
        .enumerate()
        .find(|(_, word)| (*word & EXPR_FUNCTION_REF_MASK) == EXPR_FUNCTION_REF_PREFIX)?;
    let helper_index = usize::from(application_word & 0x000f);
    let path = find_indexed_path(file, group)?;
    let helper_group = groups.get(path.refs.get(helper_index)?.checked_sub(1)?)?;
    let helper_name = decode_label_name(file, helper_group).unwrap_or_else(|| "f".to_string());
    let arg_text = path
        .refs
        .iter()
        .enumerate()
        .find_map(|(index, ordinal)| {
            (index != helper_index).then(|| {
                let arg_group = groups.get(ordinal.checked_sub(1)?)?;
                display_group_reference_label(file, groups, anchors, arg_group, visiting)
            })?
        })
        .or_else(|| {
            postfix_display_label_from_words(
                file,
                groups,
                anchors,
                group,
                words.get(application_offset + 1..)?,
                visiting,
            )
            .map(|arg| arg.text)
        })?;
    Some(format!("{helper_name}({arg_text})"))
}

fn postfix_display_label_from_words(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    words: &[u16],
    visiting: &mut BTreeSet<usize>,
) -> Option<DisplayExprLabel> {
    let mut stack = Vec::<DisplayExprLabel>::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        if matches!(word, 0x000b | 0x000c) {
            index += 1;
            continue;
        }
        if word == 0
            && matches!(
                words.get(index + 1).copied(),
                Some(EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW)
            )
        {
            index += 1;
            continue;
        }
        match word {
            EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                let (symbol, precedence) = match word {
                    EXPR_OP_ADD => (" + ", 1),
                    EXPR_OP_SUB => (" - ", 1),
                    EXPR_OP_MUL => ("*", 2),
                    EXPR_OP_DIV => (" / ", 2),
                    EXPR_OP_POW => ("^", 3),
                    _ => unreachable!(),
                };
                stack.push(DisplayExprLabel {
                    text: format!(
                        "{}{}{}",
                        lhs.parenthesized_for(precedence),
                        symbol,
                        rhs.parenthesized_for(precedence + usize::from(word == EXPR_OP_SUB) as u8)
                    ),
                    precedence,
                });
                index += 1;
            }
            EXPR_VARIABLE_WORD => {
                stack.push(DisplayExprLabel::atom("x".to_string()));
                index += 1;
            }
            EXPR_PI_WORD => {
                stack.push(DisplayExprLabel::atom("π".to_string()));
                index += 1;
            }
            _ if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX => {
                let parameter_index = usize::from(word & 0x000f);
                stack.push(display_parameter_expr_label(
                    file,
                    groups,
                    anchors,
                    group,
                    parameter_index,
                    visiting,
                )?);
                index += 1;
            }
            _ if display_unary_function(word).is_some() => {
                let arg = stack.pop()?;
                let op = display_unary_function(word)?;
                stack.push(DisplayExprLabel::atom(format!(
                    "{op}({})",
                    arg.parenthesized_for(0)
                )));
                index += 1;
            }
            _ => {
                stack.push(DisplayExprLabel::atom(format_number(f64::from(word))));
                index += 1;
            }
        }
    }
    (stack.len() == 1).then(|| stack.pop().unwrap())
}

fn linear_display_label_from_words(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    words: &[u16],
    visiting: &mut BTreeSet<usize>,
) -> Option<DisplayExprLabel> {
    let mut index = 0;
    let mut lhs =
        display_operand_label(file, groups, anchors, group, *words.get(index)?, visiting)?;
    index += 1;
    while index < words.len() {
        let op_word = words[index];
        let (symbol, precedence) = match op_word {
            EXPR_OP_ADD => (" + ", 1),
            EXPR_OP_SUB => (" - ", 1),
            EXPR_OP_MUL => ("*", 2),
            EXPR_OP_DIV => (" / ", 2),
            EXPR_OP_POW => ("^", 3),
            _ => return None,
        };
        let rhs = display_operand_label(
            file,
            groups,
            anchors,
            group,
            *words.get(index + 1)?,
            visiting,
        )?;
        lhs = DisplayExprLabel {
            text: format!(
                "{}{}{}",
                lhs.parenthesized_for(precedence),
                symbol,
                rhs.parenthesized_for(precedence)
            ),
            precedence,
        };
        index += 2;
    }
    Some(lhs)
}

fn display_operand_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    word: u16,
    visiting: &mut BTreeSet<usize>,
) -> Option<DisplayExprLabel> {
    if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX {
        return display_parameter_expr_label(
            file,
            groups,
            anchors,
            group,
            usize::from(word & 0x000f),
            visiting,
        );
    }
    match word {
        EXPR_VARIABLE_WORD => Some(DisplayExprLabel::atom("x".to_string())),
        EXPR_PI_WORD => Some(DisplayExprLabel::atom("π".to_string())),
        _ => Some(DisplayExprLabel::atom(format_number(f64::from(word)))),
    }
}

fn display_unary_function(word: u16) -> Option<&'static str> {
    match word {
        0x2000 => Some("sin"),
        0x2001 => Some("cos"),
        0x2002 => Some("tan"),
        0x2006 => Some("abs"),
        0x2007 => Some("sqrt"),
        0x2008 => Some("ln"),
        0x2009 => Some("log"),
        0x200a => Some("sgn"),
        0x200b => Some("round"),
        0x200c => Some("trunc"),
        _ => None,
    }
}

fn display_parameter_expr_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    parameter_index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<DisplayExprLabel> {
    let text = display_parameter_label(file, groups, anchors, group, parameter_index, visiting)?;
    Some(DisplayExprLabel {
        precedence: display_label_precedence(&text),
        text,
    })
}

fn display_label_precedence(text: &str) -> u8 {
    if text.contains(" + ") || text.contains(" - ") {
        1
    } else if text.contains('*') || text.contains(" / ") {
        2
    } else if text.contains('^') {
        3
    } else {
        4
    }
}

fn display_parameter_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    parameter_index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let parameter_group = groups.get(path.refs.get(parameter_index)?.checked_sub(1)?)?;
    if parameter_group.header.kind() == crate::format::GroupKind::FunctionExpr
        && !visiting.contains(&parameter_group.ordinal)
    {
        if let Some(label) = payload_function_expr_application_display_label(
            file,
            groups,
            anchors,
            parameter_group,
            visiting,
        ) {
            return Some(label);
        }
        if let Some(label) = decode_label_name(file, parameter_group) {
            return Some(label);
        }
        if let Some(label) =
            payload_function_expr_display_label(file, groups, anchors, parameter_group, visiting)
        {
            return Some(label);
        }
    }
    display_group_reference_label(file, groups, anchors, parameter_group, visiting)
}

fn display_group_reference_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    _anchors: &[Option<PointRecord>],
    parameter_group: &ObjectGroup,
    _visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    parameter_anchor_display_label(file, groups, parameter_group)
        .or_else(|| decode_label_name(file, parameter_group))
        .or_else(|| {
            if parameter_group.header.kind() == crate::format::GroupKind::DistanceValue {
                let path = find_indexed_path(file, parameter_group)?;
                return Some(distance_value_label_name(file, groups, &path));
            }
            None
        })
        .or_else(|| {
            numeric_helper_function_parameter(file, groups, parameter_group).map(|(name, _)| name)
        })
        .or_else(|| {
            if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
                let anchor_path = find_indexed_path(file, parameter_group)?;
                let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
                decode_label_name(file, point_group)
            } else {
                None
            }
        })
        .or_else(|| {
            let expr = try_decode_function_expr(file, groups, parameter_group).ok()?;
            Some(function_expr_label(expr))
        })
}

fn parameter_anchor_display_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    if group.header.kind() != crate::format::GroupKind::ParameterAnchor {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
    if !point_group.header.kind().is_point_constraint() {
        return None;
    }
    let point_name =
        decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
    match host_group.header.kind() {
        crate::format::GroupKind::Polygon => {
            let polygon_name = polygon_vertex_name(file, groups, host_group)?;
            Some(format!("{point_name}在{polygon_name}上的值"))
        }
        kind if kind.is_line_like() => None,
        crate::format::GroupKind::PointTrace
        | crate::format::GroupKind::CoordinateTrace
        | crate::format::GroupKind::CustomTransformTrace => {
            let object_name = trace_object_name(file, groups, host_group)?;
            Some(format!("{point_name}在{object_name}上的值"))
        }
        kind if super::decode::is_circle_group_kind(kind) => {
            let circle_name = circle_name(file, groups, host_group)?;
            Some(format!("{point_name}在⊙{circle_name}上的值"))
        }
        _ => None,
    }
}

