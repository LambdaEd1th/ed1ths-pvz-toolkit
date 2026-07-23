use crate::color::wgpu_color_from_srgb8;
use crate::{GpuRenderer, RenderTarget, RendererError, Result, SharedStage};

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeViewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub window_width: f32,
    pub window_height: f32,
    pub corner_radii: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct PhysicalViewport {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    scale: f32,
    corner_radii: [f32; 4],
}

impl NativeViewport {
    fn physical(self, surface_width: u32, surface_height: u32) -> PhysicalViewport {
        let scale_x = surface_width as f32 / self.window_width.max(1.0);
        let scale_y = surface_height as f32 / self.window_height.max(1.0);
        let left = (self.x * scale_x).round().max(0.0) as u32;
        let top = (self.y * scale_y).round().max(0.0) as u32;
        let right = ((self.x + self.width) * scale_x)
            .round()
            .max(left as f32 + 1.0) as u32;
        let bottom = ((self.y + self.height) * scale_y)
            .round()
            .max(top as f32 + 1.0) as u32;
        let x = left.min(surface_width.saturating_sub(1));
        let y = top.min(surface_height.saturating_sub(1));
        let width = right.min(surface_width).saturating_sub(x).max(1);
        let height = bottom.min(surface_height).saturating_sub(y).max(1);
        PhysicalViewport {
            x,
            y,
            width,
            height,
            scale: ((scale_x + scale_y) * 0.5).max(0.01),
            corner_radii: self.corner_radii.map(|radius| {
                (radius * ((scale_x + scale_y) * 0.5)).clamp(0.0, width.min(height) as f32 * 0.5)
            }),
        }
    }
}

pub struct NativeStageRenderer {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: GpuRenderer,
    stage: SharedStage,
    viewport: NativeViewport,
    backdrop: wgpu::Color,
}

impl NativeStageRenderer {
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        stage: SharedStage,
    ) -> Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(target)
            .map_err(|error| RendererError::NativeSurface(error.to_string()))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|_| RendererError::NoAdapter)?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("pam-viewer native device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            }))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(RendererError::NoAdapter)?;
        let width = width.max(1);
        let height = height.max(1);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let renderer = GpuRenderer::new(&device, &queue, format);
        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            config,
            renderer,
            stage,
            viewport: NativeViewport {
                width: width as f32,
                height: height as f32,
                window_width: width as f32,
                window_height: height as f32,
                ..NativeViewport::default()
            },
            backdrop: dark_backdrop(),
        })
    }

    pub fn set_viewport(&mut self, viewport: NativeViewport) {
        self.viewport = viewport;
    }

    pub fn set_dark_background(&mut self, dark: bool) {
        self.backdrop = if dark {
            dark_backdrop()
        } else {
            light_backdrop()
        };
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn frame(&mut self) -> Result<()> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RendererError::NativeSurface(
                    "surface frame failed validation".into(),
                ));
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let viewport = self
            .viewport
            .physical(self.config.width, self.config.height);
        let scene = self.stage.snapshot();
        self.renderer.render_stage(
            RenderTarget {
                view: &view,
                x: viewport.x,
                y: viewport.y,
                width: viewport.width,
                height: viewport.height,
                scale_factor: viewport.scale,
                clear: self.backdrop,
                corner_radii: viewport.corner_radii,
            },
            &scene,
        )?;
        self.queue.present(frame);
        Ok(())
    }
}

fn dark_backdrop() -> wgpu::Color {
    wgpu_color_from_srgb8([16, 19, 24])
}

fn light_backdrop() -> wgpu::Color {
    wgpu_color_from_srgb8([248, 251, 252])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_scales_from_webview_to_surface_pixels() {
        let viewport = NativeViewport {
            x: 100.0,
            y: 50.0,
            width: 800.0,
            height: 500.0,
            window_width: 1_000.0,
            window_height: 600.0,
            corner_radii: [0.0, 0.0, 24.0, 24.0],
        };
        let physical = viewport.physical(2_000, 1_200);
        assert_eq!([physical.x, physical.y], [200, 100]);
        assert_eq!([physical.width, physical.height], [1_600, 1_000]);
        assert_eq!(physical.scale, 2.0);
        assert_eq!(physical.corner_radii, [0.0, 0.0, 48.0, 48.0]);
    }
}
