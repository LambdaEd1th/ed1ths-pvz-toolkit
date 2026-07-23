use serde::{Deserialize, Serialize};

pub const DEFAULT_TOOLBAR_ORDER: &[&str] = &[
    "file",
    "selectors",
    "playback",
    "speed",
    "layers",
    "view",
    "size",
    "export",
    "convert",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    ZhCn,
    En,
}

impl Locale {
    pub fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
        }
    }
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
    pub images_panel_open: bool,
    pub sprites_panel_open: bool,
    pub image_panel_width: u32,
    pub sprite_panel_width: u32,
    pub toolbar_order: Vec<String>,
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
            images_panel_open: true,
            sprites_panel_open: true,
            image_panel_width: 250,
            sprite_panel_width: 270,
            toolbar_order: DEFAULT_TOOLBAR_ORDER
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
        }
    }
}

impl Preferences {
    pub fn normalized(mut self) -> Self {
        self.image_panel_width = self.image_panel_width.clamp(180, 500);
        self.sprite_panel_width = self.sprite_panel_width.clamp(180, 500);
        let mut order = Vec::new();
        for id in &self.toolbar_order {
            if DEFAULT_TOOLBAR_ORDER.contains(&id.as_str()) && !order.contains(id) {
                order.push(id.clone());
            }
        }
        for id in DEFAULT_TOOLBAR_ORDER {
            if !order.iter().any(|value| value == id) {
                order.push((*id).into());
            }
        }
        self.toolbar_order = order;
        self
    }
}
