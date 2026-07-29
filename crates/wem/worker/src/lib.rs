use ogg::PacketReader;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;
use thiserror::Error;
use wem_audio::{
    AacWemEncoder, AdpcmWemEncoder, CodebookLibrary, PcmWemEncoder, VorbisOptions,
    VorbisWemDecoder, VorbisWemEncoder, WemCodec, WemDecodeOptions, decode_wem_to_wav,
    extract_wem_aac, inspect_wem, probe_aac_bytes,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Wem,
    Wav,
    Ogg,
    Aac,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportTarget {
    Automatic,
    Wav,
    Ogg,
    Aac,
    PcmWem,
    AdpcmWem,
    VorbisWem,
    AacWem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioInfo {
    pub format: SourceFormat,
    pub codec: String,
    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrepareRequest {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedAudio {
    pub info: AudioInfo,
    #[serde(with = "serde_bytes")]
    pub playback: Vec<u8>,
    pub playback_mime: String,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub target: ExportTarget,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertedAudio {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub mime: String,
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("无法识别该音频格式")]
    UnknownFormat,
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    Codec(String),
}

pub type Result<T> = std::result::Result<T, WorkerError>;

pub fn prepare(request: PrepareRequest) -> Result<PreparedAudio> {
    let format = detect_format(&request.name, &request.data)?;
    match format {
        SourceFormat::Wem => prepare_wem(request.data),
        SourceFormat::Wav => {
            let info = inspect_wav(&request.data)?;
            Ok(PreparedAudio {
                info,
                playback: request.data,
                playback_mime: "audio/wav".to_string(),
                warning: None,
            })
        }
        SourceFormat::Ogg => {
            let info = inspect_ogg(&request.data)?;
            Ok(PreparedAudio {
                info,
                playback: request.data,
                playback_mime: "audio/ogg".to_string(),
                warning: None,
            })
        }
        SourceFormat::Aac => {
            let extension = extension(&request.name);
            let metadata = probe_aac_bytes(request.data.clone(), extension).ok();
            Ok(PreparedAudio {
                info: AudioInfo {
                    format,
                    codec: "AAC".to_string(),
                    channels: metadata.map(|metadata| metadata.channels),
                    sample_rate: metadata.map(|metadata| metadata.sample_rate),
                    byte_size: request.data.len() as u64,
                },
                playback: request.data,
                playback_mime: aac_mime(&request.name).to_string(),
                warning: metadata
                    .is_none()
                    .then(|| "可以播放该文件，但暂时无法读取编码参数或转换为 WEM。".to_string()),
            })
        }
    }
}

pub fn convert(request: ConvertRequest) -> Result<ConvertedAudio> {
    let source = detect_format(&request.name, &request.data)?;
    let target = resolve_target(source, request.target, &request.data)?;
    let stem = file_stem(&request.name);
    let (name, mime, data) = match target {
        ExportTarget::Wav if source == SourceFormat::Wem => (
            format!("{stem}.wav"),
            "audio/wav",
            decode_wem_wav(&request.data)?,
        ),
        ExportTarget::Ogg if source == SourceFormat::Wem => (
            format!("{stem}.ogg"),
            "audio/ogg",
            decode_wem_ogg(&request.data)?,
        ),
        ExportTarget::Aac if source == SourceFormat::Wem => {
            let mut output = Vec::new();
            extract_wem_aac(Cursor::new(&request.data), &mut output)
                .map_err(codec_error("AAC 提取失败"))?;
            (format!("{stem}.m4a"), "audio/mp4", output)
        }
        ExportTarget::PcmWem if source == SourceFormat::Wav => {
            let mut output = Vec::new();
            PcmWemEncoder::new(Cursor::new(&request.data))
                .map_err(codec_error("WAV 读取失败"))?
                .encode(&mut output)
                .map_err(codec_error("PCM WEM 编码失败"))?;
            (format!("{stem}.wem"), "application/octet-stream", output)
        }
        ExportTarget::AdpcmWem if source == SourceFormat::Wav => {
            let mut output = Vec::new();
            AdpcmWemEncoder::new(Cursor::new(&request.data))
                .map_err(codec_error("WAV 读取失败"))?
                .encode(&mut output)
                .map_err(codec_error("ADPCM WEM 编码失败"))?;
            (format!("{stem}.wem"), "application/octet-stream", output)
        }
        ExportTarget::VorbisWem if source == SourceFormat::Ogg => {
            let data = VorbisWemEncoder::new(Cursor::new(&request.data))
                .encode_to_vec()
                .map_err(codec_error("Vorbis WEM 编码失败"))?;
            (format!("{stem}.wem"), "application/octet-stream", data)
        }
        ExportTarget::AacWem if source == SourceFormat::Aac => {
            let metadata = probe_aac_bytes(request.data.clone(), extension(&request.name))
                .map_err(codec_error("AAC 参数读取失败"))?;
            let mut output = Vec::new();
            AacWemEncoder::new(Cursor::new(&request.data), metadata)
                .map_err(codec_error("AAC WEM 初始化失败"))?
                .encode(&mut output)
                .map_err(codec_error("AAC WEM 编码失败"))?;
            (format!("{stem}.wem"), "application/octet-stream", output)
        }
        _ => {
            return Err(WorkerError::Unsupported(
                "源格式与所选输出格式不兼容".to_string(),
            ));
        }
    };
    Ok(ConvertedAudio {
        name,
        data,
        mime: mime.to_string(),
    })
}

fn prepare_wem(data: Vec<u8>) -> Result<PreparedAudio> {
    let metadata = inspect_wem(&mut Cursor::new(&data)).map_err(codec_error("WEM 读取失败"))?;
    let codec = codec_name(metadata.codec).to_string();
    match decode_wem_wav(&data) {
        Ok(playback) => Ok(PreparedAudio {
            info: AudioInfo {
                format: SourceFormat::Wem,
                codec,
                channels: Some(metadata.channels),
                sample_rate: Some(metadata.sample_rate),
                byte_size: data.len() as u64,
            },
            playback,
            playback_mime: "audio/wav".to_string(),
            warning: None,
        }),
        Err(error) if metadata.codec == WemCodec::Aac => {
            let mut playback = Vec::new();
            extract_wem_aac(Cursor::new(&data), &mut playback)
                .map_err(codec_error("AAC WEM 播放数据提取失败"))?;
            Ok(PreparedAudio {
                info: AudioInfo {
                    format: SourceFormat::Wem,
                    codec,
                    channels: Some(metadata.channels),
                    sample_rate: Some(metadata.sample_rate),
                    byte_size: data.len() as u64,
                },
                playback,
                playback_mime: "audio/mp4".to_string(),
                warning: Some(format!("AAC 解码不可用，播放器将直接读取原始音频：{error}")),
            })
        }
        Err(error) => Err(error),
    }
}

fn decode_wem_wav(data: &[u8]) -> Result<Vec<u8>> {
    let source = Arc::<[u8]>::from(data);
    let mut first_error = None;
    for codebooks in [CodebookLibrary::aotuv_603(), CodebookLibrary::standard()] {
        let mut output = Cursor::new(Vec::new());
        let options = WemDecodeOptions::new().with_vorbis_codebooks(codebooks);
        match decode_wem_to_wav(Cursor::new(Arc::clone(&source)), &mut output, &options) {
            Ok(()) => return Ok(output.into_inner()),
            Err(error) => first_error.get_or_insert(error.to_string()),
        };
    }
    Err(WorkerError::Codec(
        first_error.unwrap_or_else(|| "WEM 解码失败".to_string()),
    ))
}

fn decode_wem_ogg(data: &[u8]) -> Result<Vec<u8>> {
    let metadata = inspect_wem(&mut Cursor::new(data)).map_err(codec_error("WEM 读取失败"))?;
    if metadata.codec != WemCodec::Vorbis {
        return Err(WorkerError::Unsupported(
            "只有 Vorbis WEM 可以导出 Ogg".to_string(),
        ));
    }
    let mut first_error = None;
    for codebooks in [CodebookLibrary::aotuv_603(), CodebookLibrary::standard()] {
        let options = VorbisOptions::new().with_codebooks(codebooks);
        match VorbisWemDecoder::with_options(Cursor::new(data), options) {
            Ok(mut decoder) => {
                let mut output = Vec::new();
                match decoder.decode_to_ogg(&mut output) {
                    Ok(()) => return Ok(output),
                    Err(error) => first_error.get_or_insert(error.to_string()),
                };
            }
            Err(error) => {
                first_error.get_or_insert(error.to_string());
            }
        }
    }
    Err(WorkerError::Codec(
        first_error.unwrap_or_else(|| "Vorbis WEM 解码失败".to_string()),
    ))
}

fn inspect_wav(data: &[u8]) -> Result<AudioInfo> {
    let reader = hound::WavReader::new(Cursor::new(data))
        .map_err(|error| WorkerError::Codec(format!("WAV 读取失败：{error}")))?;
    let spec = reader.spec();
    Ok(AudioInfo {
        format: SourceFormat::Wav,
        codec: format!("PCM {}-bit", spec.bits_per_sample),
        channels: Some(spec.channels),
        sample_rate: Some(spec.sample_rate),
        byte_size: data.len() as u64,
    })
}

fn inspect_ogg(data: &[u8]) -> Result<AudioInfo> {
    let mut packets = PacketReader::new(Cursor::new(data));
    let packet = packets
        .read_packet()
        .map_err(|error| WorkerError::Codec(format!("Ogg 读取失败：{error:?}")))?
        .ok_or_else(|| WorkerError::Codec("Ogg 文件没有音频包".to_string()))?;
    if packet.data.len() < 16 || !packet.data.starts_with(b"\x01vorbis") {
        return Err(WorkerError::Unsupported(
            "当前仅支持 Ogg Vorbis 输入".to_string(),
        ));
    }
    let channels = u16::from(packet.data[11]);
    let sample_rate = u32::from_le_bytes([
        packet.data[12],
        packet.data[13],
        packet.data[14],
        packet.data[15],
    ]);
    Ok(AudioInfo {
        format: SourceFormat::Ogg,
        codec: "Vorbis".to_string(),
        channels: Some(channels),
        sample_rate: Some(sample_rate),
        byte_size: data.len() as u64,
    })
}

fn detect_format(name: &str, data: &[u8]) -> Result<SourceFormat> {
    let extension = extension(name).unwrap_or_default();
    if extension.eq_ignore_ascii_case("wem") {
        return Ok(SourceFormat::Wem);
    }
    if extension.eq_ignore_ascii_case("wav") {
        return Ok(SourceFormat::Wav);
    }
    if matches!(extension.to_ascii_lowercase().as_str(), "ogg" | "oga") {
        return Ok(SourceFormat::Ogg);
    }
    if matches!(
        extension.to_ascii_lowercase().as_str(),
        "aac" | "m4a" | "mp4"
    ) {
        return Ok(SourceFormat::Aac);
    }
    if data.starts_with(b"OggS") {
        return Ok(SourceFormat::Ogg);
    }
    if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WAVE") {
        let tag = data
            .get(20..22)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
        return Ok(match tag {
            Some(0xFFFF | 0x8311 | 0xAAC0) => SourceFormat::Wem,
            _ => SourceFormat::Wav,
        });
    }
    if data.get(4..8) == Some(b"ftyp")
        || data.first().copied() == Some(0xFF)
            && data.get(1).is_some_and(|value| value & 0xF0 == 0xF0)
    {
        return Ok(SourceFormat::Aac);
    }
    Err(WorkerError::UnknownFormat)
}

fn resolve_target(source: SourceFormat, target: ExportTarget, data: &[u8]) -> Result<ExportTarget> {
    if target != ExportTarget::Automatic {
        return Ok(target);
    }
    match source {
        SourceFormat::Wem => {
            let metadata =
                inspect_wem(&mut Cursor::new(data)).map_err(codec_error("WEM 读取失败"))?;
            Ok(match metadata.codec {
                WemCodec::Vorbis => ExportTarget::Ogg,
                WemCodec::Aac => ExportTarget::Aac,
                WemCodec::Pcm | WemCodec::ExtensiblePcm | WemCodec::ImaAdpcm => ExportTarget::Wav,
                WemCodec::Unknown(tag) => {
                    return Err(WorkerError::Unsupported(format!(
                        "不支持 WEM 编码 0x{tag:04X}"
                    )));
                }
                _ => {
                    return Err(WorkerError::Unsupported("不支持该 WEM 编码".to_string()));
                }
            })
        }
        SourceFormat::Wav => Ok(ExportTarget::PcmWem),
        SourceFormat::Ogg => Ok(ExportTarget::VorbisWem),
        SourceFormat::Aac => Ok(ExportTarget::AacWem),
    }
}

fn codec_name(codec: WemCodec) -> &'static str {
    match codec {
        WemCodec::Pcm | WemCodec::ExtensiblePcm => "PCM",
        WemCodec::Vorbis => "Wwise Vorbis",
        WemCodec::ImaAdpcm => "Wwise IMA ADPCM",
        WemCodec::Aac => "AAC",
        WemCodec::Unknown(_) => "Unknown",
        _ => "Unknown",
    }
}

fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_, extension)| extension)
}

