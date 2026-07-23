use super::I18nSource;

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct WebI18nEntry {
    code: String,
    file_name: String,
}

mod i18n_manifest {
    include!(concat!(env!("OUT_DIR"), "/i18n_manifest.rs"));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_i18n_sources() -> Vec<I18nSource> {
    i18n_dir()
        .map(|directory| read_i18n_sources_from_dir(&directory))
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
pub async fn read_i18n_sources_async() -> Vec<I18nSource> {
    let mut entries = manifest_web_i18n_entries();
    if let Ok(listing) = fetch_text("assets/rton/i18n/").await {
        entries.extend(parse_web_i18n_directory_listing(&listing));
    }
    let entries = unique_web_i18n_entries(entries);

    let mut sources = Vec::new();
    for entry in entries {
        let path = format!("assets/rton/i18n/{}", entry.file_name);
        let Ok(source) = fetch_text(&path).await else {
            continue;
        };
        sources.push(I18nSource {
            code: entry.code,
            source,
        });
    }
    sources
}

#[cfg(not(target_arch = "wasm32"))]
fn i18n_dir() -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("assets").join("rton").join("i18n"));
        candidates.push(
            current_dir
                .join("app")
                .join("assets")
                .join("rton")
                .join("i18n"),
        );
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
    {
        candidates.push(exe_dir.join("assets").join("rton").join("i18n"));
    }
    candidates.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("rton")
            .join("i18n"),
    );
    candidates.into_iter().find(|path| path.is_dir())
}

#[cfg(not(target_arch = "wasm32"))]
fn read_i18n_sources_from_dir(directory: &std::path::Path) -> Vec<I18nSource> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut sources = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let is_ftl = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ftl"));
            if !is_ftl {
                return None;
            }

            let code = path.file_stem()?.to_str()?.to_string();
            let source = std::fs::read_to_string(path).ok()?;
            Some(I18nSource { code, source })
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.code.cmp(&right.code));
    sources
}

#[cfg(target_arch = "wasm32")]
async fn fetch_text(path: &str) -> Result<String, String> {
    use js_sys::futures::JsFuture;
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let response_value = JsFuture::from(window.fetch_with_str(path))
        .await
        .map_err(|error| format!("{error:?}"))?;
    let response = response_value
        .dyn_into::<web_sys::Response>()
        .map_err(|error| format!("{error:?}"))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }

    let text_promise = response.text().map_err(|error| format!("{error:?}"))?;
    let text_value = JsFuture::from(text_promise)
        .await
        .map_err(|error| format!("{error:?}"))?;
    text_value
        .as_string()
        .ok_or_else(|| "response body is not text".to_string())
}

#[cfg(any(target_arch = "wasm32", test))]
fn manifest_web_i18n_entries() -> Vec<WebI18nEntry> {
    i18n_manifest::I18N_FILE_NAMES
        .iter()
        .filter_map(|file_name| parse_web_i18n_listing_token(file_name))
        .collect()
}

#[cfg(any(target_arch = "wasm32", test))]
fn parse_web_i18n_directory_listing(listing: &str) -> Vec<WebI18nEntry> {
    let mut entries: Vec<WebI18nEntry> = Vec::new();
    for token in listing
        .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | '='))
    {
        let Some(entry) = parse_web_i18n_listing_token(token) else {
            continue;
        };
        if !entries.iter().any(|existing| existing.code == entry.code) {
            entries.push(entry);
        }
    }
    entries
}

#[cfg(any(target_arch = "wasm32", test))]
fn unique_web_i18n_entries(mut entries: Vec<WebI18nEntry>) -> Vec<WebI18nEntry> {
    entries.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.file_name.cmp(&right.file_name))
    });
    entries.dedup_by(|left, right| left.code == right.code);
    entries
}

#[cfg(any(target_arch = "wasm32", test))]
fn parse_web_i18n_listing_token(token: &str) -> Option<WebI18nEntry> {
    let path = token
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches("./");
    let file_name = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .rsplit('\\')
        .next()
        .unwrap_or(path);
    if file_name.is_empty() || file_name == "." || file_name == ".." || !file_name.ends_with(".ftl")
    {
        return None;
    }

    let stem = file_name.strip_suffix(".ftl")?;
    let code = canonical_web_i18n_locale_code(stem)?;

    Some(WebI18nEntry {
        code,
        file_name: file_name.to_string(),
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn canonical_web_i18n_locale_code(code: &str) -> Option<String> {
    let normalized = code.trim().replace('_', "-");
    let parts = normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    let mut canonical = Vec::with_capacity(parts.len());
    for (index, part) in parts.iter().enumerate() {
        if !(1..=8).contains(&part.len()) || !part.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return None;
        }

        if index == 0 {
            if part.len() < 2 || !part.chars().all(|ch| ch.is_ascii_alphabetic()) {
                return None;
            }
            canonical.push(part.to_ascii_lowercase());
        } else if part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            canonical.push(part.to_ascii_uppercase());
        } else if part.len() == 4 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let mut chars = part.chars();
            let first = chars.next()?.to_ascii_uppercase();
            let rest = chars.as_str().to_ascii_lowercase();
            canonical.push(format!("{first}{rest}"));
        } else {
            canonical.push(part.to_ascii_lowercase());
        }
    }

    Some(canonical.join("-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_web_i18n_entries_are_generated() {
        let entries = manifest_web_i18n_entries();
        let codes = entries
            .iter()
            .map(|entry| entry.code.as_str())
            .collect::<Vec<_>>();

        assert_eq!(codes, vec!["en-US", "es-ES", "fr-FR", "ru-RU", "zh-CN"]);
    }

    #[test]
    fn parses_web_i18n_directory_listing_links() {
        let entries = parse_web_i18n_directory_listing(
            r#"
            <a href="de-DE.ftl">de-DE.ftl</a>
            <a href="/assets/rton/i18n/pt_BR.ftl?cache=1">pt_BR.ftl</a>
            <a href="./ja-JP.ftl#file">ja-JP.ftl</a>
            <a href="notes.txt">notes.txt</a>
            "#,
        );

        assert_eq!(
            entries,
            vec![
                WebI18nEntry {
                    code: "de-DE".to_string(),
                    file_name: "de-DE.ftl".to_string(),
                },
                WebI18nEntry {
                    code: "pt-BR".to_string(),
                    file_name: "pt_BR.ftl".to_string(),
                },
                WebI18nEntry {
                    code: "ja-JP".to_string(),
                    file_name: "ja-JP.ftl".to_string(),
                },
            ]
        );
    }

    #[test]
    fn deduplicates_web_i18n_directory_listing_by_locale() {
        let entries = parse_web_i18n_directory_listing(
            r#"<a href="fr-FR.ftl">fr-FR.ftl</a><a href="fr_fr.ftl">fr_fr.ftl</a>"#,
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, "fr-FR");
        assert_eq!(entries[0].file_name, "fr-FR.ftl");
    }

    #[test]
    fn sorts_and_deduplicates_web_i18n_entries() {
        let entries = unique_web_i18n_entries(vec![
            WebI18nEntry {
                code: "zh-CN".to_string(),
                file_name: "zh-CN.ftl".to_string(),
            },
            WebI18nEntry {
                code: "en-US".to_string(),
                file_name: "en-US.ftl".to_string(),
            },
            WebI18nEntry {
                code: "en-US".to_string(),
                file_name: "en_us.ftl".to_string(),
            },
        ]);

        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.code.as_str(), entry.file_name.as_str()))
                .collect::<Vec<_>>(),
            vec![("en-US", "en-US.ftl"), ("zh-CN", "zh-CN.ftl")]
        );
    }
}
