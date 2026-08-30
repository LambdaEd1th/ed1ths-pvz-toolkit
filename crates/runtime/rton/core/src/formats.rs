use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextFormat {
    Json,
    Yaml,
    Toml,
}

impl TextFormat {
    pub fn extension(self) -> &'static str {
        match self {
            TextFormat::Json => "json",
            TextFormat::Yaml => "yaml",
            TextFormat::Toml => "toml",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TextFormat::Json => "JSON",
            TextFormat::Yaml => "YAML",
            TextFormat::Toml => "TOML",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceFormat {
    Rton,
    Json,
    Yaml,
    Toml,
    Unknown,
}

impl SourceFormat {
    pub fn from_file_name(name: &str) -> Self {
        let Some(extension) = name.rsplit('.').next() else {
            return SourceFormat::Unknown;
        };

        match extension.to_ascii_lowercase().as_str() {
            "rton" => SourceFormat::Rton,
            "json" => SourceFormat::Json,
            "yaml" | "yml" => SourceFormat::Yaml,
            "toml" => SourceFormat::Toml,
            _ => SourceFormat::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BinaryEncoding {
    #[default]
    Standard,
    Compact,
}

impl BinaryEncoding {
    pub fn label(self) -> &'static str {
        match self {
            BinaryEncoding::Standard => "Standard RTON",
            BinaryEncoding::Compact => "Compact RTON",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodeOptions {
    pub encoding: BinaryEncoding,
    pub encrypted: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            encoding: BinaryEncoding::Standard,
            encrypted: false,
        }
    }
}
