#[cfg(not(target_arch = "wasm32"))]
use crate::app_constants::{
    DESKTOP_WINDOW_DEFAULT_HEIGHT, DESKTOP_WINDOW_DEFAULT_WIDTH, DESKTOP_WINDOW_MIN_HEIGHT,
    DESKTOP_WINDOW_MIN_WIDTH,
};
use crate::domain::EditorMode;

#[cfg(target_arch = "wasm32")]
const LOCALE_PREFERENCE_KEY: &str = "rton-editor-locale";
#[cfg(target_arch = "wasm32")]
const TOOLBAR_LAYOUT_KEY: &str = "rton-editor-toolbar-layout";
#[cfg(target_arch = "wasm32")]
const LINE_WRAPPING_KEY: &str = "rton-editor-line-wrapping";
#[cfg(target_arch = "wasm32")]
const EDITOR_MODE_KEY: &str = "rton-editor-editor-mode";

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn app_config_dir() -> Option<std::path::PathBuf> {
    Some(config_dir()?.join("rton-editor"))
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSizePreference {
    pub width: u32,
    pub height: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for WindowSizePreference {
    fn default() -> Self {
        Self {
            width: DESKTOP_WINDOW_DEFAULT_WIDTH,
            height: DESKTOP_WINDOW_DEFAULT_HEIGHT,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_locale_preference() -> Option<String> {
    let path = locale_preference_path()?;
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_arch = "wasm32")]
pub fn read_locale_preference() -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(LOCALE_PREFERENCE_KEY).ok().flatten())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_locale_preference(locale_code: &str) -> Result<(), String> {
    let path = locale_preference_path()
        .ok_or_else(|| "could not resolve locale preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, locale_code).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn save_locale_preference(locale_code: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let storage = window
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| "localStorage is unavailable".to_string())?;
    storage
        .set_item(LOCALE_PREFERENCE_KEY, locale_code)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_line_wrapping_preference() -> bool {
    let Some(path) = line_wrapping_preference_path() else {
        return false;
    };
    std::fs::read_to_string(path)
        .ok()
        .is_some_and(|value| bool_preference_from_str(&value))
}

#[cfg(target_arch = "wasm32")]
pub fn read_line_wrapping_preference() -> bool {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(LINE_WRAPPING_KEY).ok().flatten())
        .is_some_and(|value| bool_preference_from_str(&value))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_line_wrapping_preference(enabled: bool) -> Result<(), String> {
    let path = line_wrapping_preference_path()
        .ok_or_else(|| "could not resolve line wrapping preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, bool_preference_value(enabled)).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_editor_mode_preference() -> Option<EditorMode> {
    let path = editor_mode_preference_path()?;
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| EditorMode::from_code(&value))
}

#[cfg(target_arch = "wasm32")]
pub fn read_editor_mode_preference() -> Option<EditorMode> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(EDITOR_MODE_KEY).ok().flatten())
        .and_then(|value| EditorMode::from_code(&value))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_editor_mode_preference(mode: EditorMode) -> Result<(), String> {
    let path = editor_mode_preference_path()
        .ok_or_else(|| "could not resolve editor mode preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, mode.code()).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn save_editor_mode_preference(mode: EditorMode) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let storage = window
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| "localStorage is unavailable".to_string())?;
    storage
        .set_item(EDITOR_MODE_KEY, mode.code())
        .map_err(|error| format!("{error:?}"))
}

#[cfg(target_arch = "wasm32")]
pub fn save_line_wrapping_preference(enabled: bool) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let storage = window
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| "localStorage is unavailable".to_string())?;
    storage
        .set_item(LINE_WRAPPING_KEY, bool_preference_value(enabled))
        .map_err(|error| format!("{error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn read_window_size_preference() -> WindowSizePreference {
    let Some(path) = window_size_preference_path() else {
        return WindowSizePreference::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| parse_window_size_preference(&value))
        .map(clamp_window_size_preference)
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_window_size_preference(width: u32, height: u32) -> Result<(), String> {
    let size = clamp_window_size_preference(WindowSizePreference { width, height });
    let path = window_size_preference_path()
        .ok_or_else(|| "could not resolve window size preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, format!("{}x{}", size.width, size.height))
        .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_toolbar_layout_preference() -> Option<String> {
    let path = toolbar_layout_preference_path()?;
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_arch = "wasm32")]
pub fn read_toolbar_layout_preference() -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(TOOLBAR_LAYOUT_KEY).ok().flatten())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_toolbar_layout_preference(layout: &str) -> Result<(), String> {
    let path = toolbar_layout_preference_path()
        .ok_or_else(|| "could not resolve toolbar layout preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, layout).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn save_toolbar_layout_preference(layout: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let storage = window
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| "localStorage is unavailable".to_string())?;
    storage
        .set_item(TOOLBAR_LAYOUT_KEY, layout)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok())
        .and_then(|value| value.split(':').next().map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "C" && value != "POSIX")
}

#[cfg(target_arch = "wasm32")]
pub fn system_locale() -> Option<String> {
    web_sys::window()
        .and_then(|window| window.navigator().language())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn locale_preference_path() -> Option<std::path::PathBuf> {
    Some(app_config_dir()?.join("locale"))
}

#[cfg(not(target_arch = "wasm32"))]
fn toolbar_layout_preference_path() -> Option<std::path::PathBuf> {
    Some(app_config_dir()?.join("toolbar-layout"))
}

#[cfg(not(target_arch = "wasm32"))]
fn line_wrapping_preference_path() -> Option<std::path::PathBuf> {
    Some(app_config_dir()?.join("line-wrapping"))
}

#[cfg(not(target_arch = "wasm32"))]
fn editor_mode_preference_path() -> Option<std::path::PathBuf> {
    Some(app_config_dir()?.join("editor-mode"))
}

#[cfg(not(target_arch = "wasm32"))]
fn window_size_preference_path() -> Option<std::path::PathBuf> {
    Some(app_config_dir()?.join("window-size"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_window_size_preference(value: &str) -> Option<WindowSizePreference> {
    let (width, height) = value.trim().split_once(['x', 'X', ','])?;
    Some(WindowSizePreference {
        width: width.trim().parse().ok()?,
        height: height.trim().parse().ok()?,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn clamp_window_size_preference(size: WindowSizePreference) -> WindowSizePreference {
    WindowSizePreference {
        width: size.width.max(DESKTOP_WINDOW_MIN_WIDTH),
        height: size.height.max(DESKTOP_WINDOW_MIN_HEIGHT),
    }
}

fn bool_preference_from_str(value: &str) -> bool {
    let value = value.trim();
    value == "1"
        || value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("on")
}

fn bool_preference_value(enabled: bool) -> &'static str {
    if enabled { "true" } else { "false" }
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|home| home.join("Library").join("Application Support"))
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(std::path::PathBuf::from)
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    not(target_os = "windows")
))]
fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".config"))
        })
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use crate::app_constants::{DESKTOP_WINDOW_MIN_HEIGHT, DESKTOP_WINDOW_MIN_WIDTH};

    #[cfg(not(target_arch = "wasm32"))]
    use super::{WindowSizePreference, clamp_window_size_preference, parse_window_size_preference};
    use super::{bool_preference_from_str, bool_preference_value};

    #[test]
    fn parses_boolean_preferences() {
        assert!(bool_preference_from_str("true"));
        assert!(bool_preference_from_str("1"));
        assert!(bool_preference_from_str("ON"));
        assert!(bool_preference_from_str(" yes "));
        assert!(!bool_preference_from_str("false"));
        assert!(!bool_preference_from_str("0"));
        assert!(!bool_preference_from_str(""));
    }

    #[test]
    fn serializes_boolean_preferences() {
        assert_eq!(bool_preference_value(true), "true");
        assert_eq!(bool_preference_value(false), "false");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn parses_window_size_preferences() {
        assert_eq!(
            parse_window_size_preference("1440x900"),
            Some(WindowSizePreference {
                width: 1440,
                height: 900
            })
        );
        assert_eq!(
            parse_window_size_preference("1280, 760"),
            Some(WindowSizePreference {
                width: 1280,
                height: 760
            })
        );
        assert_eq!(parse_window_size_preference("bad"), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn clamps_window_size_preferences() {
        assert_eq!(
            clamp_window_size_preference(WindowSizePreference {
                width: 100,
                height: 100
            }),
            WindowSizePreference {
                width: DESKTOP_WINDOW_MIN_WIDTH,
                height: DESKTOP_WINDOW_MIN_HEIGHT,
            }
        );
    }
}
