use compiled_text::{
    CompiledTextMetadata, DecodeOptions, EncodeOptions, SmfVariant, decode_detailed,
    encode_with_options, inspect,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderVariant {
    Compact32,
    Extended64,
}

impl HeaderVariant {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact32 => "32-bit / 8-byte",
            Self::Extended64 => "64-bit / 16-byte",
        }
    }
}

impl From<SmfVariant> for HeaderVariant {
    fn from(value: SmfVariant) -> Self {
        match value {
            SmfVariant::Compact32 => Self::Compact32,
            SmfVariant::Extended64 => Self::Extended64,
        }
    }
}

impl From<HeaderVariant> for SmfVariant {
    fn from(value: HeaderVariant) -> Self {
        match value {
            HeaderVariant::Compact32 => Self::Compact32,
            HeaderVariant::Extended64 => Self::Extended64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub variant: HeaderVariant,
    pub encoded_size: u64,
    pub ciphertext_size: u64,
    pub container_size: u64,
    pub decoded_size: u64,
}

impl From<CompiledTextMetadata> for DocumentMetadata {
    fn from(value: CompiledTextMetadata) -> Self {
        Self {
            variant: value.variant.into(),
            encoded_size: value.encoded_size,
            ciphertext_size: value.ciphertext_size,
            container_size: value.container_size,
            decoded_size: value.decoded_size,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodeRequest {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub seed: String,
    pub max_output_size: u64,
    pub allow_base64_whitespace: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodeResponse {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub metadata: DocumentMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncodeRequest {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub seed: String,
    pub variant: HeaderVariant,
    pub compression_level: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncodeResponse {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("Seed 不能为空")]
    EmptySeed,
    #[error("压缩级别必须在 0 到 9 之间")]
    InvalidCompressionLevel,
    #[error("Compiled Text 处理失败：{0}")]
    Codec(String),
}

pub type Result<T> = std::result::Result<T, WorkerError>;

pub fn decode(request: DecodeRequest) -> Result<DecodeResponse> {
    validate_seed(&request.seed)?;
    let decoded = decode_detailed(
        &request.data,
        &request.seed,
        DecodeOptions {
            max_output_size: request.max_output_size,
            allow_base64_whitespace: request.allow_base64_whitespace,
        },
    )
    .map_err(codec_error)?;
    Ok(DecodeResponse {
        data: decoded.data,
        metadata: decoded.metadata.into(),
    })
}

pub fn encode(request: EncodeRequest) -> Result<EncodeResponse> {
    validate_seed(&request.seed)?;
    if request.compression_level > 9 {
        return Err(WorkerError::InvalidCompressionLevel);
    }
    let options = EncodeOptions::new(request.variant.into(), request.compression_level);
    let data = encode_with_options(&request.data, &request.seed, options).map_err(codec_error)?;
    let metadata = inspect(&data, &request.seed, DecodeOptions::safe()).map_err(codec_error)?;
    Ok(EncodeResponse {
        data,
        metadata: metadata.into(),
    })
}

fn validate_seed(seed: &str) -> Result<()> {
    if seed.is_empty() {
        Err(WorkerError::EmptySeed)
    } else {
        Ok(())
    }
}

fn codec_error(error: compiled_text::CompiledTextError) -> WorkerError {
    WorkerError::Codec(error.to_string())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn decode_compiled_text(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<DecodeRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        decode(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn encode_compiled_text(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<EncodeRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        encode(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_and_extended_roundtrip() {
        let source = "compiled text 测试\n".repeat(1024).into_bytes();
        for variant in [HeaderVariant::Compact32, HeaderVariant::Extended64] {
            let encoded = encode(EncodeRequest {
                data: source.clone(),
                seed: "AS-23DSRFG-209JH0".to_string(),
                variant,
                compression_level: 6,
            })
            .unwrap();
            assert_eq!(encoded.metadata.variant, variant);
            let decoded = decode(DecodeRequest {
                data: encoded.data,
                seed: "AS-23DSRFG-209JH0".to_string(),
                max_output_size: 8 * 1024 * 1024,
                allow_base64_whitespace: true,
            })
            .unwrap();
            assert_eq!(decoded.data, source);
        }
    }

    #[test]
    fn rejects_invalid_parameters() {
        assert!(matches!(
            encode(EncodeRequest {
                data: Vec::new(),
                seed: String::new(),
                variant: HeaderVariant::Compact32,
                compression_level: 9,
            }),
            Err(WorkerError::EmptySeed)
        ));
    }
}
