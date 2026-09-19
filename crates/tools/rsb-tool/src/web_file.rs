//! Browser archives retain a File handle, not a copy of every packet in WASM memory.
//! All reads are bounded slices; only the small metadata prefix stays resident.
use crate::domain::{ArchiveDocument, PacketRecord};
use crate::loader::{ArchiveSource, read_archive_index};
use dioxus_html::FileData;
use rsb_archive::{Rsb, RsgHeader};
use std::{io::Cursor, sync::Arc};
use wasm_bindgen_futures::JsFuture;

const MAX_READ_BYTES: usize = 256 * 1024 * 1024;

pub fn check_read_size(length: usize) -> Result<(), String> {
    if length > MAX_READ_BYTES {
        return Err("此操作需要一次读取超过 256 MiB，已停止以避免浏览器内存耗尽；请使用桌面版处理。归档仍可按需浏览。".into());
    }
    Ok(())
}

pub fn browser_file(file: &FileData) -> Result<&web_sys::File, String> {
    file.inner()
        .downcast_ref::<web_sys::File>()
        .ok_or_else(|| "无法获取浏览器文件引用，请重新选择或拖入文件".into())
}

pub async fn read_range(file: &FileData, offset: u64, length: usize) -> Result<Vec<u8>, String> {
    check_read_size(length)?;
    let end = offset
        .checked_add(length as u64)
        .filter(|end| *end <= file.size())
        .ok_or_else(|| "归档条目超出文件范围".to_string())?;
    let blob = browser_file(file)?
        .slice_with_f64_and_f64(offset as f64, end as f64)
        .map_err(|error| format!("无法读取归档区段：{error:?}"))?;
    let buffer = JsFuture::from(blob.array_buffer())
        .await
        .map_err(|error| format!("读取归档区段失败：{error:?}"))?;
    let array = js_sys::Uint8Array::new(&buffer);
    if array.length() as usize != length {
        return Err("归档区段读取不完整".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| "浏览器内存不足，请关闭其他归档后重试".to_string())?;
    bytes.resize(length, 0);
    array.copy_to(&mut bytes);
    Ok(bytes)
}

pub async fn open(file: FileData) -> Result<ArchiveDocument, String> {
    let prefix = read_range(&file, 0, file.size().min(112) as usize).await?;
    let header = Rsb::open(Cursor::new(prefix))
        .map_err(|error| error.to_string())?
        .header;
    let length = crate::loader::browser_metadata_length(&header, file.size())?;
    let metadata = Arc::new(read_range(&file, 0, length).await?);
    let source = ArchiveSource::WebFile {
        file: file.clone(),
        metadata: metadata.clone(),
    };
    let (mut document, infos) = read_archive_index(
        Cursor::new(metadata.as_slice()),
        file.name(),
        file.size(),
        source,
    )?;
    let mut records = Vec::with_capacity(infos.len());
    for (index, info) in infos.into_iter().enumerate() {
        let result = if info.rsg_length < 80 {
            Err("RSG 包短于固定头部".to_string())
        } else if u64::from(info.rsg_offset) + u64::from(info.rsg_length) > file.size() {
            Err("RSG 包超出归档文件范围".to_string())
        } else {
            read_range(&file, u64::from(info.rsg_offset), 80)
                .await
                .and_then(|bytes| {
                    RsgHeader::read_from(&mut Cursor::new(bytes)).map_err(|error| error.to_string())
                })
        };
        let (header, error) = match result {
            Ok(header) => (Some(header), None),
            Err(error) => (None, Some(error)),
        };
        records.push(PacketRecord {
            index,
            info,
            header,
            error,
        });
    }
    document.packets = Arc::new(records);
    Ok(document)
}
