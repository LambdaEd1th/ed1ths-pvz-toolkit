mod animation;
pub mod fla;
mod loader;
mod text;

pub use animation::{
    encode_animated_webp, encode_animated_webp_with_cancel, encode_apng, encode_apng_with_cancel,
    encode_png,
};
pub use fla::{export_fla, export_fla_with_cancel, generate_xfl};
pub use loader::{InputFile, LoadedPam, load_pam_document};
pub use pam_viewer_core::parse_image_file_name;
pub use text::{TextFormat, decode_text, encode_text};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("no PAM, JSON, YAML, TOML, FLA, or XFL animation was found")]
    AnimationNotFound,
    #[error("input is not valid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("PAM core error: {0}")]
    Core(#[from] pam_viewer_core::CoreError),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("TOML encode error: {0}")]
    TomlEncode(#[from] toml::ser::Error),
    #[error("image decode error: {0}")]
    Image(#[from] image::ImageError),
    #[error("PNG encode error: {0}")]
    Png(#[from] png::EncodingError),
    #[error("WebP encode error: {0}")]
    Webp(#[from] image_webp::EncodingError),
    #[error("invalid RGBA frame size: expected {expected} bytes, received {actual}")]
    InvalidFrameSize { expected: usize, actual: usize },
    #[error("animation requires at least one frame")]
    NoFrames,
    #[error("export was cancelled")]
    Cancelled,
    #[error("FLA error: {0}")]
    Fla(String),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FormatError>;
