use rton_editor_core::TextFormat;

pub(crate) fn file_list_file_key(id: usize) -> String {
    format!("file:{id}")
}

pub(crate) fn file_list_tab_key(id: usize) -> String {
    format!("tab:{id}")
}

pub(crate) fn parse_file_list_file_key(key: &str) -> Option<usize> {
    key.strip_prefix("file:")?.parse().ok()
}

pub(crate) fn parse_file_list_tab_key(key: &str) -> Option<usize> {
    key.strip_prefix("tab:")?.parse().ok()
}

pub(crate) fn loadable_file_kind_label(name: &str) -> String {
    match name
        .rsplit('.')
        .next()
        .filter(|extension| *extension != name)
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => "JSON",
        Some("yaml" | "yml") => "YAML",
        Some("toml") => "TOML",
        Some("rton") => "RTON",
        _ => "RTON",
    }
    .to_string()
}

pub(crate) fn is_loadable_display_name(name: &str) -> bool {
    !leaf_display_name(name).trim().is_empty()
}

pub(crate) fn export_rton_name(source_name: &str) -> String {
    let stem = file_stem(source_name);
    if stem.is_empty() {
        "export.rton".to_string()
    } else {
        format!("{stem}.rton")
    }
}

pub(crate) fn export_text_name(source_name: &str, format: TextFormat) -> String {
    format!("{}.{}", file_stem(source_name), format.extension())
}

pub(crate) fn file_stem(source_name: &str) -> String {
    let file = source_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source_name);
    file.rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file)
        .to_string()
}

pub(crate) fn leaf_display_name(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .rfind(|part| !part.is_empty())
        .unwrap_or(path)
        .to_string()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn normalize_display_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches('/').to_string()
}
