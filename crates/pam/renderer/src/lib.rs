mod color;
mod gpu;
mod scene;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(all(target_arch = "wasm32", feature = "web-host"))]
mod web;

pub use gpu::{
    GpuRenderer, RenderTarget, render_offscreen_frames, render_offscreen_frames_with_cancel,
};
#[cfg(not(target_arch = "wasm32"))]
pub use native::{NativeStageRenderer, NativeViewport};
pub use scene::{SharedStage, StageScene};
#[cfg(all(target_arch = "wasm32", feature = "web-host"))]
pub use web::RendererHandle;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("no compatible GPU adapter is available")]
    NoAdapter,
    #[error("failed to request a GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("GPU buffer mapping failed: {0}")]
    BufferMap(#[from] wgpu::BufferAsyncError),
    #[error("GPU callback was dropped")]
    CallbackDropped,
    #[error("GPU buffer access failed: {0}")]
    BufferAccess(String),
    #[error("animation export was cancelled")]
    Cancelled,
    #[error("animation render failed: {0}")]
    Core(#[from] pam_viewer_core::CoreError),
    #[error("web canvas is unavailable: {0}")]
    WebCanvas(String),
    #[error("native surface is unavailable: {0}")]
    NativeSurface(String),
}

pub type Result<T> = std::result::Result<T, RendererError>;
