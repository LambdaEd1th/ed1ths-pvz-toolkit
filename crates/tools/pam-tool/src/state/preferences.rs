use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    ZhCn,
    En,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub locale: Locale,
    pub theme: Theme,
    pub loop_playback: bool,
    pub reverse: bool,
    pub autoplay: bool,
    pub keep_speed: bool,
    pub boundary: bool,
    pub speed_fps: Option<u32>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            locale: Locale::ZhCn,
            theme: Theme::System,
            loop_playback: true,
            reverse: false,
            autoplay: false,
            keep_speed: false,
            boundary: true,
            speed_fps: None,
        }
    }
}
