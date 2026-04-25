pub(super) fn parse_first_quoted(input: &str) -> Option<String> {
    let (start, delimiter) = find_first_quote(input)?;
    let tail = &input[start + delimiter.len_utf8()..];
    let mut escaped = false;
    let mut out = String::new();
    for ch in tail.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == delimiter {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

pub(super) fn parse_leading_quoted(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    let delimiter = chars.next()?;
    if delimiter != '"' && delimiter != '\'' {
        return None;
    }
    let mut escaped = false;
    let mut out = String::new();
    let mut end_idx = None;
    for (idx, ch) in input.char_indices().skip(1) {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == delimiter {
            end_idx = Some(idx);
            break;
        }
        out.push(ch);
    }
    let end_idx = end_idx?;
    let rest = &input[end_idx + 1..];
    Some((out, rest))
}

pub(super) fn is_simple_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
}

pub(super) fn find_first_quote(input: &str) -> Option<(usize, char)> {
    input
        .char_indices()
        .find(|(_, ch)| *ch == '"' || *ch == '\'')
}
