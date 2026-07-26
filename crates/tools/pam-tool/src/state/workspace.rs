use std::sync::Arc;

use pam_viewer_core::{
    FrameLabel, LoadedPamPayload, PamDocument, SpecialLayerIndices, SpriteInfo, SpriteKey,
};
#[cfg(target_arch = "wasm32")]
use pam_viewer_core::{RenderDocumentGeometryPayload, RenderViewPayload};
#[cfg(not(target_arch = "wasm32"))]
use pam_viewer_renderer::StageScene;

use super::Preferences;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRange {
    pub begin: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
pub struct ViewerTab {
    pub id: u64,
    pub processing_id: u64,
    pub document: Arc<PamDocument>,
    pub active_sprite: SpriteKey,
    pub frame_range: FrameRange,
    pub current_frame: usize,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub image_filter: Vec<bool>,
    pub sprite_filter: Vec<bool>,
    pub special_layers: SpecialLayerIndices,
    pub speed_fps: u32,
    pub export_size: [u32; 2],
    pub export_scale: Option<u32>,
    pub image_regex: String,
    pub sprite_regex: String,
    pub selected_label: Option<usize>,
    pub loaded_images: usize,
    pub image_thumbnails: Vec<Option<String>>,
}

fn apply_default_sprite_visibility(
    sprite_filter: &mut [bool],
    special_layers: &SpecialLayerIndices,
) {
    sprite_filter.fill(true);
    for index in special_layers
        .default_hidden_layers
        .iter()
        .chain(&special_layers.zombie_state_layers)
    {
        if let Some(visible) = sprite_filter.get_mut(*index) {
            *visible = false;
        }
    }
}

impl ViewerTab {
    pub fn new(
        id: u64,
        loaded: LoadedPamPayload,
        preferences: &Preferences,
    ) -> Result<Self, String> {
        let document = Arc::new(
            loaded
                .document
                .into_document()
                .map_err(|error| error.to_string())?,
        );
        let active_sprite = if document.pam.main_sprite.is_some() {
            SpriteKey::Main
        } else {
            SpriteKey::Sprite(0)
        };
        let frame_count = sprite_for_document(&document, active_sprite)
            .map(|sprite| sprite.frame.len())
            .unwrap_or(0);
        let special_layers =
            pam_viewer_core::special_layer_indices(&document.pam, &document.source_name);
        let mut sprite_filter = vec![true; document.pam.sprite.len()];
        apply_default_sprite_visibility(&mut sprite_filter, &special_layers);
        let bounds = document.stage_bounds();
        let native_fps = sprite_for_document(&document, active_sprite)
            .and_then(|sprite| sprite.frame_rate)
            .unwrap_or(document.pam.frame_rate as f64)
            .round()
            .clamp(1.0, 120.0) as u32;
        let speed_fps = if preferences.keep_speed {
            preferences.speed_fps.unwrap_or(native_fps)
        } else {
            native_fps
        };
        let export_size = [
            document.pam.size[0].round().max(1.0) as u32,
            document.pam.size[1].round().max(1.0) as u32,
        ];
        let image_count = document.pam.image.len();
        let image_thumbnails = document
            .images
            .iter()
            .map(|asset| {
                asset
                    .as_ref()
                    .and_then(|asset| crate::platform::thumbnail_url(&asset.encoded))
            })
            .collect();
        let range = FrameRange {
            begin: 0,
            end: frame_count.saturating_sub(1),
        };
        Ok(Self {
            id,
            processing_id: id,
            document,
            active_sprite,
            frame_range: range,
            current_frame: if preferences.reverse {
                range.end
            } else {
                range.begin
            },
            zoom: 1.0,
            pan: [
                -bounds.x - bounds.width / 2.0,
                -bounds.y - bounds.height / 2.0,
            ],
            image_filter: vec![true; image_count],
            sprite_filter,
            special_layers,
            speed_fps,
            export_size,
            export_scale: Some(1),
            image_regex: String::new(),
            sprite_regex: String::new(),
            selected_label: None,
            loaded_images: loaded.loaded_images,
            image_thumbnails,
        })
    }

    pub fn restore_default_sprite_visibility(&mut self) {
        apply_default_sprite_visibility(&mut self.sprite_filter, &self.special_layers);
    }

    pub fn display_name(&self) -> String {
        self.document
            .source_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&self.document.source_name)
            .to_string()
    }

    pub fn active_sprite_info(&self) -> Option<&SpriteInfo> {
        sprite_for_document(&self.document, self.active_sprite)
    }

    pub fn labels(&self) -> Vec<FrameLabel> {
        self.active_sprite_info()
            .map(pam_viewer_core::parse_frame_labels)
            .unwrap_or_default()
    }

    pub fn frame_count(&self) -> usize {
        self.active_sprite_info()
            .map(|sprite| sprite.frame.len())
            .unwrap_or(0)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn render_view(&self, boundary: bool, dark_background: bool) -> RenderViewPayload {
        RenderViewPayload {
            sprite: self.active_sprite,
            frame: self.current_frame,
            image_filter: self.image_filter.clone(),
            sprite_filter: self.sprite_filter.clone(),
            zoom: self.zoom,
            pan: self.pan,
            document_geometry: Some(RenderDocumentGeometryPayload {
                position: self.document.pam.position,
                size: self.document.pam.size,
            }),
            boundary,
            dark_background,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn stage_scene(&self, boundary: bool, dark_background: bool) -> StageScene {
        StageScene {
            document: Some(self.document.clone()),
            document_revision: 0,
            sprite: self.active_sprite,
            frame: self.current_frame,
            image_filter: self.image_filter.clone(),
            sprite_filter: self.sprite_filter.clone(),
            zoom: self.zoom,
            pan: self.pan,
            boundary,
            dark_background,
        }
    }
}

pub fn sprite_for_document(document: &PamDocument, key: SpriteKey) -> Option<&SpriteInfo> {
    match key {
        SpriteKey::Main => document.pam.main_sprite.as_ref(),
        SpriteKey::Sprite(index) => document.pam.sprite.get(index),
    }
}
