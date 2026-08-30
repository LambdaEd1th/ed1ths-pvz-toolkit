use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{0}")]
    Rton(String),
    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Invalid TOML: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("Cannot write TOML: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("Invalid hex bytes: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("JSON does not support non-finite number: {0}")]
    NonFiniteJson(&'static str),
    #[error("Invalid value path: {0}")]
    InvalidPath(String),
    #[error("Worker request is invalid: {0}")]
    InvalidWorkerRequest(String),
    #[error("Value path not found: {0}")]
    PathNotFound(String),
    #[error("{0} values cannot be edited as scalars")]
    NonEditableValue(&'static str),
    #[error("Invalid {kind} value: {message}")]
    InvalidScalarValue { kind: &'static str, message: String },
}

pub type Result<T> = std::result::Result<T, CoreError>;
