use crate::app::PreviewModal;
use crate::domain::ArchiveDocument;
use crate::editing::{PacketEdits, RemovedPackets};
use crate::preview::{PreviewAsset, PreviewImage};
use crate::resource_actions::{self, ResourceReader, ResourceZip};
use crate::resources::ResourceCatalog;
use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::{ToolKind, ToolOpenHandler};

#[derive(Clone, PartialEq)]
struct ImagePreview {
    asset: PreviewAsset,
    name: String,
    png: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

#[component]
pub fn ResourceOperations(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed: RemovedPackets,
    catalog: Arc<ResourceCatalog>,
    selected_indices: Vec<usize>,
    focused_index: Option<usize>,
    mut open_request: Signal<Option<usize>>,
    mut message: Signal<String>,
) -> Element {
    let handler = use_hook(try_consume_context::<ToolOpenHandler>);
    let mut busy = use_signal(|| false);
    let mut cancelled = use_signal(|| false);
    let mut preview = use_signal(|| None::<ImagePreview>);
    let mut open_as = use_signal(String::new);
    let mut warnings = use_signal(Vec::<String>::new);
    use_effect(use_reactive(
        &(archive.clone(), edits.clone(), removed.clone()),
        move |_| {
            preview.set(None);
            cancelled.set(true);
            open_request.set(None);
            open_as.set(String::new());
        },
    ));
    let perform_open = EventHandler::new({
        let archive = archive.clone();
        let edits = edits.clone();
        let removed = removed.clone();
        let catalog = catalog.clone();
        move |(index, explicit, image_only): (usize, Option<ToolKind>, bool)| {
            if busy() {
                return;
            }
            let Some(row) = catalog.rows.get(index).cloned() else {
                return;
            };
            if !resource_actions::can_extract(&row) {
                message.set(format!(
                    "不能打开：{} · {}",
                    row.state.label(),
                    row.explanation
                ));
                return;
            }
            let is_image =
                image_only || (explicit.is_none() && resource_actions::can_preview(&row));
            let kind = explicit.or_else(|| resource_actions::tool_kind(&row));
            if !is_image && (kind.is_none() || handler.is_none()) {
                message.set("未识别对应工具，请从“打开方式”选择，或导出原始文件。".into());
                return;
            }
            let mut reader = ResourceReader::new(archive.clone(), edits.clone(), removed.clone());
            let catalog = catalog.clone();
            busy.set(true);
            cancelled.set(false);
            warnings.set(Vec::new());
            message.set(if kind == Some(ToolKind::Pam) && !is_image {
                "正在读取 PAM 并收集引用的图集子图…".into()
            } else {
                "正在读取资源…".into()
            });
            spawn(async move {
                if is_image {
                    match reader.png(&row).await {
                        Ok(png) if !cancelled() => {
                            match image::load_from_memory(&png)
                                .map_err(|error| error.to_string())
                                .and_then(|pixels| {
                                    let asset =
                                        PreviewAsset::new(crate::platform::preview_url(&png)?);
                                    Ok(ImagePreview {
                                        asset,
                                        name: resource_actions::output_path(&row),
                                        width: pixels.width(),
                                        height: pixels.height(),
                                        png: Arc::new(png),
                                    })
                                }) {
                                Ok(image) => {
                                    message.set(format!(
                                        "预览：{} · {} × {}",
                                        image.name, image.width, image.height
                                    ));
                                    preview.set(Some(image));
                                }
                                Err(error) => message.set(format!("预览失败：{error}")),
                            }
                        }
                        Ok(_) => message.set("已取消".into()),
                        Err(error) => message.set(format!("预览失败：{error}")),
                    }
                } else if let (Some(kind), Some(handler)) = (kind, handler) {
                    let result = if kind == ToolKind::Pam {
                        reader
                            .pam_files(&row, &catalog, move || *cancelled.read())
                            .await
                    } else {
                        reader
                            .extract(&row)
                            .await
                            .map(|file| (vec![file], Vec::new()))
                    };
                    match result {
                        Ok((files, notes)) if !cancelled() => {
                            for note in &notes {
                                toolkit_ui::push_application_log("RSB", "WARN", "OPEN", note);
                            }
                            message.set(format!(
                                "已在 {} 中打开 · {} 个附带图片 · {} 条提示",
                                kind.label(),
                                files.len().saturating_sub(1),
                                notes.len()
                            ));
                            warnings.set(notes);
                            handler.0.call((kind, files));
                        }
                        Ok(_) => message.set("已取消".into()),
                        Err(error) => message.set(format!("打开失败：{error}")),
                    }
                }
                busy.set(false);
            });
        }
    });
    use_effect(move || {
        if busy() {
            return;
        }
        if let Some(index) = open_request() {
            open_request.set(None);
            perform_open.call((index, None, false));
        }
    });

    let export = EventHandler::new({
        let archive = archive.clone();
        let catalog = catalog.clone();
        let indices = selected_indices.clone();
        move |_| {
            if busy() || indices.is_empty() {
                return;
            }
            let mut indices = indices.clone();
            // Group by RSG and parent atlas, so each atlas is decoded once per batch.
            indices.sort_by(|a, b| {
                catalog.rows[*a]
                    .locations
                    .cmp(&catalog.rows[*b].locations)
                    .then(a.cmp(b))
            });
            let catalog = catalog.clone();
            let mut reader = ResourceReader::new(archive.clone(), edits.clone(), removed.clone());
            let zip_name = format!(
                "{}-resources.zip",
                archive
                    .display_name
                    .rsplit_once('.')
                    .map(|(name, _)| name)
                    .unwrap_or(&archive.display_name)
            );
            busy.set(true);
            cancelled.set(false);
            warnings.set(Vec::new());
            spawn(async move {
                let result: Result<String, String> = async {
                    if indices.len() == 1 {
                        message.set("正在导出所选资源…".into());
                        let file = reader.extract(&catalog.rows[indices[0]]).await?;
                        if cancelled() {
                            return Ok("已取消导出".into());
                        }
                        let name = file.name.rsplit('/').next().unwrap_or(&file.name);
                        return crate::platform::save_bytes(name, &file.bytes)
                            .await
                            .map(|saved| {
                                if saved {
                                    format!("已导出 {name}")
                                } else {
                                    "已取消保存".into()
                                }
                            });
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        export_zip_native(
                            &mut reader,
                            &catalog,
                            &indices,
                            &zip_name,
                            message,
                            cancelled,
                            warnings,
                        )
                        .await
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        export_zip_web(
                            &mut reader,
                            &catalog,
                            &indices,
                            &zip_name,
                            message,
                            cancelled,
                            warnings,
                        )
                        .await
                    }
                }
                .await;
                message.set(result.unwrap_or_else(|error| format!("导出失败：{error}")));
                busy.set(false);
            });
        }
    });
    let current = focused_index.and_then(|index| catalog.rows.get(index));
    let can_open = current.is_some_and(resource_actions::can_extract);
    let can_preview = current.is_some_and(resource_actions::can_preview);
    let current_tool = current.and_then(resource_actions::tool_kind);
    rsx! {
        button { class: "rsb-tool-button", disabled: busy() || selected_indices.is_empty(), onclick: move |_| export.call(()), "导出所选 ({selected_indices.len()})" }
        button { class: "rsb-tool-button", disabled: busy() || !can_preview,
            onclick: move |_| { if let Some(index) = focused_index { perform_open.call((index, None, true)); } }, "预览图片" }
        select { class: "rsb-res-open-as", aria_label: "打开方式", value: open_as(), disabled: busy() || !can_open,
            onchange: move |event| open_as.set(event.value()),
            option { value: "", if let Some(kind) = current_tool { "自动 · {kind.label()}" } else { "打开方式…" } }
            for kind in ToolKind::ALL { option { value: kind.label(), "{kind.label()}" } }
        }
        button { class: "rsb-tool-button", disabled: busy() || !can_open || handler.is_none(),
            onclick: move |_| { if let Some(index) = focused_index {
                let explicit = ToolKind::ALL.into_iter().find(|kind| kind.label() == open_as());
                perform_open.call((index, explicit, false));
            } }, "打开" }
        if busy() { button { class: "rsb-tool-button", onclick: move |_| { cancelled.set(true); message.set("正在取消，等待当前解码结束…".into()); }, "取消任务" } }
        span { class: "rsb-res-selection-hint", title: "选中文件夹时递归包含所有子目录资源；批量导出保存为 ZIP", "含子目录 · Ctrl / ⌘ 多选" }
        if !warnings().is_empty() {
            details { class: "rsb-res-operation-notes", summary { "{warnings.read().len()} 条操作提示" }
                for warning in warnings().iter().take(200) { p { "{warning}" } }
                if warnings.read().len() > 200 { p { "仅显示前 200 条；完整导出错误见 ZIP 内 _export-errors.txt。" } }
            }
        }
        if let Some(image) = preview() {
            ResourceImagePreview {
                image,
                on_close: move |_| preview.set(None),
                on_error: move |error| message.set(error),
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
const EXPORT_CANCELLED: &str = "已取消导出，未保存文件";

#[cfg(not(target_arch = "wasm32"))]
async fn export_zip_native(
    reader: &mut ResourceReader,
    catalog: &ResourceCatalog,
    indices: &[usize],
    zip_name: &str,
    mut message: Signal<String>,
    cancelled: Signal<bool>,
    mut warnings: Signal<Vec<String>>,
) -> Result<String, String> {
    let Some(path) = crate::platform::pick_export_path(zip_name).await else {
        return Ok("已取消保存".into());
    };
    let (count, errors) = match export_zip_native_to_path(
        reader,
        catalog,
        indices,
        &path,
        |position, total, id| {
            message.set(format!("正在导出 {position}/{total} · {id}"));
        },
        || (*cancelled)(),
    )
    .await
    {
        Ok(result) => result,
        Err(error) if error == EXPORT_CANCELLED => return Ok(error),
        Err(error) => return Err(error),
    };
    warnings.set(errors.clone());
    Ok(format!(
        "已导出 {count} 个资源 · 跳过 {} 项；ZIP 保留逻辑目录，同名变体自动编号",
        errors.len()
    ))
}

#[cfg(not(target_arch = "wasm32"))]
async fn export_zip_native_to_path(
    reader: &mut ResourceReader,
    catalog: &ResourceCatalog,
    indices: &[usize],
    path: &std::path::Path,
    mut progress: impl FnMut(usize, usize, &str),
    cancelled: impl Fn() -> bool,
) -> Result<(usize, Vec<String>), String> {
    use std::io::BufWriter;

    let (file, temporary) = crate::platform::create_atomic_temp(path)?;
    let mut zip = ResourceZip::new(BufWriter::new(file));
    let mut errors = Vec::new();
    for (position, index) in indices.iter().enumerate() {
        if cancelled() {
            let _ = std::fs::remove_file(&temporary);
            return Err(EXPORT_CANCELLED.into());
        }
        let row = &catalog.rows[*index];
        progress(position + 1, indices.len(), &row.entry.id);
        match reader.extract(row).await {
            Ok(file) => {
                if let Err(error) = zip.add(&file.name, &file.bytes) {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(error);
                }
            }
            Err(error) => errors.push(format!(
                "{} [{}]：{error}",
                resource_actions::output_path(row),
                row.entry.subgroup
            )),
        }
        if position % 64 == 63 {
            futures_timer::Delay::new(std::time::Duration::ZERO).await;
        }
    }
    let count = zip.count;
    if count == 0 {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!(
            "没有可导出的资源（{} 项未映射或读取失败）",
            errors.len()
        ));
    }
    if !errors.is_empty()
        && let Err(error) = zip.add("_export-errors.txt", errors.join("\n").as_bytes())
    {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    let writer = match zip.finish() {
        Ok(writer) => writer,
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if let Err(error) = crate::platform::finish_atomic_temp(&temporary, path, writer) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok((count, errors))
}

#[cfg(target_arch = "wasm32")]
async fn export_zip_web(
    reader: &mut ResourceReader,
    catalog: &ResourceCatalog,
    indices: &[usize],
    zip_name: &str,
    mut message: Signal<String>,
    cancelled: Signal<bool>,
    mut warnings: Signal<Vec<String>>,
) -> Result<String, String> {
    let mut zip = ResourceZip::memory();
    let mut errors = Vec::new();
    let mut part = 1usize;
    let mut exported = 0usize;
    let mut parts = 0usize;
    for (position, index) in indices.iter().enumerate() {
        if cancelled() {
            warnings.set(errors);
            return Ok(if parts == 0 {
                "已取消导出，未保存文件".into()
            } else {
                format!("已取消保存，已导出前 {parts} 个分卷")
            });
        }
        let row = &catalog.rows[*index];
        message.set(format!(
            "正在导出 {}/{} · {}",
            position + 1,
            indices.len(),
            row.entry.id
        ));
        let file = match reader.extract(row).await {
            Ok(file) => file,
            Err(error) => {
                errors.push(format!(
                    "{} [{}]：{error}",
                    resource_actions::output_path(row),
                    row.entry.subgroup
                ));
                continue;
            }
        };
        let file_size = file.bytes.len() as u64;
        if file_size > resource_actions::MAX_EXPORT_BYTES as u64 {
            errors.push(format!(
                "{}：单个文件超过 512 MiB，已跳过",
                resource_actions::output_path(row)
            ));
            continue;
        }
        if zip.count > 0
            && zip.total_bytes.saturating_add(file_size) > resource_actions::MAX_EXPORT_BYTES as u64
        {
            let count = zip.count;
            let bytes = zip.into_bytes()?;
            let name = web_part_name(zip_name, part);
            if !crate::platform::save_bytes(&name, &bytes).await? {
                warnings.set(errors);
                return Ok(format!("已取消保存，已导出前 {parts} 个分卷"));
            }
            exported += count;
            parts += 1;
            part += 1;
            zip = ResourceZip::memory();
        }
        if let Err(error) = zip.add(&file.name, &file.bytes) {
            warnings.set(errors);
            return Err(error);
        }
        gloo_timers::future::TimeoutFuture::new(0).await;
    }
    if zip.count == 0 {
        warnings.set(errors.clone());
        return Err(format!(
            "没有可导出的资源（{} 项未映射或读取失败）",
            errors.len()
        ));
    }
    if !errors.is_empty() {
        zip.add("_export-errors.txt", errors.join("\n").as_bytes())?;
    }
    let count = zip.count;
    let bytes = zip.into_bytes()?;
    let name = web_part_name(zip_name, part);
    if !crate::platform::save_bytes(&name, &bytes).await? {
        warnings.set(errors);
        return Ok(format!("已取消保存，已导出前 {parts} 个分卷"));
    }
    exported += count;
    parts += 1;
    warnings.set(errors.clone());
    Ok(format!(
        "已导出 {exported} 个资源 · 跳过 {} 项 · 共 {parts} 个 ZIP；ZIP 保留逻辑目录，同名变体自动编号",
        errors.len()
    ))
}

#[cfg(target_arch = "wasm32")]
fn web_part_name(base: &str, part: usize) -> String {
    if part <= 1 {
        return base.to_string();
    }
    let stem = base.strip_suffix(".zip").unwrap_or(base);
    format!("{stem}-part{part:02}.zip")
}

#[component]
fn ResourceImagePreview(
    image: ImagePreview,
    on_close: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let modal_image = PreviewImage::from_png(
        image.name.clone(),
        image.asset.clone(),
        image.width,
        image.height,
        image.png.len(),
    );
    let export_name = image.name.clone();
    let export_png = Arc::clone(&image.png);
    rsx! {
        PreviewModal {
            id: "rsb-resource-preview-modal".to_string(),
            image: modal_image,
            on_close,
            on_export: move |_| {
                let name = export_name.clone();
                let png = Arc::clone(&export_png);
                spawn(async move {
                    let file_name = name.rsplit('/').next().unwrap_or(&name);
                    let stem = file_name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file_name);
                    if let Err(error) = crate::platform::save_bytes(&format!("{stem}.png"), &png).await {
                        on_error.call(error);
                    }
                });
            },
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::editing::{PacketEdits, RemovedPackets};
    use crate::resources;
    use std::sync::Arc;

    #[test]
    fn native_export_streams_every_selected_resource_to_a_zip() {
        let archive = crate::edit_fixture::archive();
        let data = pollster::block_on(resources::load_manifests(
            archive.clone(),
            PacketEdits::new(),
            RemovedPackets::new(),
        ));
        let files =
            resources::physical_files(&archive, &PacketEdits::new(), &RemovedPackets::new());
        let catalog = resources::build_catalog(&data, &files);
        let indices = catalog
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| resource_actions::can_extract(row))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert!(!indices.is_empty());
        let path = std::env::temp_dir().join(format!(
            "rsb-native-export-{}-{}.zip",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let mut reader = ResourceReader::new(
            Arc::clone(&archive),
            PacketEdits::new(),
            RemovedPackets::new(),
        );
        let (count, errors) = pollster::block_on(export_zip_native_to_path(
            &mut reader,
            &catalog,
            &indices,
            &path,
            |_, _, _| {},
            || false,
        ))
        .unwrap();
        assert!(count > 0);
        assert_eq!(count, indices.len() - errors.len());
        let archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(archive.len(), count + usize::from(!errors.is_empty()));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn native_export_cancellation_leaves_no_partial_file() {
        let archive = crate::edit_fixture::archive();
        let data = pollster::block_on(resources::load_manifests(
            archive.clone(),
            PacketEdits::new(),
            RemovedPackets::new(),
        ));
        let files =
            resources::physical_files(&archive, &PacketEdits::new(), &RemovedPackets::new());
        let catalog = resources::build_catalog(&data, &files);
        let indices = catalog
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| resource_actions::can_extract(row))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let path = std::env::temp_dir().join(format!(
            "rsb-cancel-export-{}-{}.zip",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let mut reader = ResourceReader::new(
            Arc::clone(&archive),
            PacketEdits::new(),
            RemovedPackets::new(),
        );
        let error = pollster::block_on(export_zip_native_to_path(
            &mut reader,
            &catalog,
            &indices,
            &path,
            |_, _, _| {},
            || true,
        ))
        .unwrap_err();
        assert_eq!(error, EXPORT_CANCELLED);
        assert!(!path.exists());
    }
}
