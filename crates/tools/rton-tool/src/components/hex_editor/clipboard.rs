use super::state::HexPane;

pub(super) fn format_hex_clipboard_text(bytes: &[u8], pane: HexPane) -> String {
    match pane {
        HexPane::Hex => bytes
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
        HexPane::Ascii => bytes.iter().map(|byte| *byte as char).collect(),
    }
}

pub(super) fn parse_hex_clipboard_text(text: &str, pane: HexPane) -> Option<Vec<u8>> {
    match pane {
        HexPane::Hex => parse_hex_bytes_text(text),
        HexPane::Ascii => text
            .chars()
            .map(|ch| {
                let code = ch as u32;
                (code <= 0xff).then_some(code as u8)
            })
            .collect(),
    }
}

fn parse_hex_bytes_text(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for token in text.split(|ch: char| ch.is_ascii_whitespace() || ",;:-".contains(ch)) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let token = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        if token.is_empty()
            || token.len() % 2 != 0
            || !token.chars().all(|ch| ch.is_ascii_hexdigit())
        {
            return None;
        }
        for chunk_start in (0..token.len()).step_by(2) {
            bytes.push(u8::from_str_radix(&token[chunk_start..chunk_start + 2], 16).ok()?);
        }
    }
    (!bytes.is_empty()).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_byte_ascii_clipboard_text() {
        assert_eq!(parse_hex_clipboard_text("中", HexPane::Ascii), None);
    }
}
