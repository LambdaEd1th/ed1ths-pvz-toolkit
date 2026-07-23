use pam_viewer_core::{Color, DrawCommand, ImageAsset, Matrix, PamDocument, Rectangle};

use crate::StageScene;
use crate::color::linear_rgb_from_srgb8;

use super::GpuInstance;

#[derive(Clone, Copy)]
pub(super) struct RenderQuad {
    pub(super) instance: GpuInstance,
    pub(super) texture: usize,
    pub(super) additive: bool,
}

pub(super) fn background_quad(width: u32, height: u32, dark: bool) -> RenderQuad {
    let [r, g, b] = if dark {
        linear_rgb_from_srgb8([24, 27, 32])
    } else {
        linear_rgb_from_srgb8([250, 250, 250])
    };
    RenderQuad {
        instance: GpuInstance {
            matrix: [1.0, 0.0, 0.0, 1.0],
            translation_size: [0.0, 0.0, width as f32, height as f32],
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [r, g, b, -1.0],
        },
        texture: usize::MAX,
        additive: false,
    }
}

pub(super) fn draw_commands_to_quads(
    commands: &[DrawCommand],
    images: &[Option<ImageAsset>],
    camera: Matrix,
) -> Vec<RenderQuad> {
    commands
        .iter()
        .filter_map(|command| {
            let asset = images.get(command.image_index)?.as_ref()?;
            let matrix = pam_viewer_core::multiply_matrix(camera, command.matrix);
            Some(RenderQuad {
                instance: GpuInstance {
                    matrix: [matrix[0], matrix[1], matrix[2], matrix[3]],
                    translation_size: [matrix[4], matrix[5], command.width, command.height],
                    uv: source_uv(command.source_rectangle, asset.width, asset.height),
                    color: [
                        command.color.r,
                        command.color.g,
                        command.color.b,
                        command.color.a,
                    ],
                },
                texture: command.image_index,
                additive: command.additive,
            })
        })
        .collect()
}

fn source_uv(rectangle: Option<Rectangle>, width: u32, height: u32) -> [f32; 4] {
    let width = width.max(1) as f32;
    let height = height.max(1) as f32;
    rectangle
        .map(|rectangle| {
            let x = rectangle.position[0] as f32;
            let y = rectangle.position[1] as f32;
            let right = x + rectangle.size[0] as f32;
            let bottom = y + rectangle.size[1] as f32;
            [
                (x / width).clamp(0.0, 1.0),
                (y / height).clamp(0.0, 1.0),
                (right / width).clamp(0.0, 1.0),
                (bottom / height).clamp(0.0, 1.0),
            ]
        })
        .unwrap_or([0.0, 0.0, 1.0, 1.0])
}

pub(super) fn stage_camera(
    scene: &StageScene,
    width: u32,
    height: u32,
    scale_factor: f32,
) -> Matrix {
    let bounds = scene.stage_bounds().unwrap_or_default();
    let padding = 36.0 * scale_factor;
    let available_width = (width as f32 - padding * 2.0).max(1.0);
    let available_height = (height as f32 - padding * 2.0).max(1.0);
    let fit = (available_width / bounds.width.max(1.0))
        .min(available_height / bounds.height.max(1.0))
        .max(0.0001);
    let scale = fit * scene.zoom;
    [
        scale,
        0.0,
        0.0,
        scale,
        width as f32 / 2.0 + scene.pan[0] * scale,
        height as f32 / 2.0 + scene.pan[1] * scale,
    ]
}

pub(super) fn export_camera(document: &PamDocument, width: u32, height: u32) -> Matrix {
    let scale = (width as f32 / document.pam.size[0].max(1.0) as f32)
        .min(height as f32 / document.pam.size[1].max(1.0) as f32);
    [
        scale,
        0.0,
        0.0,
        scale,
        document.pam.position[0] as f32 * scale,
        document.pam.position[1] as f32 * scale,
    ]
}

pub(super) fn append_boundary_quads(
    quads: &mut Vec<RenderQuad>,
    document: &PamDocument,
    camera: Matrix,
    scale_factor: f32,
) {
    let bounds = document.pam_bounds();
    let x = camera[0] * bounds.x + camera[4];
    let y = camera[3] * bounds.y + camera[5];
    let width = bounds.width * camera[0];
    let height = bounds.height * camera[3];
    let line = scale_factor.max(1.0);
    let blue = Color {
        r: 0.0,
        g: 0.78,
        b: 1.0,
        a: 0.72,
    };
    push_solid_quad(quads, x, y, width, line, blue);
    push_solid_quad(quads, x, y + height - line, width, line, blue);
    push_solid_quad(quads, x, y, line, height, blue);
    push_solid_quad(quads, x + width - line, y, line, height, blue);

    let handle = 5.0 * scale_factor;
    for [hx, hy] in [
        [x, y],
        [x + width / 2.0, y],
        [x + width, y],
        [x, y + height / 2.0],
        [x + width, y + height / 2.0],
        [x, y + height],
        [x + width / 2.0, y + height],
        [x + width, y + height],
    ] {
        push_solid_quad(
            quads,
            hx - handle / 2.0,
            hy - handle / 2.0,
            handle,
            handle,
            blue,
        );
    }
    let center_x = camera[4];
    let center_y = camera[5];
    let red = Color {
        r: 1.0,
        g: 0.39,
        b: 0.39,
        a: 0.72,
    };
    push_solid_quad(quads, center_x - 10.0, center_y, 20.0, line, red);
    push_solid_quad(quads, center_x, center_y - 10.0, line, 20.0, red);
}

fn push_solid_quad(
    quads: &mut Vec<RenderQuad>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
) {
    quads.push(RenderQuad {
        instance: GpuInstance {
            matrix: [1.0, 0.0, 0.0, 1.0],
            translation_size: [x, y, width, height],
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [color.r, color.g, color.b, color.a],
        },
        texture: usize::MAX,
        additive: false,
    });
}
