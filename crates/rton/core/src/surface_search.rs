use serde::{Deserialize, Serialize};

use crate::ByteRead;

pub const SURFACE_SEARCH_MATCH_LIMIT: usize = 5_000;
#[cfg(not(target_arch = "wasm32"))]
const PARALLEL_SEARCH_MIN_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSearchMatch {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSearchResult {
    pub matches: Vec<TextSearchMatch>,
    pub capped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HexSearchMatch {
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HexSearchResult {
    pub matches: Vec<HexSearchMatch>,
    pub capped: bool,
}

pub fn find_text_search_result(text: &str, query: &str, case_sensitive: bool) -> TextSearchResult {
    if query.is_empty() || text.len() < query.len() {
        return TextSearchResult {
            matches: Vec::new(),
            capped: false,
        };
    }

    #[cfg(not(target_arch = "wasm32"))]
    if text.len() >= PARALLEL_SEARCH_MIN_BYTES && query.is_ascii() {
        return find_text_search_result_parallel(text, query.as_bytes(), case_sensitive);
    }

    find_text_search_result_sequential(text, query.as_bytes(), case_sensitive)
}

fn find_text_search_result_sequential(
    text: &str,
    query: &[u8],
    case_sensitive: bool,
) -> TextSearchResult {
    let haystack = text.as_bytes();
    let mut matches = Vec::new();
    let mut offset = 0;
    let mut capped = false;
    while offset + query.len() <= haystack.len() {
        if text_matches_at(text, haystack, query, offset, case_sensitive) {
            let end = offset + query.len();
            matches.push(TextSearchMatch { start: offset, end });
            if matches.len() >= SURFACE_SEARCH_MATCH_LIMIT {
                capped = has_later_text_match(text, haystack, query, end, case_sensitive);
                break;
            }
            offset = end;
        } else {
            offset += 1;
        }
    }
    TextSearchResult { matches, capped }
}

#[cfg(not(target_arch = "wasm32"))]
fn find_text_search_result_parallel(
    text: &str,
    query: &[u8],
    case_sensitive: bool,
) -> TextSearchResult {
    use rayon::prelude::*;

    let haystack = text.as_bytes();
    let max_start = haystack.len() - query.len();
    let chunk_count = rayon::current_num_threads().max(1);
    let chunk_size = (max_start + 1).div_ceil(chunk_count).max(1);
    let mut matches = (0..=max_start)
        .step_by(chunk_size)
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|start| {
            let end = (start + chunk_size).min(max_start + 1);
            let mut found = Vec::new();
            for offset in start..end {
                if text_matches_at(text, haystack, query, offset, case_sensitive) {
                    found.push(TextSearchMatch {
                        start: offset,
                        end: offset + query.len(),
                    });
                    if found.len() > SURFACE_SEARCH_MATCH_LIMIT {
                        break;
                    }
                }
            }
            found
        })
        .flatten()
        .collect::<Vec<_>>();
    matches.sort_unstable_by_key(|match_| match_.start);
    let mut filtered = Vec::with_capacity(matches.len().min(SURFACE_SEARCH_MATCH_LIMIT));
    let mut next_allowed = 0;
    let mut capped = false;
    for match_ in matches {
        if match_.start < next_allowed {
            continue;
        }
        if filtered.len() >= SURFACE_SEARCH_MATCH_LIMIT {
            capped = true;
            break;
        }
        next_allowed = match_.end;
        filtered.push(match_);
    }
    TextSearchResult {
        matches: filtered,
        capped,
    }
}

fn has_later_text_match(
    text: &str,
    haystack: &[u8],
    query: &[u8],
    start: usize,
    case_sensitive: bool,
) -> bool {
    (start..=haystack.len().saturating_sub(query.len()))
        .any(|offset| text_matches_at(text, haystack, query, offset, case_sensitive))
}

fn text_matches_at(
    text: &str,
    haystack: &[u8],
    query: &[u8],
    offset: usize,
    case_sensitive: bool,
) -> bool {
    let end = offset.saturating_add(query.len());
    if end > haystack.len() || !text.is_char_boundary(offset) || !text.is_char_boundary(end) {
        return false;
    }
    if case_sensitive {
        haystack[offset..end] == *query
    } else {
        haystack[offset..end]
            .iter()
            .zip(query)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    }
}

pub fn find_hex_search_result<B: ByteRead + Sync + ?Sized>(
    bytes: &B,
    pattern: &[u8],
    ascii_insensitive: bool,
) -> HexSearchResult {
    if pattern.is_empty() || bytes.len() < pattern.len() {
        return HexSearchResult {
            matches: Vec::new(),
            capped: false,
        };
    }

    #[cfg(not(target_arch = "wasm32"))]
    if bytes.len() >= PARALLEL_SEARCH_MIN_BYTES {
        return find_hex_search_result_parallel(bytes, pattern, ascii_insensitive);
    }

    find_hex_search_result_sequential(bytes, pattern, ascii_insensitive)
}

fn find_hex_search_result_sequential<B: ByteRead + ?Sized>(
    bytes: &B,
    pattern: &[u8],
    ascii_insensitive: bool,
) -> HexSearchResult {
    let mut matches = Vec::new();
    let mut offset = 0;
    let mut capped = false;
    while offset + pattern.len() <= bytes.len() {
        if hex_matches_at(bytes, offset, pattern, ascii_insensitive) {
            matches.push(HexSearchMatch {
                offset,
                length: pattern.len(),
            });
            if matches.len() >= SURFACE_SEARCH_MATCH_LIMIT {
                capped =
                    has_later_hex_match(bytes, pattern, offset + pattern.len(), ascii_insensitive);
                break;
            }
            offset += pattern.len();
        } else {
            offset += 1;
        }
    }
    HexSearchResult { matches, capped }
}

#[cfg(not(target_arch = "wasm32"))]
fn find_hex_search_result_parallel<B: ByteRead + Sync + ?Sized>(
    bytes: &B,
    pattern: &[u8],
    ascii_insensitive: bool,
) -> HexSearchResult {
    use rayon::prelude::*;

    let max_start = bytes.len() - pattern.len();
    let chunk_count = rayon::current_num_threads().max(1);
    let chunk_size = (max_start + 1).div_ceil(chunk_count).max(1);
    let mut matches = (0..=max_start)
        .step_by(chunk_size)
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|start| {
            let end = (start + chunk_size).min(max_start + 1);
            let mut found = Vec::new();
            for offset in start..end {
                if hex_matches_at(bytes, offset, pattern, ascii_insensitive) {
                    found.push(HexSearchMatch {
                        offset,
                        length: pattern.len(),
                    });
                    if found.len() > SURFACE_SEARCH_MATCH_LIMIT {
                        break;
                    }
                }
            }
            found
        })
        .flatten()
        .collect::<Vec<_>>();
    matches.sort_unstable_by_key(|match_| match_.offset);
    let mut filtered = Vec::with_capacity(matches.len().min(SURFACE_SEARCH_MATCH_LIMIT));
    let mut next_allowed = 0;
    let mut capped = false;
    for match_ in matches {
        if match_.offset < next_allowed {
            continue;
        }
        if filtered.len() >= SURFACE_SEARCH_MATCH_LIMIT {
            capped = true;
            break;
        }
        next_allowed = match_.offset + match_.length;
        filtered.push(match_);
    }
    HexSearchResult {
        matches: filtered,
        capped,
    }
}

fn has_later_hex_match<B: ByteRead + ?Sized>(
    bytes: &B,
    pattern: &[u8],
    start: usize,
    ascii_insensitive: bool,
) -> bool {
    (start..=bytes.len().saturating_sub(pattern.len()))
        .any(|offset| hex_matches_at(bytes, offset, pattern, ascii_insensitive))
}

fn hex_matches_at<B: ByteRead + ?Sized>(
    bytes: &B,
    offset: usize,
    pattern: &[u8],
    ascii_insensitive: bool,
) -> bool {
    pattern.iter().enumerate().all(|(index, expected)| {
        bytes.byte_at(offset + index).is_some_and(|actual| {
            if ascii_insensitive {
                actual.eq_ignore_ascii_case(expected)
            } else {
                actual == *expected
            }
        })
    })
}
