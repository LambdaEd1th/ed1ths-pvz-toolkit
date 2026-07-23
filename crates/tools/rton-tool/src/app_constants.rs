pub(crate) const LEFT_PANEL_DEFAULT_WIDTH: i32 = 300;
pub(crate) const RIGHT_PANEL_DEFAULT_WIDTH: i32 = 380;
pub(crate) const PANEL_MIN_WIDTH: i32 = 220;
pub(crate) const PANEL_MAX_WIDTH: i32 = 560;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_DEFAULT_WIDTH: u32 = 1440;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_DEFAULT_HEIGHT: u32 = 900;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_MIN_WIDTH: u32 = 1180;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_MIN_HEIGHT: u32 = 720;
pub(crate) const TAB_DROP_MIDPOINT_PX: f64 = 90.0;
pub(crate) const TOOLBAR_GROUP_DROP_MIDPOINT_PX: f64 = 72.0;
#[cfg(test)]
pub(crate) const TEXT_SEARCH_MATCH_DISPLAY_LIMIT: usize = 5000;
pub(crate) const LOADABLE_FILE_ACCEPT: &str = "";
pub(crate) const LOADABLE_FILE_HINT: &str = ".rton / .json / .yaml / .yml / .toml / RTON bytes";
