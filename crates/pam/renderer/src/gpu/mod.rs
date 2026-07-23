mod geometry;
mod offscreen;
mod resources;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{Result, StageScene};

use geometry::{
    RenderQuad, append_boundary_quads, background_quad, draw_commands_to_quads, stage_camera,
};
pub use offscreen::{render_offscreen_frames, render_offscreen_frames_with_cancel};
use resources::{
    additive_blend, create_instance_buffer, create_pipeline, create_scene_bind_group, upload_asset,
    upload_texture,
};

const INITIAL_INSTANCE_CAPACITY: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ViewUniform {
    viewport: [f32; 2],
    padding: [f32; 2],
    corner_radii: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuInstance {
    matrix: [f32; 4],
    translation_size: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
}

struct GpuTexture {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderTarget<'a> {
    pub view: &'a wgpu::TextureView,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub clear: wgpu::Color,
    /// Top-left, top-right, bottom-right, and bottom-left radii in target pixels.
    pub corner_radii: [f32; 4],
}

pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    normal_pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    view_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    scene_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<Option<GpuTexture>>,
    white: GpuTexture,
    uploaded_revision: Option<u64>,
}

impl GpuRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pam-viewer shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader.wgsl").into()),
        });
        let view_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pam-viewer view uniform"),
            contents: bytemuck::bytes_of(&ViewUniform {
                viewport: [1.0, 1.0],
                padding: [0.0; 2],
                corner_radii: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let instance_buffer = create_instance_buffer(device, INITIAL_INSTANCE_CAPACITY);
        let scene_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pam-viewer scene layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let scene_bind_group =
            create_scene_bind_group(device, &scene_bind_group_layout, &view_buffer);
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pam-viewer texture layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pam-viewer image sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pam-viewer pipeline layout"),
            bind_group_layouts: &[
                Some(&scene_bind_group_layout),
                Some(&texture_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let normal_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            format,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "pam-viewer normal pipeline",
        );
        let additive_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            format,
            additive_blend(),
            "pam-viewer additive pipeline",
        );
        let white = upload_texture(
            device,
            queue,
            &texture_bind_group_layout,
            &sampler,
            1,
            1,
            &[255, 255, 255, 255],
            "pam-viewer white texture",
        );
        Self {
            device: device.clone(),
            queue: queue.clone(),
            normal_pipeline,
            additive_pipeline,
            view_buffer,
            instance_buffer,
            instance_capacity: INITIAL_INSTANCE_CAPACITY,
            scene_bind_group,
            texture_bind_group_layout,
            sampler,
            textures: Vec::new(),
            white,
            uploaded_revision: None,
        }
    }

    pub fn ensure_document_textures(&mut self, scene: &StageScene) {
        if self.uploaded_revision == Some(scene.document_revision) {
            return;
        }
        self.textures.clear();
        if let Some(document) = scene.document.as_ref() {
            self.textures = document
                .images
                .iter()
                .enumerate()
                .map(|(index, asset)| {
                    asset.as_ref().map(|asset| {
                        upload_asset(
                            &self.device,
                            &self.queue,
                            &self.texture_bind_group_layout,
                            &self.sampler,
                            asset,
                            &format!("pam-viewer image {index}"),
                        )
                    })
                })
                .collect();
        }
        self.uploaded_revision = Some(scene.document_revision);
    }

    pub fn render_stage(&mut self, target: RenderTarget<'_>, scene: &StageScene) -> Result<()> {
        self.ensure_document_textures(scene);
        let Some(document) = scene.document.as_ref() else {
            return self.render_quads(
                target,
                &[background_quad(
                    target.width,
                    target.height,
                    scene.dark_background,
                )],
            );
        };
        let commands = document.compiled.flatten_frame(
            &document.pam,
            &document.images,
            scene.sprite,
            scene.frame,
            &scene.image_filter,
            &scene.sprite_filter,
        )?;
        let camera = stage_camera(scene, target.width, target.height, target.scale_factor);
        let mut quads = vec![background_quad(
            target.width,
            target.height,
            scene.dark_background,
        )];
        quads.extend(draw_commands_to_quads(&commands, &document.images, camera));
        if scene.boundary {
            append_boundary_quads(&mut quads, document, camera, target.scale_factor);
        }
        self.render_quads(target, &quads)
    }

    fn render_quads(&mut self, target: RenderTarget<'_>, quads: &[RenderQuad]) -> Result<()> {
        self.ensure_instance_capacity(quads.len().max(1));
        let instances = quads.iter().map(|quad| quad.instance).collect::<Vec<_>>();
        if !instances.is_empty() {
            self.queue
                .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
        self.queue.write_buffer(
            &self.view_buffer,
            0,
            bytemuck::bytes_of(&ViewUniform {
                viewport: [target.width.max(1) as f32, target.height.max(1) as f32],
                padding: [0.0; 2],
                corner_radii: target.corner_radii,
            }),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pam-viewer frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pam-viewer frame pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(target.clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                target.x as f32,
                target.y as f32,
                target.width.max(1) as f32,
                target.height.max(1) as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(
                target.x,
                target.y,
                target.width.max(1),
                target.height.max(1),
            );
            pass.set_bind_group(0, &self.scene_bind_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            let mut additive = None;
            let mut texture_index = None;
            for (index, quad) in quads.iter().enumerate() {
                if additive != Some(quad.additive) {
                    pass.set_pipeline(if quad.additive {
                        &self.additive_pipeline
                    } else {
                        &self.normal_pipeline
                    });
                    additive = Some(quad.additive);
                }
                if texture_index != Some(quad.texture) {
                    let bind_group = if quad.texture == usize::MAX {
                        &self.white.bind_group
                    } else if let Some(texture) =
                        self.textures.get(quad.texture).and_then(Option::as_ref)
                    {
                        &texture.bind_group
                    } else {
                        continue;
                    };
                    pass.set_bind_group(1, bind_group, &[]);
                    texture_index = Some(quad.texture);
                }
                pass.draw(0..6, index as u32..index as u32 + 1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    fn ensure_instance_capacity(&mut self, required: usize) {
        if required <= self.instance_capacity {
            return;
        }
        self.instance_capacity = required.next_power_of_two();
        self.instance_buffer = create_instance_buffer(&self.device, self.instance_capacity);
    }
}
