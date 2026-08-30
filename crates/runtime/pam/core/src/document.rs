use std::sync::Arc;

use pam_codec::PamInfo;

use crate::{CompiledAnimation, Rect, Result, compile_animation};

#[derive(Clone, Debug)]
pub struct ImageAsset {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
    pub encoded: Arc<[u8]>,
}

impl ImageAsset {
    pub fn new(
        name: impl Into<String>,
        width: u32,
        height: u32,
        rgba: impl Into<Arc<[u8]>>,
        encoded: impl Into<Arc<[u8]>>,
    ) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            rgba: rgba.into(),
            encoded: encoded.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PamDocument {
    pub source_name: String,
    pub pam: PamInfo,
    pub images: Vec<Option<ImageAsset>>,
    pub compiled: CompiledAnimation,
    content_bounds: Option<Rect>,
}

impl PamDocument {
    pub fn new(
        source_name: impl Into<String>,
        pam: PamInfo,
        images: Vec<Option<ImageAsset>>,
    ) -> Result<Self> {
        let compiled = compile_animation(&pam)?;
        let mut document = Self {
            source_name: source_name.into(),
            pam,
            images,
            compiled,
            content_bounds: None,
        };
        document.refresh_content_bounds();
        Ok(document)
    }

    pub fn from_compiled(
        source_name: impl Into<String>,
        pam: PamInfo,
        images: Vec<Option<ImageAsset>>,
        compiled: CompiledAnimation,
        content_bounds: Option<Rect>,
    ) -> Self {
        Self {
            source_name: source_name.into(),
            pam,
            images,
            compiled,
            content_bounds,
        }
    }

    pub fn rebuild(&mut self) -> Result<()> {
        self.compiled = compile_animation(&self.pam)?;
        self.refresh_content_bounds();
        Ok(())
    }

    pub fn pam_bounds(&self) -> Rect {
        Rect {
            x: -self.pam.position[0] as f32,
            y: -self.pam.position[1] as f32,
            width: self.pam.size[0].max(1.0) as f32,
            height: self.pam.size[1].max(1.0) as f32,
        }
    }

    pub fn content_bounds(&self) -> Option<Rect> {
        self.content_bounds
    }

    fn refresh_content_bounds(&mut self) {
        let Some(root) = self.pam.main_sprite.as_ref() else {
            self.content_bounds = None;
            return;
        };
        if root.frame.is_empty() {
            self.content_bounds = None;
            return;
        }
        let image_filter = vec![true; self.pam.image.len()];
        let sprite_filter = vec![true; self.pam.sprite.len()];
        let frame_bounds = |frame| {
            self.compiled
                .flatten_frame(
                    &self.pam,
                    &self.images,
                    crate::SpriteKey::Main,
                    frame,
                    &image_filter,
                    &sprite_filter,
                )
                .ok()
                .and_then(|commands| bounds_for_commands(&commands))
        };
        #[cfg(not(target_arch = "wasm32"))]
        let bounds = if root.frame.len() >= 32 {
            use rayon::prelude::*;
            (0..root.frame.len())
                .into_par_iter()
                .filter_map(frame_bounds)
                .reduce_with(Rect::union)
        } else {
            (0..root.frame.len())
                .filter_map(frame_bounds)
                .reduce(Rect::union)
        };
        #[cfg(target_arch = "wasm32")]
        let bounds = (0..root.frame.len())
            .filter_map(frame_bounds)
            .reduce(Rect::union);
        self.content_bounds = bounds;
    }

    pub fn stage_bounds(&self) -> Rect {
        self.content_bounds()
            .map(|bounds| self.pam_bounds().union(bounds))
            .unwrap_or_else(|| self.pam_bounds())
    }
}

fn bounds_for_commands(commands: &[crate::DrawCommand]) -> Option<Rect> {
    commands
        .iter()
        .filter_map(|command| {
            let points = [
                crate::math::transform_point(command.matrix, 0.0, 0.0),
                crate::math::transform_point(command.matrix, command.width, 0.0),
                crate::math::transform_point(command.matrix, command.width, command.height),
                crate::math::transform_point(command.matrix, 0.0, command.height),
            ];
            Rect::from_points(points)
        })
        .reduce(Rect::union)
}
