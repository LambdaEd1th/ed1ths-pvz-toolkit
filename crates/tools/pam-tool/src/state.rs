mod preferences;
mod workspace;

pub use preferences::{Locale, Preferences, Theme};
pub use workspace::ViewerTab;

use dioxus::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use pam_viewer_renderer::SharedStage;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    Ok,
    Warning,
    Error,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub message: String,
    pub tone: Tone,
}

impl Status {
    pub fn new(message: impl Into<String>, tone: Tone) -> Self {
        Self {
            message: message.into(),
            tone,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExportProgress {
    pub operation_id: u64,
    pub document_id: u64,
    pub title: String,
    pub detail: String,
    pub progress: f32,
    pub cancel_requested: bool,
}

#[derive(Clone, Copy)]
pub struct AppContext {
    pub tabs: Signal<Vec<ViewerTab>>,
    pub active_tab: Signal<Option<u64>>,
    pub next_tab_id: Signal<u64>,
    pub preferences: Signal<Preferences>,
    pub images_sheet_open: Signal<bool>,
    pub sprites_sheet_open: Signal<bool>,
    pub status: Signal<Status>,
    pub playing: Signal<bool>,
    pub export: Signal<Option<ExportProgress>>,
    pub dragged_tab: Signal<Option<u64>>,
    pub stage_drag: Signal<Option<StageDrag>>,
    pub stage_size: Signal<[f64; 2]>,
    pub pointer_coord: Signal<Option<[f32; 2]>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub stage: Signal<SharedStage>,
}

impl AppContext {
    pub fn new(theme: Theme) -> Self {
        crate::platform::log_buffer::initialize();
        let mut preferences = crate::platform::load_preferences();
        preferences.theme = theme;
        #[cfg(not(target_arch = "wasm32"))]
        let stage = {
            let stage = SharedStage::default();
            stage.update(|scene| {
                scene.dark_background = match preferences.theme {
                    Theme::Dark => true,
                    Theme::Light => false,
                    Theme::System => crate::platform::system_is_dark(),
                };
            });
            stage
        };
        Self {
            tabs: Signal::new(Vec::new()),
            active_tab: Signal::new(None),
            next_tab_id: Signal::new(1),
            preferences: Signal::new(preferences),
            images_sheet_open: Signal::new(false),
            sprites_sheet_open: Signal::new(false),
            status: Signal::new(Status::default()),
            playing: Signal::new(false),
            export: Signal::new(None),
            dragged_tab: Signal::new(None),
            stage_drag: Signal::new(None),
            stage_size: Signal::new([1.0, 1.0]),
            pointer_coord: Signal::new(None),
            #[cfg(not(target_arch = "wasm32"))]
            stage: Signal::new(stage),
        }
    }

    pub fn active_tab_index(&self) -> Option<usize> {
        let id = *self.active_tab.read();
        self.tabs.read().iter().position(|tab| Some(tab.id) == id)
    }

    pub fn set_status(mut self, status: Status) {
        let level = match status.tone {
            Tone::Neutral => "INFO",
            Tone::Ok => "INFO",
            Tone::Warning => "WARN",
            Tone::Error => "ERROR",
        };
        crate::platform::log_buffer::push(level, "STATUS", &status.message);
        self.status.set(status);
    }

    pub fn active_tab_snapshot(&self) -> Option<ViewerTab> {
        self.active_tab_index()
            .and_then(|index| self.tabs.read().get(index).cloned())
    }

    pub fn update_active_tab(&self, update: impl FnOnce(&mut ViewerTab)) {
        let Some(index) = self.active_tab_index() else {
            return;
        };
        let mut tabs = self.tabs;
        update(&mut tabs.write()[index]);
        self.sync_stage();
    }

    pub fn sync_stage(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let preferences = self.preferences.read();
            let boundary = preferences.boundary;
            let dark_background = match preferences.theme {
                Theme::Dark => true,
                Theme::Light => false,
                Theme::System => crate::platform::system_is_dark(),
            };
            drop(preferences);
            if let Some(tab) = self.active_tab_snapshot() {
                let next = tab.stage_scene(boundary, dark_background);
                self.stage.read().update(|scene| scene.replace(next));
            } else {
                self.stage.read().update(|scene| {
                    scene.set_document(None);
                    scene.boundary = boundary;
                    scene.dark_background = dark_background;
                });
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn shared_stage(&self) -> SharedStage {
        self.stage.read().clone()
    }

    pub fn save_preferences(&self) {
        crate::platform::save_preferences(&self.preferences.read());
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StageDrag {
    Pan {
        start: [f64; 2],
        pan: [f32; 2],
    },
    Boundary {
        edge: BoundaryEdge,
        start: [f64; 2],
        scale: f64,
        size: [f64; 2],
        position: [f64; 2],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}
