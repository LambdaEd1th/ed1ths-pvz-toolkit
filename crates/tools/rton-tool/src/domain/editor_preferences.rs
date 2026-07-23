use rton_editor_core::TextFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorMode {
    RtonHex,
    Json,
    Yaml,
    Toml,
}

impl EditorMode {
    pub(crate) fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_lowercase().as_str() {
            "rton" => Some(Self::RtonHex),
            "json" => Some(Self::Json),
            "yaml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            _ => None,
        }
    }

    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::RtonHex => "rton",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            EditorMode::RtonHex => "RTON",
            EditorMode::Json => "JSON",
            EditorMode::Yaml => "YAML",
            EditorMode::Toml => "TOML",
        }
    }

    pub(crate) fn text_format(self) -> Option<TextFormat> {
        match self {
            EditorMode::RtonHex => None,
            EditorMode::Json => Some(TextFormat::Json),
            EditorMode::Yaml => Some(TextFormat::Yaml),
            EditorMode::Toml => Some(TextFormat::Toml),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub(crate) fn shell_class(self) -> &'static str {
        match self {
            Self::System => "rton-app-shell font-sans system-theme",
            Self::Light => "rton-app-shell font-sans light-theme",
            Self::Dark => "rton-app-shell font-sans dark-theme",
        }
    }
}
