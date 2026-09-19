use crate::{
    domain::ArchiveDocument,
    editing::{AddedFile, PacketEdits, RemovedPackets},
    resource_edit::{self, ResourceChange, ResourceCommit},
    resources::{AtlasRegion, MappedResource, MappingState, ResourceCatalog, ResourceEntry},
};
use dioxus::prelude::*;
use rsb_archive::RsbPtxInfo;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Edit,
    Add,
    Replace,
    Delete,
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Self::Edit => "编辑资源定义",
            Self::Add => "新增资源定义",
            Self::Replace => "替换资源内容",
            Self::Delete => "删除资源定义",
        }
    }
}

#[derive(Clone)]
struct Draft {
    mode: Mode,
    before: Option<ResourceEntry>,
    entry: ResourceEntry,
    coordinates: [String; 4],
}

impl Draft {
    fn new(mode: Mode, entry: ResourceEntry) -> Self {
        let region = entry.region.clone().unwrap_or_default();
        Self {
            mode,
            before: (mode != Mode::Add).then(|| entry.clone()),
            entry,
            coordinates: [region.x, region.y, region.width, region.height].map(|n| n.to_string()),
        }
    }

    fn change(&self, replacement: Option<AddedFile>) -> Result<ResourceChange, String> {
        if self.mode == Mode::Replace && replacement.is_none() {
            return Err("请先选择替换文件".into());
        }
        let mut entry = self.entry.clone();
        if matches!(self.mode, Mode::Edit | Mode::Add) {
            entry.region = if entry.parent.is_empty() {
                None
            } else {
                let values = self
                    .coordinates
                    .iter()
                    .map(|v| {
                        v.parse::<u32>()
                            .map_err(|_| "图集坐标必须是非负整数".to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Some(AtlasRegion {
                    x: values[0],
                    y: values[1],
                    width: values[2],
                    height: values[3],
                })
            };
        }
        Ok(ResourceChange {
            before: self.before.clone(),
            after: (self.mode != Mode::Delete).then_some(entry),
            replacement,
        })
    }
}

#[component]
pub fn ResourceEditor(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed: RemovedPackets,
    ptx_infos: Vec<RsbPtxInfo>,
    catalog: Arc<ResourceCatalog>,
    current: Option<MappedResource>,
    disabled: bool,
    on_commit: EventHandler<ResourceCommit>,
) -> Element {
    let mut draft = use_signal(|| None::<Draft>);
    let mut replacement = use_signal(|| None::<AddedFile>);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let can_define = current
        .as_ref()
        .is_some_and(|r| r.state != MappingState::Unlisted);
    let can_replace = current
        .as_ref()
        .is_some_and(crate::resource_actions::can_extract);
    let open = EventHandler::new({
        let catalog = catalog.clone();
        move |mode| {
            let mut entry = current
                .as_ref()
                .map(|r| r.entry.clone())
                .unwrap_or_default();
            if mode == Mode::Add {
                if entry.source.is_empty() || !can_define {
                    entry = catalog
                        .rows
                        .iter()
                        .find(|r| r.state != MappingState::Unlisted)
                        .map(|r| r.entry.clone())
                        .unwrap_or_default();
                }
                entry.id.clear();
                entry.path.clear();
                entry.parent.clear();
                entry.atlas = false;
                entry.region = None;
                entry.kind = "File".into();
            }
            replacement.set(None);
            error.set(String::new());
            draft.set(Some(Draft::new(mode, entry)));
        }
    });
    let submit = EventHandler::new(move |_| {
        if busy() {
            return;
        }
        let Some(form) = draft() else {
            return;
        };
        let change = match form.change(replacement()) {
            Ok(change) => change,
            Err(message) => {
                error.set(message);
                return;
            }
        };
        let archive = archive.clone();
        let edits = edits.clone();
        let removed = removed.clone();
        let catalog = catalog.clone();
        let ptx = ptx_infos.clone();
        busy.set(true);
        error.set(String::new());
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(0).await;
            match resource_edit::apply(archive, edits, removed, ptx, catalog, change).await {
                Ok(commit) => {
                    on_commit.call(commit);
                    draft.set(None);
                    replacement.set(None);
                }
                Err(message) => error.set(message),
            }
            busy.set(false);
        });
    });
    let file_summary = replacement
        .read()
        .as_ref()
        .map(|file| (file.name.clone(), file.data.len()));
    rsx! {
        button { class: "rsb-tool-button", disabled: disabled || busy() || !can_replace, onclick: move |_| open.call(Mode::Replace), "替换内容" }
        button { class: "rsb-tool-button", disabled: disabled || busy() || !can_define, onclick: move |_| open.call(Mode::Edit), "编辑定义" }
        button { class: "rsb-tool-button", disabled: disabled || busy(), onclick: move |_| open.call(Mode::Add), "新增定义" }
        button { class: "rsb-tool-button", disabled: disabled || busy() || !can_define, onclick: move |_| open.call(Mode::Delete), "删除定义" }
        if let Some(form) = draft() {
            dialog { id: "rsb-resource-edit-dialog", class: "rsb-dialog-layer rsb-resource-editor-layer", aria_modal: "true", aria_label: form.mode.title(),
                onmounted: move |_| { let _ = document::eval("document.getElementById('rsb-resource-edit-dialog')?.showModal()"); },
                oncancel: move |event| { event.prevent_default(); if !busy() { draft.set(None); } },
                div { class: "rsb-dialog-backdrop" }
                section { class: "rsb-dialog rsb-resource-editor",
                    header {
                        div { strong { "{form.mode.title()}" } span { if form.entry.id.is_empty() { "{form.entry.path}" } else { "{form.entry.id}" } } }
                        button { disabled: busy(), aria_label: "关闭资源编辑", onclick: move |_| draft.set(None), "×" }
                    }
                    div { class: "rsb-dialog-body",
                        if matches!(form.mode, Mode::Edit | Mode::Add) {
                            div { class: "rsb-resource-fields",
                                for (field, label, value) in [
                                    ("id", "资源 ID", &form.entry.id), ("path", "逻辑路径", &form.entry.path),
                                    ("kind", "资源类型", &form.entry.kind), ("group", "资源组", &form.entry.group),
                                    ("subgroup", "RSG 子组", &form.entry.subgroup), ("resolution", "分辨率（可留空）", &form.entry.resolution),
                                    ("language", "语言（可留空）", &form.entry.language), ("parent", "父图集 ID（可留空）", &form.entry.parent),
                                ] {
                                    label { "{label}"
                                        input { disabled: busy(), aria_label: label, value: "{value}",
                                            oninput: move |event| { if let Some(d) = draft.write().as_mut() { let value = event.value(); match field {
                                                "id" => d.entry.id = value, "path" => d.entry.path = value, "kind" => d.entry.kind = value,
                                                "group" => d.entry.group = value, "subgroup" => d.entry.subgroup = value,
                                                "resolution" => d.entry.resolution = value, "language" => d.entry.language = value, "parent" => d.entry.parent = value, _ => {}
                                            } } }
                                        }
                                    }
                                }
                                label { class: "rsb-resource-checkbox", input { r#type: "checkbox", disabled: busy(), checked: form.entry.atlas,
                                    onchange: move |event| { if let Some(d) = draft.write().as_mut() { d.entry.atlas = event.checked(); } } } "这是父图集" }
                                if !form.entry.parent.is_empty() {
                                    for (index, label) in ["裁剪 X", "裁剪 Y", "裁剪宽度", "裁剪高度"].into_iter().enumerate() {
                                        label { "{label}" input { r#type: "number", min: "0", disabled: busy(), aria_label: label, value: "{form.coordinates[index]}",
                                            oninput: move |event| { if let Some(d) = draft.write().as_mut() { d.coordinates[index] = event.value(); } }
                                        } }
                                    }
                                }
                            }
                            p { class: "rsb-dialog-copy", "路径和分组只修改清单，不移动包内文件；未匹配的定义会显示“未找到”。重命名图集会同步子图的父图集 ID，但不会改写 PAM、关卡或脚本中的其他引用。" }
                            details { summary { "写入清单来源" } pre { "{form.entry.source}" } }
                        }
                        if matches!(form.mode, Mode::Replace | Mode::Add) {
                            ResourceFileButton { disabled: busy(), on_file: move |file| { replacement.set(Some(file)); error.set(String::new()); }, on_error: move |message| error.set(message) }
                            if let Some((name, length)) = file_summary { p { "已选择：{name}（{length} 字节）" } }
                            if form.mode == Mode::Replace {
                                p { class: "rsb-dialog-copy", "普通资源按原始字节替换。PTX / 图集子图请选择同尺寸图片，将按原纹理格式重新编码；有损格式可能影响整个图集画质。修改尚未写入原文件。" }
                            } else {
                                p { class: "rsb-dialog-copy", "可选：同时添加普通文件，需要已存在的 RSG 子组和带扩展名的路径。子图只新增定义，再单独替换图片；新增 PTX 请使用包内文件视图。" }
                            }
                        }
                        if form.mode == Mode::Delete {
                            p { "确定删除 {form.entry.id} 的资源定义吗？" }
                            p { class: "rsb-dialog-copy", "仅从包内清单删除定义，保留实际文件。仍被子图引用的图集不能删除。PAM、关卡和脚本中的引用不会自动改写。" }
                        }
                        if !error().is_empty() { p { class: "rsb-resource-edit-error", role: "alert", "{error}" } }
                        if busy() { p { role: "status", "正在校验并同步资源…" } }
                    }
                    footer {
                        button { class: "rsb-dialog-button", disabled: busy(), onclick: move |_| draft.set(None), "取消" }
                        button { class: "rsb-dialog-button rsb-dialog-button--primary", disabled: busy(), onclick: move |_| submit.call(()), if form.mode == Mode::Delete { "确认删除定义" } else { "应用修改" } }
                    }
                }
            }
        }
    }
}

#[component]
fn ResourceFileButton(
    disabled: bool,
    on_file: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    #[cfg(target_arch = "wasm32")]
    return rsx! {
        label { class: "rsb-tool-button rsb-res-import", "选择内容文件"
            input { class: "rsb-file-input", r#type: "file", disabled, aria_label: "选择资源内容文件",
                onchange: move |event| async move {
                    if let Some(file) = event.files().into_iter().next() {
                        if file.size() > 128 * 1024 * 1024 { on_error.call("替换文件不能超过 128 MiB".into()); return; }
                        match file.read_bytes().await {
                            Ok(bytes) => on_file.call(AddedFile { name: file.name(), data: bytes.to_vec() }),
                            Err(error) => on_error.call(error.to_string()),
                        }
                    }
                }
            }
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    rsx! {
        button { class: "rsb-tool-button", disabled, onclick: move |_| async move {
            let Some(file) = rfd::AsyncFileDialog::new().pick_file().await else { return; };
            let data = std::fs::metadata(file.path()).map_err(|e| e.to_string()).and_then(|metadata| {
                if metadata.len() > 128 * 1024 * 1024 { return Err("替换文件不能超过 128 MiB".into()); }
                std::fs::read(file.path()).map_err(|e| e.to_string())
            });
            match data { Ok(data) => on_file.call(AddedFile { name: file.file_name(), data }), Err(error) => on_error.call(error) }
        }, "选择内容文件" }
    }
}
