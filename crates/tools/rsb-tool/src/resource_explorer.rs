use crate::domain::ArchiveDocument;
use crate::editing::{AddedFile, PacketEdits, RemovedPackets};
use crate::resource_browser::{
    BrowserFilter, BrowserSelection, BrowserTarget, DirectoryIndex, NavigationHistory, item_window,
    parent_path,
};
use crate::resources::{self, ManifestData, ResourceLocation};
use crate::virtual_scroll::{TABLE_DEFAULT_VIEWPORT_HEIGHT, measured_table_viewport_height};
use dioxus::prelude::*;
use dioxus_html::{ScrollBehavior, geometry::PixelsVector2D};
use std::collections::BTreeSet;
use std::sync::Arc;

#[component]
pub fn ResourceExplorer(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed: RemovedPackets,
    active: bool,
    on_close: EventHandler<()>,
    on_locate: EventHandler<ResourceLocation>,
) -> Element {
    let mut source = use_resource(use_reactive(
        &(archive.clone(), edits.clone(), removed.clone()),
        |(archive, edits, removed)| async move {
            Arc::new(resources::load_manifests(archive, edits, removed).await)
        },
    ));
    let mut imported = use_signal(ManifestData::default);
    let mut message = use_signal(String::new);
    let mut query = use_signal(String::new);
    let mut group = use_signal(String::new);
    let mut kind = use_signal(String::new);
    let mut state = use_signal(|| "all".to_string());
    let mut history = use_signal(NavigationHistory::default);
    let mut expanded = use_signal(|| BTreeSet::from([String::new()]));
    let mut selected = use_signal(BrowserSelection::default);
    let mut open_request = use_signal(|| None::<usize>);
    let mut grid = use_signal(|| false);
    let mut details = use_signal(|| true);
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut viewport_height = use_signal(|| TABLE_DEFAULT_VIEWPORT_HEIGHT);
    let mut viewport_width = use_signal(|| 800_usize);
    let mut mounted = use_signal(|| None::<MountedEvent>);

    let files = use_memo(use_reactive(
        &(archive.clone(), edits.clone(), removed.clone()),
        |(archive, edits, removed)| Arc::new(resources::physical_files(&archive, &edits, &removed)),
    ));
    let manifest = use_memo(move || {
        let mut data = source
            .read()
            .as_ref()
            .map(|data| data.as_ref().clone())
            .unwrap_or_default();
        data.merge(imported());
        Arc::new(data)
    });
    let catalog_data = use_resource(move || {
        let manifest = manifest.read().clone();
        let files = files.read().clone();
        async move {
            crate::processing::build_resource_catalog(manifest, files)
                .await
                .map(Arc::new)
        }
    });
    let catalog = use_memo(move || {
        catalog_data
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .cloned()
            .unwrap_or_default()
    });
    let tree = use_memo(move || Arc::new(DirectoryIndex::build(&catalog.read())));
    let items = use_memo(move || {
        Arc::new(tree.read().items(
            &catalog.read(),
            history.read().current(),
            BrowserFilter {
                query: &query(),
                group: &group(),
                kind: &kind(),
                state: &state(),
            },
        ))
    });
    use_effect(move || {
        let _ = items.read();
        selected.set(BrowserSelection::default());
    });
    use_effect(move || {
        let _ = items.read();
        let _ = grid.read();
        scroll_top.set(0.0);
        if let Some(event) = mounted.peek().clone() {
            spawn(async move {
                let _ = event
                    .scroll(PixelsVector2D::new(0.0, 0.0), ScrollBehavior::Instant)
                    .await;
            });
        }
    });
    use_effect(move || {
        if !catalog_data
            .read()
            .as_ref()
            .is_some_and(|result| result.is_ok())
        {
            return;
        }
        let tree = tree.read();
        let current = history.read().current().to_string();
        if !tree.directories.contains_key(&current) {
            history.set(NavigationHistory::default());
        }
    });
    let navigate = EventHandler::new(move |path: String| {
        for ancestor in tree.read().breadcrumbs(&path) {
            expanded.write().insert(ancestor);
        }
        history.write().navigate(path);
        query.set(String::new());
    });
    let data = manifest.read().clone();
    let catalog = catalog.read().clone();
    let tree = tree.read().clone();
    let items = items.read().clone();
    let current_path = history.read().current().to_string();
    let current_name = tree
        .directories
        .get(&current_path)
        .map(|folder| folder.name.clone())
        .unwrap_or_default();
    let tree_rows = tree.visible_tree(&expanded.read());
    let breadcrumbs = tree.breadcrumbs(&current_path);
    let window = item_window(
        items.len(),
        scroll_top(),
        viewport_height(),
        viewport_width(),
        grid(),
    );
    let current = selected
        .read()
        .focused
        .clone()
        .and_then(|target| match target {
            BrowserTarget::Resource(index) => catalog.rows.get(index).cloned(),
            BrowserTarget::Folder(_) => None,
        });
    let current_folder = selected
        .read()
        .focused
        .clone()
        .and_then(|target| match target {
            BrowserTarget::Folder(path) => tree
                .directories
                .get(&path)
                .cloned()
                .map(|folder| (path, folder)),
            BrowserTarget::Resource(_) => None,
        });
    let folder_count = items
        .iter()
        .filter(|item| matches!(item.target, BrowserTarget::Folder(_)))
        .count();
    let on_select = EventHandler::new({
        let items = items.clone();
        move |(index, toggle, range): (usize, bool, bool)| {
            selected.write().select(&items, index, toggle, range);
        }
    });
    let selected_indices = selected.read().resource_indices(&tree);
    let focused_index = match selected.read().focused {
        Some(BrowserTarget::Resource(index)) => Some(index),
        _ => None,
    };
    let loading = source.read().is_none() || catalog_data.read().is_none();
    let catalog_error = catalog_data
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned();
    let counts = [
        ("all", "全部状态", catalog.rows.len()),
        ("mapped", "已映射", catalog.mapped),
        ("missing", "未找到", catalog.missing),
        ("ambiguous", "多重匹配", catalog.ambiguous),
        ("unlisted", "未列入清单", catalog.unlisted),
        ("program", "程序资源", catalog.program),
    ];
    rsx! {
        section { class: "rsb-resource-explorer", hidden: !active, aria_label: "资源管理器",
            header { class: "rsb-res-heading",
                div { class: "rsb-res-title", ResourceIcon { kind: "archive" } strong { "资源管理器" } span { "RSB v{archive.header.version} · 逻辑路径" } }
                div { class: "rsb-res-actions",
                    ManifestImportButton {
                        on_import: move |file: AddedFile| {
                            let name = file.name.clone();
                            message.set(format!("正在读取 {name}…"));
                            spawn(async move {
                                match crate::processing::parse_resource_manifest(file).await {
                                    Ok(data) => {
                                        message.set(format!("已导入 {} · {} 个资源，仅用于映射", name, data.resources.len()));
                                        imported.write().merge(data);
                                    }
                                    Err(error) => message.set(format!("导入失败：{error}")),
                                }
                            });
                        },
                        on_error: move |error| message.set(error),
                    }
                    button { class: "rsb-tool-button", disabled: loading, onclick: move |_| { source.restart(); message.set(String::new()); }, "重新扫描" }
                    button { class: "rsb-tool-button", onclick: move |_| on_close.call(()), "返回包内文件" }
                }
            }
            div { class: "rsb-res-addressbar",
                div { class: "rsb-res-navigation",
                    button { title: "后退", aria_label: "后退", disabled: !history.read().can_back(),
                        onclick: move |_| { history.write().back(); query.set(String::new()); }, ResourceIcon { kind: "back" } }
                    button { title: "前进", aria_label: "前进", disabled: !history.read().can_forward(),
                        onclick: move |_| { history.write().forward(); query.set(String::new()); }, ResourceIcon { kind: "forward" } }
                    button { title: "上一级", aria_label: "上一级", disabled: current_path.is_empty(),
                        onclick: { let path = parent_path(&current_path).to_string(); move |_| navigate.call(path.clone()) }, ResourceIcon { kind: "up" } }
                }
                nav { class: "rsb-res-breadcrumbs", aria_label: "资源路径",
                    for (index, path) in breadcrumbs.iter().enumerate() {
                        if index > 0 { ResourceIcon { kind: "right" } }
                        button { title: "{tree.directories[path].name}", aria_current: if *path == current_path { "page" } else { "false" },
                            onclick: { let path = path.clone(); move |_| navigate.call(path.clone()) },
                            if index == 0 { ResourceIcon { kind: "archive" } }
                            "{tree.directories[path].name}"
                        }
                    }
                }
                div { class: "rsb-res-search",
                    ResourceIcon { kind: "search" }
                    input { r#type: "search", placeholder: "搜索 {current_name} 及子目录", aria_label: "搜索资源", value: query(), oninput: move |event| query.set(event.value()) }
                }
            }
            div { class: "rsb-res-filterbar",
                select { aria_label: "资源组", value: group(), onchange: move |event| group.set(event.value()),
                    option { value: "", "全部资源组" }
                    for (name, count) in &catalog.groups { option { value: "{name}", "{name} ({count})" } }
                }
                select { aria_label: "资源类型", value: kind(), onchange: move |event| kind.set(event.value()),
                    option { value: "", "全部类型" }
                    for name in &catalog.kinds { option { value: "{name}", "{name}" } }
                }
                select { aria_label: "映射状态", value: state(), onchange: move |event| state.set(event.value()),
                    for (id, label, count) in counts { option { value: "{id}", "{label} ({count})" } }
                }
                if !group().is_empty() || !kind().is_empty() || state() != "all" || !query().is_empty() {
                    button { class: "rsb-res-reset", onclick: move |_| { group.set(String::new()); kind.set(String::new()); state.set("all".into()); query.set(String::new()); }, "清除筛选" }
                }
                div { class: "rsb-res-view-switch",
                    button { title: "列表视图", aria_label: "列表视图", aria_pressed: !grid(), onclick: move |_| grid.set(false), ResourceIcon { kind: "list" } }
                    button { title: "图标视图", aria_label: "图标视图", aria_pressed: grid(), onclick: move |_| grid.set(true), ResourceIcon { kind: "grid" } }
                    button { title: "详情窗格", aria_label: "详情窗格", aria_pressed: details(), onclick: move |_| { let next = !details(); details.set(next); }, ResourceIcon { kind: "panel" } }
                }
            }
            div { class: "rsb-res-operationbar",
                button { class: "rsb-tool-button", disabled: items.is_empty(),
                    onclick: { let items = items.clone(); move |_| selected.set(BrowserSelection::all(&items)) }, "全选" }
                button { class: "rsb-tool-button", disabled: selected.read().targets.is_empty(),
                    onclick: move |_| selected.set(BrowserSelection::default()), "取消选择" }
                crate::resource_operations::ResourceOperations {
                    archive: archive.clone(), edits, removed, catalog: catalog.clone(),
                    selected_indices, focused_index, open_request, message,
                }
            }
            div { class: if details() { "rsb-res-body" } else { "rsb-res-body without-details" },
                nav { class: "rsb-res-tree", aria_label: "资源目录",
                    div { class: "rsb-res-tree-title", "目录" small { "逻辑资源路径" } }
                    for (path, depth) in tree_rows {
                        if let Some(folder) = tree.directories.get(&path) {
                            div { class: if path == current_path { "rsb-res-tree-row is-active" } else { "rsb-res-tree-row" }, style: "padding-left: {depth * 14 + 6}px",
                                button { class: "rsb-res-expand", aria_label: format!("{} {}", if expanded.read().contains(&path) { "折叠" } else { "展开" }, folder.name), disabled: folder.children.is_empty(),
                                    onclick: { let path = path.clone(); move |_| { if !expanded.write().remove(&path) { expanded.write().insert(path.clone()); } } },
                                    if !folder.children.is_empty() { ResourceIcon { kind: if expanded.read().contains(&path) { "down" } else { "right" } } }
                                }
                                button { class: "rsb-res-tree-link", title: "{folder.name} · {folder.resources} 个资源", aria_current: if path == current_path { "page" } else { "false" },
                                    onclick: { let path = path.clone(); move |_| navigate.call(path.clone()) },
                                    ResourceIcon { kind: if path.is_empty() { "archive" } else { "folder" } }
                                    span { "{folder.name}" } small { "{folder.resources}" }
                                }
                            }
                        }
                    }
                }
                main { class: if grid() { "rsb-res-results is-grid" } else { "rsb-res-results" },
                    div { class: "rsb-res-table-head",
                        if grid() { span { if query().trim().is_empty() { "{current_name}" } else { "搜索结果 · 包含子目录" } } }
                        else { span { "名称" } span { "类型 / 子组" } span { "映射状态" } }
                    }
                    div { class: "rsb-res-list", role: "list", aria_label: "资源列表",
                        tabindex: 0,
                        onkeydown: { let items = items.clone(); move |event| {
                            let modifiers = event.modifiers();
                            if (modifiers.ctrl() || modifiers.meta()) && event.key().to_string().eq_ignore_ascii_case("a") {
                                event.prevent_default(); selected.set(BrowserSelection::all(&items));
                            } else if event.key() == Key::Escape { selected.set(BrowserSelection::default()); }
                        } },
                        onmounted: move |event| {
                            mounted.set(Some(event.clone()));
                            async move { if let Ok(rect) = event.get_client_rect().await {
                                viewport_height.set(measured_table_viewport_height(rect.height()));
                                viewport_width.set(rect.width().max(1.0) as usize);
                            } }
                        },
                        onresize: move |event| { if let Ok(size) = event.get_content_box_size() {
                            viewport_height.set(measured_table_viewport_height(size.height));
                            viewport_width.set(size.width.max(1.0) as usize);
                        } },
                        onscroll: move |event| scroll_top.set(event.scroll_top()),
                        if loading { div { class: "rsb-res-empty", "正在读取资源清单…" } }
                        else if items.is_empty() { div { class: "rsb-res-empty", ResourceIcon { kind: "search" } p { "当前目录没有符合条件的资源" } } }
                        div { class: "rsb-res-virtual-space", style: "height: {window.content_height}px",
                            for visual_index in window.start..window.end {
                                if let Some(item) = items.get(visual_index) {
                                    {
                                        let target = item.target.clone();
                                        let is_selected = selected.read().targets.contains(&target);
                                        let class = format!("rsb-res-item {} {}", if grid() { "rsb-res-tile" } else { "rsb-res-row" }, if is_selected { "is-selected" } else { "" });
                                        let width = 100.0 / window.columns as f64;
                                        let style = format!("top: {}px; left: {}%; width: {}%; height: {}px", (visual_index / window.columns) * window.height, (visual_index % window.columns) as f64 * width, width, window.height);
                                        match target {
                                            BrowserTarget::Folder(path) => {
                                                let folder = &tree.directories[&path];
                                                let open_path = path.clone();
                                                rsx! {
                                                    button { key: "folder-{path}", class, style, title: "{folder.name} · {item.count} 个资源（含子目录）", aria_pressed: is_selected,
                                                        "data-resource-folder": "{folder.name}",
                                                        onclick: move |event| { let m = event.modifiers(); on_select.call((visual_index, m.ctrl() || m.meta(), m.shift())); },
                                                        ondoubleclick: move |_| navigate.call(open_path.clone()),
                                                        onkeydown: { let path = path.clone(); move |event| { if event.key() == Key::Enter { navigate.call(path.clone()); } } },
                                                        span { class: "rsb-res-file-cell",
                                                            span { class: "rsb-res-check", role: "checkbox", aria_checked: is_selected, aria_label: "选择 {folder.name}",
                                                                onclick: move |event| { event.stop_propagation(); on_select.call((visual_index, true, false)); },
                                                                if is_selected { "✓" }
                                                            }
                                                            ResourceIcon { kind: "folder" }
                                                            span { class: "rsb-res-name", strong { "{folder.name}" } small { "{item.count} 个资源" } }
                                                        }
                                                        span { class: "rsb-res-kind", span { "文件夹" } small { "{folder.children.len()} 个子目录" } }
                                                        span { class: "rsb-res-folder-arrow", ResourceIcon { kind: "right" } }
                                                    }
                                                }
                                            }
                                            BrowserTarget::Resource(index) => {
                                                let row = &catalog.rows[index];
                                                let path = &tree.paths[index];
                                                rsx! {
                                                    button { key: "resource-{index}", class, style, title: "{row.entry.path}\n{row.entry.id}\n{row.entry.subgroup}", aria_pressed: is_selected,
                                                        "data-resource-id": "{row.entry.id}",
                                                        onclick: move |event| { let m = event.modifiers(); on_select.call((visual_index, m.ctrl() || m.meta(), m.shift())); },
                                                        ondoubleclick: move |_| open_request.set(Some(index)),
                                                        onkeydown: move |event| { if event.key() == Key::Enter { open_request.set(Some(index)); } },
                                                        span { class: "rsb-res-file-cell",
                                                            span { class: "rsb-res-check", role: "checkbox", aria_checked: is_selected, aria_label: "选择 {path.name}",
                                                                onclick: move |event| { event.stop_propagation(); on_select.call((visual_index, true, false)); },
                                                                if is_selected { "✓" }
                                                            }
                                                            ResourceIcon { kind: resource_icon(&row.entry.kind) }
                                                            span { class: "rsb-res-name",
                                                                strong { "{path.name}" }
                                                                small { if query().trim().is_empty() { "{row.entry.id}" } else { "{row.entry.path}" } }
                                                            }
                                                        }
                                                        span { class: "rsb-res-kind", span { "{row.entry.kind}" } small { "{row.entry.subgroup}" } }
                                                        span { class: "rsb-res-badge {row.state.class()}", "{row.state.label()}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if details() {
                    aside { class: "rsb-res-details", aria_label: "资源详情",
                        if let Some(row) = current {
                            div { class: "rsb-res-detail-symbol", ResourceIcon { kind: resource_icon(&row.entry.kind) } }
                            header { strong { if row.entry.id.is_empty() { "包内文件" } else { "{row.entry.id}" } } span { class: "rsb-res-badge {row.state.class()}", "{row.state.label()}" } }
                            p { class: "rsb-res-explanation", "{row.explanation}" }
                            dl {
                                dt { "逻辑路径" } dd { "{row.entry.path}" }
                                dt { "类型" } dd { "{row.entry.kind}" }
                                dt { "资源组" } dd { "{row.entry.group}" }
                                dt { "子组 / RSG" } dd { "{row.entry.subgroup}" }
                                if !row.entry.resolution.is_empty() { dt { "分辨率" } dd { "{row.entry.resolution}" } }
                                if !row.entry.language.is_empty() { dt { "语言" } dd { "{row.entry.language}" } }
                                if row.entry.atlas { dt { "图集" } dd { "是" } }
                                if !row.entry.parent.is_empty() { dt { "父图集 ID" } dd { "{row.entry.parent}" } }
                                if let Some(region) = row.entry.region { dt { "图集裁剪区域" } dd { "x={region.x}, y={region.y}\n{region.width} × {region.height}" } }
                                dt { "清单来源" } dd { "{row.entry.source}" }
                            }
                            for location in row.locations {
                                div { class: "rsb-res-target",
                                    small { "{location.packet_name}" }
                                    code { "{location.path}" }
                                    button { class: "rsb-tool-button", onclick: move |_| on_locate.call(location.clone()), "定位包内文件" }
                                }
                            }
                        } else if let Some((path, folder)) = current_folder {
                            div { class: "rsb-res-detail-symbol", ResourceIcon { kind: "folder" } }
                            header { strong { "{folder.name}" } span { class: "rsb-res-badge", "文件夹" } }
                            p { class: "rsb-res-explanation", "按清单中的逻辑路径组织，资源仍保存在各自的 RSG 中。" }
                            dl {
                                dt { "包含资源（含子目录）" } dd { "{folder.resources}" }
                                dt { "直接子目录" } dd { "{folder.children.len()}" }
                            }
                            button { class: "rsb-tool-button rsb-res-open-folder", onclick: move |_| navigate.call(path.clone()), "打开文件夹" }
                        } else {
                            div { class: "rsb-res-empty",
                                ResourceIcon { kind: "folder" }
                                strong { "选择文件或文件夹" }
                                p { "双击文件夹进入，双击资源打开工具或预览图片。" }
                                p { "勾选或 Ctrl / ⌘ 多选，Shift 连选。图集子图可预览、导出为 PNG。" }
                            }
                        }
                    }
                }
            }
            footer { class: "rsb-res-footer",
                div { class: "rsb-res-statusline",
                    span { "{folder_count} 个文件夹 · {items.len() - folder_count} 个资源" }
                    span { "{catalog.mapped} 已映射 · {catalog.missing} 未找到 · {files.read().len()} 个包内文件" }
                }
                if let Some(error) = catalog_error { p { role: "alert", "{error}" } }
                if !message().is_empty() { p { role: "status", "{message}" } }
                if data.sources.is_empty() && !loading {
                    p { "未发现资源清单，当前显示包内文件索引。可导入 RESOURCES.RTON、RESOURCES.NEWTON 或 JSON 资源描述。" }
                } else {
                    details {
                        summary { "已合并 {data.sources.len()} 份清单 · {data.resources.len()} 个逻辑资源 · 只读映射，不修改 RSB" }
                        for source in &data.sources { p { "{source}" } }
                    }
                }
                if !data.warnings.is_empty() {
                    details { summary { "{data.warnings.len()} 条清单读取提示" }
                        for warning in &data.warnings { p { "{warning}" } }
                    }
                }
            }
        }
    }
}

fn resource_icon(kind: &str) -> &'static str {
    match kind {
        "Image" => "image",
        "PopAnim" => "animation",
        "SoundBank" | "DecodedSoundBank" => "audio",
        _ => "file",
    }
}

#[component]
fn ResourceIcon(kind: String) -> Element {
    let path = match kind.as_str() {
        "folder" => "M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10H3Z M3 10h18",
        "archive" => "M3 4h18v5H3z M5 9v11h14V9 M9 13h6",
        "image" => "M4 3h16v18H4z M4 16l5-5 4 4 3-3 4 4 M15 7h.01",
        "animation" => "M4 3h16v18H4z M9 8l7 4-7 4z",
        "audio" => "M9 18V5l11-2v13 M9 5v4l11-2 M9 18a3 2 0 1 1-3-2h3 M20 16a3 2 0 1 1-3-2h3",
        "back" => "M19 12H5 M11 6l-6 6 6 6",
        "forward" => "M5 12h14 M13 6l6 6-6 6",
        "up" => "M12 19V5 M6 11l6-6 6 6",
        "right" => "m9 6 6 6-6 6",
        "down" => "m6 9 6 6 6-6",
        "search" => "M16 16l5 5 M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
        "list" => "M8 6h13 M8 12h13 M8 18h13 M3 6h.01 M3 12h.01 M3 18h.01",
        "grid" => "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
        "panel" => "M3 4h18v16H3z M15 4v16",
        _ => "M4 3h10l6 6v12H4z M14 3v6h6 M8 14h8 M8 17h6",
    };
    rsx! { svg { class: "rsb-res-icon icon-{kind}", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
        path { d: path }
    } }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ManifestImportButton(
    on_import: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "rsb-tool-button rsb-res-import", "导入清单"
            input { class: "rsb-file-input", r#type: "file", accept: ".rton,.newton,.json", aria_label: "导入资源清单",
                onchange: move |event| async move {
                    if let Some(file) = event.files().into_iter().next() {
                        match file.read_bytes().await {
                            Ok(bytes) => on_import.call(AddedFile { name: file.name(), data: bytes.to_vec() }),
                            Err(error) => on_error.call(error.to_string()),
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn ManifestImportButton(
    on_import: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        button { class: "rsb-tool-button", onclick: move |_| async move {
            let Some(file) = rfd::AsyncFileDialog::new().add_filter("资源清单", &["rton", "newton", "json"]).pick_file().await else { return; };
            let name = file.file_name();
            match std::fs::read(file.path()) {
                Ok(data) => on_import.call(AddedFile { name, data }),
                Err(error) => on_error.call(error.to_string()),
            }
        }, "导入清单" }
    }
}
