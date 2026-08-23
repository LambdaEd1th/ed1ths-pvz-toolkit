const DEFAULT_WINDOW_WIDTH: u32 = 1440;
const DEFAULT_WINDOW_HEIGHT: u32 = 900;
const MIN_WINDOW_WIDTH: u32 = 720;
const MIN_WINDOW_HEIGHT: u32 = 560;
const MAX_WINDOW_WIDTH: u32 = 7680;
const MAX_WINDOW_HEIGHT: u32 = 4320;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WindowSize {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: DEFAULT_WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
        }
    }
}

pub(crate) fn read_window_size() -> WindowSize {
    let Some(path) = window_size_path() else {
        return WindowSize::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| parse_window_size(&value))
        .map(clamp_window_size)
        .unwrap_or_default()
}

pub(crate) fn save_window_size(size: WindowSize) -> Result<(), String> {
    let size = clamp_window_size(size);
    let path = window_size_path()
        .ok_or_else(|| "could not resolve window size preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, format!("{}x{}", size.width, size.height))
        .map_err(|error| error.to_string())
}

pub(crate) fn window_size_from_viewport(width: f64, height: f64) -> Option<WindowSize> {
    if !width.is_finite() || !height.is_finite() {
        return None;
    }

    let width = width.round().max(0.0) as u32;
    let height = height.round().max(0.0) as u32;
    if width < MIN_WINDOW_WIDTH || height < MIN_WINDOW_HEIGHT {
        return None;
    }
    Some(clamp_window_size(WindowSize { width, height }))
}

fn window_size_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("io", "LambdaEd1th", "pvz-toolkit")
        .map(|directories| directories.config_dir().join("window-size"))
}

fn parse_window_size(value: &str) -> Option<WindowSize> {
    let (width, height) = value.trim().split_once(['x', 'X', ','])?;
    Some(WindowSize {
        width: width.trim().parse().ok()?,
        height: height.trim().parse().ok()?,
    })
}

fn clamp_window_size(size: WindowSize) -> WindowSize {
    WindowSize {
        width: size.width.clamp(MIN_WINDOW_WIDTH, MAX_WINDOW_WIDTH),
        height: size.height.clamp(MIN_WINDOW_HEIGHT, MAX_WINDOW_HEIGHT),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_WINDOW_HEIGHT, MAX_WINDOW_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, WindowSize,
        clamp_window_size, parse_window_size, window_size_from_viewport,
    };

    #[test]
    fn parses_window_size() {
        assert_eq!(
            parse_window_size("1440x900"),
            Some(WindowSize {
                width: 1440,
                height: 900,
            })
        );
        assert_eq!(
            parse_window_size("1280, 760"),
            Some(WindowSize {
                width: 1280,
                height: 760,
            })
        );
        assert_eq!(parse_window_size("invalid"), None);
    }

    #[test]
    fn clamps_saved_window_size() {
        assert_eq!(
            clamp_window_size(WindowSize {
                width: 100,
                height: u32::MAX,
            }),
            WindowSize {
                width: MIN_WINDOW_WIDTH,
                height: MAX_WINDOW_HEIGHT,
            }
        );
        assert_eq!(
            clamp_window_size(WindowSize {
                width: u32::MAX,
                height: 100,
            }),
            WindowSize {
                width: MAX_WINDOW_WIDTH,
                height: MIN_WINDOW_HEIGHT,
            }
        );
    }

    #[test]
    fn ignores_transient_invalid_viewports() {
        assert_eq!(window_size_from_viewport(f64::NAN, 900.0), None);
        assert_eq!(window_size_from_viewport(700.0, 900.0), None);
        assert_eq!(window_size_from_viewport(1440.0, 500.0), None);
    }
}