fn file_stem(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(stem, _)| stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or("audio")
        .to_string()
}

fn aac_mime(name: &str) -> &'static str {
    if extension(name).is_some_and(|extension| extension.eq_ignore_ascii_case("aac")) {
        "audio/aac"
    } else {
        "audio/mp4"
    }
}

fn codec_error(context: &'static str) -> impl FnOnce(wem_audio::WemError) -> WorkerError + Clone {
    move |error| WorkerError::Codec(format!("{context}：{error}"))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn prepare_audio(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<PrepareRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        prepare(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn convert_audio(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<ConvertRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        convert(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_wav() -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut output,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 22_050,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for sample in 0_i16..128 {
                writer.write_sample(sample.saturating_mul(64)).unwrap();
            }
            writer.finalize().unwrap();
        }
        output.into_inner()
    }

    #[test]
    fn wav_pcm_wem_wav_flow_is_playable() {
        let source = source_wav();
        let wem = convert(ConvertRequest {
            name: "sample.wav".to_string(),
            data: source,
            target: ExportTarget::PcmWem,
        })
        .unwrap();
        assert_eq!(wem.name, "sample.wem");
        let prepared = prepare(PrepareRequest {
            name: wem.name,
            data: wem.data,
        })
        .unwrap();
        assert_eq!(prepared.info.format, SourceFormat::Wem);
        assert_eq!(prepared.info.sample_rate, Some(22_050));
        assert!(prepared.playback.starts_with(b"RIFF"));
    }

    #[test]
    fn rejects_incompatible_output_target() {
        let error = convert(ConvertRequest {
            name: "sample.wav".to_string(),
            data: source_wav(),
            target: ExportTarget::VorbisWem,
        })
        .unwrap_err();
        assert!(matches!(error, WorkerError::Unsupported(_)));
    }

    #[test]
    fn prepares_and_converts_real_pvz_wem_when_available() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../pvz2-toolkit/test_data/test_output/bnk_test/310646500.wem");
        let Ok(data) = std::fs::read(&path) else {
            eprintln!("skipping missing real WEM sample: {}", path.display());
            return;
        };
        let prepared = prepare(PrepareRequest {
            name: "310646500.wem".to_string(),
            data: data.clone(),
        })
        .expect("real PvZ WEM should be playable");
        assert_eq!(prepared.info.format, SourceFormat::Wem);
        assert!(!prepared.playback.is_empty());

        let converted = convert(ConvertRequest {
            name: "310646500.wem".to_string(),
            data,
            target: ExportTarget::Automatic,
        })
        .expect("real PvZ WEM should support automatic export");
        assert!(
            converted.data.starts_with(b"OggS") || converted.data.starts_with(b"RIFF"),
            "unexpected automatic output {}",
            converted.name
        );
    }
}
