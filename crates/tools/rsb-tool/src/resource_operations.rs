use crate::domain::ArchiveDocument;
use crate::editing::{PacketEdits, RemovedPackets};
use crate::preview::PreviewAsset;
use crate::resource_actions::{self, ResourceReader, ResourceZip};
use crate::resources::ResourceCatalog;
use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::{ToolKind, ToolOpenHandler};

#[derive(Clone)]
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
    let mut zoom = use_signal(|| "fit".to_string());
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
                                    zoom.set("fit".into());
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
                        if cancelled() { return Ok("已取消导出".into()); }
                        let name = file.name.rsplit('/').next().unwrap_or(&file.name);
                        return crate::platform::save_bytes(name, &file.bytes).await.map(|saved| if saved { format!("已导出 {name}") } else { "已取消保存".into() });
                    }
                    let mut zip = ResourceZip::new();
                    let mut errors = Vec::new();
                    for (position, index) in indices.iter().enumerate() {
                        if cancelled() { return Ok("已取消导出，未保存文件".into()); }
                        let row = &catalog.rows[*index];
                        message.set(format!("正在导出 {}/{} · {}", position + 1, indices.len(), row.entry.id));
                        match reader.extract(row).await {
                            Ok(file) => zip.add(&file.name, &file.bytes)?,
                            Err(error) => errors.push(format!("{} [{}]：{error}", resource_actions::output_path(row), row.entry.subgroup)),
                        }
                        #[cfg(target_arch = "wasm32")]
                        gloo_timers::future::TimeoutFuture::new(0).await;
                    }
                    if cancelled() { return Ok("已取消导出，未保存文件".into()); }
                    let count = zip.count;
                    warnings.set(errors.clone());
                    if count == 0 { return Err(format!("没有可导出的资源（{} 项未映射或读取失败）", errors.len())); }
                    if !errors.is_empty() { zip.add("_export-errors.txt", errors.join("\n").as_bytes())?; }
                    let bytes = zip.finish()?;
                    let saved = crate::platform::save_bytes(&zip_name, &bytes).await?;
                    Ok(if saved { format!("已导出 {count} 个资源 · 跳过 {} 项；ZIP 保留逻辑目录，同名变体自动编号", errors.len()) } else { "已取消保存".into() })
                }.await;
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
            div { class: "rsb-res-image-dialog", role: "dialog", aria_modal: true, aria_label: "资源图片预览",
                tabindex: -1,
                onmounted: move |event| async move { let _ = event.set_focus(true).await; },
                onkeydown: move |event| { if event.key() == Key::Escape { preview.set(None); } },
                header {
                    div { strong { "{image.name}" } small { "{image.width} × {image.height} · PNG · 保留透明度" } }
                    select { aria_label: "预览缩放", value: zoom(), onchange: move |event| zoom.set(event.value()),
                        option { value: "fit", "适应窗口" } option { value: "1", "100%" } option { value: "2", "200%" } option { value: "4", "400%" }
                    }
                    button { class: "rsb-tool-button", onclick: { let image = image.clone(); move |_| {
                        let image = image.clone(); spawn(async move {
                            let name = image.name.rsplit('/').next().unwrap_or(&image.name);
                            let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name);
                            if let Err(error) = crate::platform::save_bytes(&format!("{stem}.png"), &image.png).await { message.set(error); }
                        });
                    } }, "导出 PNG" }
                    button { class: "rsb-tool-button", onclick: move |_| preview.set(None), "关闭预览" }
                }
                div { class: if zoom() == "fit" { "rsb-res-image-stage is-fit" } else { "rsb-res-image-stage" },
                    img { src: "{image.asset.url()}", alt: "{image.name}",
                        style: if zoom() == "fit" { String::new() } else { format!("width: {}px; height: {}px", image.width * zoom().parse::<u32>().unwrap_or(1), image.height * zoom().parse::<u32>().unwrap_or(1)) }
                    }
                }
            }
        }
    }
}
