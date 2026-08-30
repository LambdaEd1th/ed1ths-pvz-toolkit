use std::collections::BTreeMap;

use pam_codec::{PamInfo, Rectangle, SpriteInfo};
use serde::{Deserialize, Serialize};

use crate::math::IDENTITY_MATRIX;
use crate::{
    Color, CoreError, ImageAsset, Matrix, Result, multiply_color, multiply_matrix,
    transform_to_matrix,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpriteKey {
    Main,
    Sprite(usize),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerSnapshot {
    pub index: i32,
    pub resource: usize,
    pub is_sprite: bool,
    pub additive: bool,
    pub first_frame: usize,
    pub time_scale: f32,
    pub preload_frame: i32,
    pub transform: Matrix,
    pub color: Color,
    pub source_rectangle: Option<Rectangle>,
}

pub type SpriteTimeline = Vec<Vec<LayerSnapshot>>;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CompiledAnimation {
    pub sprites: Vec<SpriteTimeline>,
    pub main: Option<SpriteTimeline>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrawCommand {
    pub image_index: usize,
    pub matrix: Matrix,
    pub color: Color,
    pub additive: bool,
    pub source_rectangle: Option<Rectangle>,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone)]
struct LiveLayer {
    resource: usize,
    is_sprite: bool,
    additive: bool,
    first_frame: usize,
    time_scale: f32,
    preload_frame: i32,
    transform: Matrix,
    color: Color,
    source_rectangle: Option<Rectangle>,
}

pub fn compile_animation(pam: &PamInfo) -> Result<CompiledAnimation> {
    #[cfg(not(target_arch = "wasm32"))]
    if pam.sprite.len() >= 8 {
        use rayon::prelude::*;

        let (sprites, main) = rayon::join(
            || {
                pam.sprite
                    .par_iter()
                    .map(|sprite| compile_sprite(pam, sprite))
                    .collect::<Result<Vec<_>>>()
            },
            || {
                pam.main_sprite
                    .as_ref()
                    .map(|sprite| compile_sprite(pam, sprite))
                    .transpose()
            },
        );
        return Ok(CompiledAnimation {
            sprites: sprites?,
            main: main?,
        });
    }

    Ok(CompiledAnimation {
        sprites: pam
            .sprite
            .iter()
            .map(|sprite| compile_sprite(pam, sprite))
            .collect::<Result<_>>()?,
        main: pam
            .main_sprite
            .as_ref()
            .map(|sprite| compile_sprite(pam, sprite))
            .transpose()?,
    })
}

fn compile_sprite(pam: &PamInfo, sprite: &SpriteInfo) -> Result<SpriteTimeline> {
    let mut layers = BTreeMap::<i32, LiveLayer>::new();
    let mut timeline = Vec::with_capacity(sprite.frame.len());

    for (frame_index, frame) in sprite.frame.iter().enumerate() {
        for remove in &frame.remove {
            layers.remove(&remove.index);
        }
        for append in &frame.append {
            layers.insert(
                append.index,
                LiveLayer {
                    resource: append.resource as usize,
                    is_sprite: append.sprite,
                    additive: append.additive,
                    first_frame: frame_index,
                    time_scale: append.time_scale,
                    preload_frame: append.preload_frame,
                    transform: IDENTITY_MATRIX,
                    color: Color::WHITE,
                    source_rectangle: None,
                },
            );
        }
        for change in &frame.change {
            let Some(layer) = layers.get_mut(&change.index) else {
                continue;
            };
            layer.transform = transform_to_matrix(&change.transform)?;
            if let Some(color) = change.color {
                layer.color = Color {
                    r: color[0] as f32,
                    g: color[1] as f32,
                    b: color[2] as f32,
                    a: color[3] as f32,
                };
            }
            if let Some(source_rectangle) = change.source_rectangle {
                layer.source_rectangle = Some(source_rectangle);
            }
            if let Some(explicit_frame) = change.sprite_frame_number
                && layer.is_sprite
                && let Some(child) = sprite_for_resource(pam, layer.resource)
                && !child.frame.is_empty()
            {
                let elapsed = frame_index as i32 - layer.first_frame as i32;
                layer.preload_frame = modulo(explicit_frame - elapsed, child.frame.len() as i32);
            }
        }

        timeline.push(
            layers
                .iter()
                .map(|(index, layer)| LayerSnapshot {
                    index: *index,
                    resource: layer.resource,
                    is_sprite: layer.is_sprite,
                    additive: layer.additive,
                    first_frame: layer.first_frame,
                    time_scale: layer.time_scale,
                    preload_frame: layer.preload_frame,
                    transform: layer.transform,
                    color: layer.color,
                    source_rectangle: layer.source_rectangle,
                })
                .collect(),
        );
    }
    Ok(timeline)
}

impl CompiledAnimation {
    pub fn timeline(&self, key: SpriteKey) -> Option<&SpriteTimeline> {
        match key {
            SpriteKey::Main => self.main.as_ref(),
            SpriteKey::Sprite(index) => self.sprites.get(index),
        }
    }

    pub fn flatten_frame(
        &self,
        pam: &PamInfo,
        images: &[Option<ImageAsset>],
        sprite: SpriteKey,
        frame: usize,
        image_filter: &[bool],
        sprite_filter: &[bool],
    ) -> Result<Vec<DrawCommand>> {
        let mut output = Vec::new();
        self.flatten_into(
            pam,
            images,
            sprite,
            frame,
            IDENTITY_MATRIX,
            Color::WHITE,
            false,
            image_filter,
            sprite_filter,
            &mut Vec::new(),
            &mut output,
        )?;
        Ok(output)
    }

    #[allow(clippy::too_many_arguments)]
    fn flatten_into(
        &self,
        pam: &PamInfo,
        images: &[Option<ImageAsset>],
        sprite_key: SpriteKey,
        frame: usize,
        parent_matrix: Matrix,
        parent_color: Color,
        parent_additive: bool,
        image_filter: &[bool],
        sprite_filter: &[bool],
        stack: &mut Vec<SpriteKey>,
        output: &mut Vec<DrawCommand>,
    ) -> Result<()> {
        if stack.len() >= 128 || stack.contains(&sprite_key) {
            return Ok(());
        }
        let sprite = sprite_by_key(pam, sprite_key).ok_or(CoreError::MissingSprite(sprite_key))?;
        let timeline = self
            .timeline(sprite_key)
            .ok_or(CoreError::MissingSprite(sprite_key))?;
        if sprite.frame.is_empty() {
            return Ok(());
        }
        let actual_frame = frame % sprite.frame.len();
        let Some(snapshot) = timeline.get(actual_frame) else {
            return Ok(());
        };

        stack.push(sprite_key);
        for layer in snapshot {
            let matrix = multiply_matrix(parent_matrix, layer.transform);
            let color = multiply_color(parent_color, layer.color);
            let additive = parent_additive || layer.additive;
            if layer.is_sprite {
                if layer.resource < sprite_filter.len() && !sprite_filter[layer.resource] {
                    continue;
                }
                let child_key = if layer.resource == pam.sprite.len() {
                    SpriteKey::Main
                } else {
                    SpriteKey::Sprite(layer.resource)
                };
                let Some(child) = sprite_by_key(pam, child_key) else {
                    continue;
                };
                if child.frame.is_empty() {
                    continue;
                }
                let elapsed = actual_frame as i64 - layer.first_frame as i64;
                let scaled = (elapsed as f64 * layer.time_scale as f64).floor() as i64;
                let child_frame = modulo_i64(
                    scaled + layer.preload_frame as i64,
                    child.frame.len() as i64,
                ) as usize;
                self.flatten_into(
                    pam,
                    images,
                    child_key,
                    child_frame,
                    matrix,
                    color,
                    additive,
                    image_filter,
                    sprite_filter,
                    stack,
                    output,
                )?;
                continue;
            }

            if layer.resource < image_filter.len() && !image_filter[layer.resource] {
                continue;
            }
            let Some(image_definition) = pam.image.get(layer.resource) else {
                continue;
            };
            let Some(asset) = images.get(layer.resource).and_then(Option::as_ref) else {
                continue;
            };
            let image_matrix = transform_to_matrix(&image_definition.transform)?;
            let final_matrix = multiply_matrix(matrix, image_matrix);
            let source_width = layer
                .source_rectangle
                .map(|rect| rect.size[0] as f32)
                .unwrap_or(asset.width as f32);
            let source_height = layer
                .source_rectangle
                .map(|rect| rect.size[1] as f32)
                .unwrap_or(asset.height as f32);
            let width = image_definition
                .size
                .map(|size| size[0] as f32)
                .unwrap_or(source_width);
            let height = image_definition
                .size
                .map(|size| size[1] as f32)
                .unwrap_or(source_height);
            if width > 0.0 && height > 0.0 && source_width > 0.0 && source_height > 0.0 {
                output.push(DrawCommand {
                    image_index: layer.resource,
                    matrix: final_matrix,
                    color: color.clamped(),
                    additive,
                    source_rectangle: layer.source_rectangle,
                    width,
                    height,
                });
            }
        }
        stack.pop();
        Ok(())
    }
}

fn sprite_for_resource(pam: &PamInfo, resource: usize) -> Option<&SpriteInfo> {
    if resource == pam.sprite.len() {
        pam.main_sprite.as_ref()
    } else {
        pam.sprite.get(resource)
    }
}

fn sprite_by_key(pam: &PamInfo, key: SpriteKey) -> Option<&SpriteInfo> {
    match key {
        SpriteKey::Main => pam.main_sprite.as_ref(),
        SpriteKey::Sprite(index) => pam.sprite.get(index),
    }
}

fn modulo(value: i32, count: i32) -> i32 {
    ((value % count) + count) % count
}

fn modulo_i64(value: i64, count: i64) -> i64 {
    ((value % count) + count) % count
}
