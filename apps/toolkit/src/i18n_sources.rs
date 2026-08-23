#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub(crate) struct I18nSource {
    pub(crate) code: String,
    pub(crate) source: String,
}

#[cfg(target_arch = "wasm32")]
mod manifest {
    include!(concat!(env!("OUT_DIR"), "/i18n_manifest.rs"));
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read_i18n_sources() -> Vec<I18nSource> {
    let Some(directory) = i18n_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut sources = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ftl"))
            {
                return None;
            }
            Some(I18nSource {
                code: path.file_stem()?.to_str()?.to_string(),
                source: std::fs::read_to_string(path).ok()?,
            })
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.code.cmp(&right.code));
    sources
}

#[cfg(not(target_arch = "wasm32"))]
fn i18n_dir() -> Option<std::path::PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = vec![manifest_dir.join("assets").join("i18n")];
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("assets").join("i18n"));
        candidates.push(
            current_dir
                .join("apps")
                .join("toolkit")
                .join("assets")
                .join("i18n"),
        );
    }
    if let Some(executable_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
    {
        candidates.push(executable_dir.join("assets").join("i18n"));
        candidates.push(
            executable_dir
                .join("..")
                .join("Resources")
                .join("assets")
                .join("i18n"),
        );
    }
    candidates.into_iter().find(|path| path.is_dir())
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_i18n_sources_async() -> usize {
    let mut file_names = manifest::I18N_FILE_NAMES
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    if let Ok(listing) = fetch_text("assets/i18n/").await {
        file_names.extend(parse_i18n_listing(&listing));
    }
    file_names.sort();
    file_names.dedup();

    let mut installed = 0;
    for file_name in file_names {
        let path = format!("assets/i18n/{file_name}");
        let Ok(source) = fetch_text(&path).await else {
            continue;
        };
        let code = file_name.strip_suffix(".ftl").unwrap_or(&file_name);
        if crate::i18n::install_locale(code, &source).is_ok() {
            installed += 1;
        }
    }
    installed
}

#[cfg(target_arch = "wasm32")]
fn parse_i18n_listing(listing: &str) -> Vec<String> {
    listing
        .split(|character: char| {
            character.is_ascii_whitespace() || matches!(character, '"' | '\'' | '<' | '>' | '=')
        })
        .filter_map(|token| {
            let token = token
                .split(['?', '#'])
                .next()
                .unwrap_or_default()
                .trim()
                .trim_start_matches("./");
            let file_name = token.rsplit(['/', '\\']).next().unwrap_or(token);
            file_name.ends_with(".ftl").then(|| file_name.to_string())
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
async fn fetch_text(path: &str) -> Result<String, String> {
    use js_sys::futures::JsFuture;
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let response = JsFuture::from(window.fetch_with_str(path))
        .await
        .map_err(|error| format!("{error:?}"))?
        .dyn_into::<web_sys::Response>()
        .map_err(|error| format!("{error:?}"))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    JsFuture::from(response.text().map_err(|error| format!("{error:?}"))?)
        .await
        .map_err(|error| format!("{error:?}"))?
        .as_string()
        .ok_or_else(|| "response body is not text".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read_locale_preference() -> Option<String> {
    std::fs::read_to_string(locale_preference_path()?)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn read_locale_preference() -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item("ed1ths-pvz-toolkit-locale").ok().flatten())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn save_locale_preference(code: &str) -> Result<(), String> {
    let path = locale_preference_path()
        .ok_or_else(|| "could not resolve locale preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, code).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn save_locale_preference(code: &str) -> Result<(), String> {
    let storage = web_sys::window()
        .ok_or_else(|| "window is unavailable".to_string())?
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| "localStorage is unavailable".to_string())?;
    storage
        .set_item("ed1ths-pvz-toolkit-locale", code)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok())
        .and_then(|value| value.split(':').next().map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "C" && value != "POSIX")
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn system_locale() -> Option<String> {
    web_sys::window()
        .and_then(|window| window.navigator().language())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn locale_preference_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("io", "LambdaEd1th", "pvz-toolkit")
        .map(|directories| directories.config_dir().join("locale"))
}
