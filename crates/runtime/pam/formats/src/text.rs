use pam_viewer_core::PamInfo;

use crate::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFormat {
    Json,
    Yaml,
    Toml,
}

pub fn decode_text(text: &str, format: TextFormat) -> Result<PamInfo> {
    match format {
        TextFormat::Json => Ok(pam_viewer_core::decode_json(text)?),
        TextFormat::Yaml => Ok(serde_yaml::from_str(text)?),
        TextFormat::Toml => Ok(toml::from_str(text)?),
    }
}

pub fn encode_text(pam: &PamInfo, format: TextFormat) -> Result<String> {
    let mut text = match format {
        TextFormat::Json => return Ok(pam_viewer_core::encode_json(pam)?),
        TextFormat::Yaml => serde_yaml::to_string(pam)?,
        TextFormat::Toml => toml::to_string_pretty(pam)?,
    };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}
