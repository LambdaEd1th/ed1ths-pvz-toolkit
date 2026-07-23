use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_channel::oneshot;
use pam_viewer_core::{PamDocument, SpriteKey};

use crate::{RendererError, Result, StageScene};

use super::geometry::{draw_commands_to_quads, export_camera};
use super::{GpuRenderer, RenderTarget};

const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

pub async fn render_offscreen_frames(
    document: Arc<PamDocument>,
    sprite: SpriteKey,
    frames: &[usize],
    image_filter: &[bool],
    sprite_filter: &[bool],
    width: u32,
    height: u32,
) -> Result<Vec<Vec<u8>>> {
    render_offscreen_frames_with_cancel(
        document,
        sprite,
        frames,
        image_filter,
        sprite_filter,
        width,
        height,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn render_offscreen_frames_with_cancel(
    document: Arc<PamDocument>,
    sprite: SpriteKey,
    frames: &[usize],
    image_filter: &[bool],
    sprite_filter: &[bool],
    width: u32,
    height: u32,
    cancelled: Option<&AtomicBool>,
) -> Result<Vec<Vec<u8>>> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|_| RendererError::NoAdapter)?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("pam-viewer export device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        })
        .await?;
    let width = width.max(1);
    let height = height.max(1);
    let mut renderer = GpuRenderer::new(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb);
    let scene = StageScene {
        document: Some(document.clone()),
        document_revision: 1,
        sprite,
        image_filter: image_filter.to_vec(),
        sprite_filter: sprite_filter.to_vec(),
        boundary: false,
        ..StageScene::default()
    };
    renderer.ensure_document_textures(&scene);
    let camera = export_camera(&document, width, height);
    let build_quads = |frame: &usize| {
        ensure_not_cancelled(cancelled)?;
        let commands = document.compiled.flatten_frame(
            &document.pam,
            &document.images,
            sprite,
            *frame,
            image_filter,
            sprite_filter,
        )?;
        Ok(draw_commands_to_quads(&commands, &document.images, camera))
    };
    #[cfg(not(target_arch = "wasm32"))]
    let frame_quads = if frames.len() >= 8 {
        use rayon::prelude::*;
        frames
            .par_iter()
            .map(build_quads)
            .collect::<Result<Vec<_>>>()?
    } else {
        frames.iter().map(build_quads).collect::<Result<Vec<_>>>()?
    };
    #[cfg(target_arch = "wasm32")]
    let frame_quads = frames.iter().map(build_quads).collect::<Result<Vec<_>>>()?;

    let unpadded = width * 4;
    let padded = unpadded.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;
    let pipeline_depth = frame_quads.len().clamp(1, 3);
    let targets = (0..pipeline_depth)
        .map(|index| ReadbackTarget::new(&device, width, height, padded, index))
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(frames.len());
    let mut pending = VecDeque::<PendingReadback>::with_capacity(pipeline_depth);
    for (index, quads) in frame_quads.iter().enumerate() {
        ensure_not_cancelled(cancelled)?;
        if pending.len() == pipeline_depth {
            let readback = pending.pop_front().expect("full readback pipeline");
            output.push(
                finish_readback(
                    &device,
                    &targets[readback.slot],
                    readback,
                    unpadded,
                    padded,
                    height,
                )
                .await?,
            );
        }
        let slot = index % pipeline_depth;
        let target = &targets[slot];
        renderer.render_quads(
            RenderTarget {
                view: &target.view,
                x: 0,
                y: 0,
                width,
                height,
                scale_factor: 1.0,
                clear: wgpu::Color::TRANSPARENT,
                corner_radii: [0.0; 4],
            },
            quads,
        )?;
        pending.push_back(begin_readback(
            &device, &queue, target, width, height, padded, slot,
        ));
    }
    while let Some(readback) = pending.pop_front() {
        output.push(
            finish_readback(
                &device,
                &targets[readback.slot],
                readback,
                unpadded,
                padded,
                height,
            )
            .await?,
        );
    }
    ensure_not_cancelled(cancelled)?;
    Ok(output)
}

struct ReadbackTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    buffer: wgpu::Buffer,
}

impl ReadbackTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32, padded: u32, index: usize) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("pam-viewer export target {index}")),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("pam-viewer export readback {index}")),
            size: padded as u64 * height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            texture,
            view,
            buffer,
        }
    }
}

struct PendingReadback {
    slot: usize,
    receiver: oneshot::Receiver<std::result::Result<(), wgpu::BufferAsyncError>>,
}

fn begin_readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &ReadbackTarget,
    width: u32,
    height: u32,
    padded: u32,
    slot: usize,
) -> PendingReadback {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("pam-viewer export copy"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &target.buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = target.buffer.slice(..);
    let (sender, receiver) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    PendingReadback { slot, receiver }
}

async fn finish_readback(
    _device: &wgpu::Device,
    target: &ReadbackTarget,
    readback: PendingReadback,
    unpadded: u32,
    padded: u32,
    height: u32,
) -> Result<Vec<u8>> {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = _device.poll(wgpu::PollType::wait_indefinitely());
    readback
        .receiver
        .await
        .map_err(|_| RendererError::CallbackDropped)??;
    let mapped = target
        .buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|error| RendererError::BufferAccess(error.to_string()))?;
    let mut rgba = vec![0; unpadded as usize * height as usize];
    for row in 0..height as usize {
        let source = &mapped[row * padded as usize..row * padded as usize + unpadded as usize];
        let destination = &mut rgba[row * unpadded as usize..(row + 1) * unpadded as usize];
        destination.copy_from_slice(source);
    }
    drop(mapped);
    target.buffer.unmap();
    Ok(rgba)
}

fn ensure_not_cancelled(cancelled: Option<&AtomicBool>) -> Result<()> {
    if cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Relaxed)) {
        Err(RendererError::Cancelled)
    } else {
        Ok(())
    }
}
