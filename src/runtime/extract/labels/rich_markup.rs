fn build_expression_rich_markup(expr_label: &str, value_text: &str) -> Option<String> {
    if expr_label.matches(" / ").count() > 1 {
        return None;
    }
    if let Some((numerator, denominator)) = split_top_level(expr_label, " / ") {
        let numerator = strip_wrapping_parens(numerator);
        return Some(format!(
            "<H</<H{}><H{}>><Tx = {}>>",
            render_expression_rich_part(numerator),
            render_expression_rich_part(denominator),
            value_text,
        ));
    }
    Some(format!(
        "<H{}<Tx = {}>>",
        render_expression_rich_part(expr_label),
        value_text,
    ))
}

fn build_ratio_value_rich_markup(name: &str, value_text: &str) -> Option<String> {
    let expr_label = strip_wrapping_parens(name);
    let (numerator, denominator) = split_top_level(expr_label, "/")?;
    if split_top_level(denominator, "/").is_some() {
        return None;
    }
    Some(format!(
        "<H</<H{}><H{}>><Tx = {}>>",
        render_expression_rich_part(numerator),
        render_expression_rich_part(denominator),
        value_text,
    ))
}

fn render_expression_rich_part(text: &str) -> String {
    let mut output = String::new();
    let mut rest = text;
    while let Some(index) = rest.find("√(") {
        output.push_str(&rich_text_node(&rest[..index]));
        let open_index = index + "√".len();
        let Some(close_index) = matching_close_paren(rest, open_index) else {
            output.push_str(&rich_text_node(&rest[index..]));
            return output;
        };
        let inner = strip_wrapping_parens(&rest[open_index + 1..close_index]);
        output.push_str("<R");
        output.push_str(&render_expression_rich_part(inner));
        output.push('>');
        rest = &rest[close_index + 1..];
    }
    output.push_str(&rich_text_node(rest));
    output
}

fn rich_text_node(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!(
        "<Tx{}>",
        text.replace('&', "＆")
            .replace('<', "＜")
            .replace('>', "＞")
            .replace('*', "\u{00b7}")
    )
}

fn matching_close_paren(text: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in text[open_index..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_index + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_wrapping_parens(text: &str) -> &str {
    let trimmed = text.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return trimmed;
    }
    if matching_close_paren(trimmed, 0) == Some(trimmed.len() - 1) {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn build_plain_text_rich_markup(text: &str) -> Option<String> {
    let escaped = text
        .replace('&', "＆")
        .replace('<', "＜")
        .replace('>', "＞")
        .replace('*', "\u{00b7}");
    (!escaped.is_empty()).then(|| format!("<H<Tx{}>>", escaped))
}

fn apply_fallback_rich_markup(label: &mut TextLabel) {
    if label.rich_markup.is_none() {
        label.rich_markup = build_plain_text_rich_markup(&label.text);
    }
}

fn split_top_level<'a>(text: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0usize;
    let bytes = text.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut index = 0usize;
    while index + needle_bytes.len() <= bytes.len() {
        match bytes[index] as char {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && &bytes[index..index + needle_bytes.len()] == needle_bytes {
            let left = text[..index].trim();
            let right = text[index + needle.len()..].trim();
            return Some((left, right));
        }
        index += 1;
    }
    None
}

