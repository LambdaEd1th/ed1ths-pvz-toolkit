use crate::i18n::I18n;
#[cfg(test)]
use rton_editor_core::TextSearchMatch;

pub(crate) fn previous_text_search_index(current: usize, count: usize) -> usize {
    if count == 0 || current == 0 || current >= count {
        count.saturating_sub(1)
    } else {
        current - 1
    }
}

pub(crate) fn next_text_search_index(current: usize, count: usize) -> usize {
    if count == 0 { 0 } else { (current + 1) % count }
}

pub(crate) fn text_search_status_text(
    query: &str,
    match_count: usize,
    current_match_index: Option<usize>,
    capped: bool,
    i18n: I18n,
) -> String {
    if query.is_empty() {
        return i18n.t("editor-type-search");
    }
    if match_count == 0 {
        return i18n.t("editor-no-matches");
    }

    let total = if capped {
        format!("{match_count}+")
    } else {
        match_count.to_string()
    };
    if let Some(index) = current_match_index {
        i18n.t_args(
            "editor-current-match",
            &[
                ("current", (index + 1).to_string()),
                ("total", total.clone()),
            ],
        )
    } else {
        i18n.t_args("editor-matches", &[("count", total)])
    }
}

pub(crate) fn replace_text_span(text: &str, start: usize, end: usize, replacement: &str) -> String {
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return text.to_string();
    }

    let mut next = String::with_capacity(text.len() - (end - start) + replacement.len());
    next.push_str(&text[..start]);
    next.push_str(replacement);
    next.push_str(&text[end..]);
    next
}

#[cfg(test)]
pub(crate) fn replace_all_text_matches(
    text: &str,
    matches: &[TextSearchMatch],
    replacement: &str,
) -> String {
    if matches.is_empty() {
        return text.to_string();
    }

    let mut next = String::with_capacity(text.len());
    let mut copied_until = 0;
    for match_ in matches {
        if match_.start < copied_until
            || match_.end > text.len()
            || !text.is_char_boundary(match_.start)
            || !text.is_char_boundary(match_.end)
        {
            return text.to_string();
        }
        next.push_str(&text[copied_until..match_.start]);
        next.push_str(replacement);
        copied_until = match_.end;
    }
    next.push_str(&text[copied_until..]);
    next
}

pub(crate) fn replace_all_text_query(
    text: &str,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
) -> String {
    if query.is_empty() || text.len() < query.len() {
        return text.to_string();
    }

    if case_sensitive {
        replace_all_text_pattern(text, text, query, replacement)
    } else {
        let haystack = text.to_ascii_lowercase();
        let needle = query.to_ascii_lowercase();
        replace_all_text_pattern(text, &haystack, &needle, replacement)
    }
}

fn replace_all_text_pattern(
    original: &str,
    haystack: &str,
    needle: &str,
    replacement: &str,
) -> String {
    let mut next = String::with_capacity(original.len());
    let mut offset = 0;
    let mut copied_until = 0;

    while offset <= haystack.len() {
        let Some(relative_start) = haystack[offset..].find(needle) else {
            break;
        };
        let start = offset + relative_start;
        let end = start + needle.len();
        if !original.is_char_boundary(start) || !original.is_char_boundary(end) {
            return original.to_string();
        }
        next.push_str(&original[copied_until..start]);
        next.push_str(replacement);
        copied_until = end;
        offset = end;
    }

    if copied_until == 0 {
        return original.to_string();
    }
    next.push_str(&original[copied_until..]);
    next
}
