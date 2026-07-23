use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures_util::future::{Either, select};
use pam_viewer_core::{RenderScenePayload, RenderViewPayload};
use wasm_bindgen::prelude::*;

use crate::color::srgb_view_format;
use crate::{GpuRenderer, RenderTarget, RendererError, StageScene};

enum CanvasTarget {
    Html(web_sys::HtmlCanvasElement),
    Offscreen(web_sys::OffscreenCanvas),
}

impl CanvasTarget {
    fn set_size(&self, width: u32, height: u32) {
        match self {
            Self::Html(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
            Self::Offscreen(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
        }
    }
}

struct Runtime {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: GpuRenderer,
    config: wgpu::SurfaceConfiguration,
    render_format: wgpu::TextureFormat,
    canvas: CanvasTarget,
    stage: StageScene,
}

impl Runtime {
    fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.canvas.set_size(width, height);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn draw(&mut self) -> crate::Result<()> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.render_format),
            ..Default::default()
        });
        self.renderer.render_stage(
            RenderTarget {
                view: &view,
                x: 0,
                y: 0,
                width: self.config.width,
                height: self.config.height,
                scale_factor: 1.0,
                clear: wgpu::Color::TRANSPARENT,
                corner_radii: [0.0; 4],
            },
            &self.stage,
        )?;
        self.queue.present(frame);
        Ok(())
    }
}

#[wasm_bindgen]
pub struct RendererHandle {
    runtime: Rc<RefCell<Option<Runtime>>>,
}

impl Default for RendererHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl RendererHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self {
            runtime: Rc::new(RefCell::new(None)),
        }
    }

    pub async fn start(
        &self,
        canvas: web_sys::HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        *self.runtime.borrow_mut() = Some(
            create_runtime(CanvasTarget::Html(canvas), width, height, true)
                .await
                .map_err(renderer_js_error)?,
        );
        Ok(())
    }

    pub async fn start_webgl(
        &self,
        canvas: web_sys::HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        *self.runtime.borrow_mut() = Some(
            create_runtime(CanvasTarget::Html(canvas), width, height, false)
                .await
                .map_err(renderer_js_error)?,
        );
        Ok(())
    }

    pub async fn start_offscreen(
        &self,
        canvas: web_sys::OffscreenCanvas,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        *self.runtime.borrow_mut() = Some(
            create_runtime(CanvasTarget::Offscreen(canvas), width, height, true)
                .await
                .map_err(renderer_js_error)?,
        );
        Ok(())
    }

    pub fn set_scene(&self, scene: JsValue) -> Result<(), JsValue> {
        let scene: RenderScenePayload = serde_wasm_bindgen::from_value(scene)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let document = scene
            .document
            .map(|document| document.into_document())
            .map(Arc::new);
        let runtime = &mut *self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        let view = scene.view;
        runtime.stage.replace(StageScene {
            document,
            document_revision: 0,
            sprite: view.sprite,
            frame: view.frame,
            image_filter: view.image_filter,
            sprite_filter: view.sprite_filter,
            zoom: view.zoom,
            pan: view.pan,
            boundary: view.boundary,
            dark_background: view.dark_background,
        });
        Ok(())
    }

    pub fn set_view(&self, view: JsValue) -> Result<(), JsValue> {
        let view: RenderViewPayload = serde_wasm_bindgen::from_value(view)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let runtime = &mut *self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        apply_view(&mut runtime.stage, view);
        Ok(())
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.resize(width, height);
        }
    }

    pub fn frame(&self) -> Result<(), JsValue> {
        let runtime = &mut *self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.draw().map_err(renderer_js_error)
    }

    pub fn destroy(&self) {
        *self.runtime.borrow_mut() = None;
    }
}

fn apply_view(stage: &mut StageScene, view: RenderViewPayload) {
    stage.sprite = view.sprite;
    stage.frame = view.frame;
    stage.image_filter = view.image_filter;
    stage.sprite_filter = view.sprite_filter;
    stage.zoom = view.zoom;
    stage.pan = view.pan;
    stage.boundary = view.boundary;
    stage.dark_background = view.dark_background;
}

async fn create_runtime(
    canvas: CanvasTarget,
    width: u32,
    height: u32,
    allow_webgpu: bool,
) -> crate::Result<Runtime> {
    let width = width.max(1);
    let height = height.max(1);
    canvas.set_size(width, height);
    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if !allow_webgpu {
        instance_descriptor
            .backends
            .remove(wgpu::Backends::BROWSER_WEBGPU);
    } else if instance_descriptor
        .backends
        .contains(wgpu::Backends::BROWSER_WEBGPU)
    {
        let detection = wgpu::util::is_browser_webgpu_supported();
        let timeout = gloo_timers::future::TimeoutFuture::new(2_000);
        futures_util::pin_mut!(detection);
        futures_util::pin_mut!(timeout);
        let supported = matches!(select(detection, timeout).await, Either::Left((true, _)));
        if !supported {
            instance_descriptor
                .backends
                .remove(wgpu::Backends::BROWSER_WEBGPU);
        }
    }
    let instance = wgpu::Instance::new(instance_descriptor);
    let surface = match &canvas {
        CanvasTarget::Html(canvas) => instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| RendererError::WebCanvas(format!("{error:?}")))?,
        CanvasTarget::Offscreen(canvas) => instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
            .map_err(|error| RendererError::WebCanvas(format!("{error:?}")))?,
    };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|_| RendererError::NoAdapter)?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("pam-viewer web device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        })
        .await?;
    let capabilities = surface.get_capabilities(&adapter);
    let surface_format = capabilities
        .formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| capabilities.formats.first().copied())
        .ok_or(RendererError::NoAdapter)?;
    let render_format = srgb_view_format(surface_format).ok_or_else(|| {
        RendererError::WebCanvas(format!(
            "surface format {surface_format:?} has no sRGB render view"
        ))
    })?;
    let view_formats = (render_format != surface_format)
        .then_some(render_format)
        .into_iter()
        .collect();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        color_space: wgpu::SurfaceColorSpace::Srgb,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: capabilities
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::PreMultiplied)
            .or_else(|| capabilities.alpha_modes.first().copied())
            .unwrap_or(wgpu::CompositeAlphaMode::Auto),
        view_formats,
    };
    surface.configure(&device, &config);
    let renderer = GpuRenderer::new(&device, &queue, render_format);
    Ok(Runtime {
        _instance: instance,
        surface,
        device,
        queue,
        renderer,
        config,
        render_format,
        canvas,
        stage: StageScene::default(),
    })
}

fn renderer_js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
