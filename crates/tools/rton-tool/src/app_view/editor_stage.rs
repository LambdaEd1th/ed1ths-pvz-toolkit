use dioxus::prelude::*;
use dioxus_html::input_data::MouseButton;

use super::layout::EmptyDropStage;
use super::text_selection::*;
use crate::components::TextJumpTarget;
use crate::components::context_menu_transition::{dismiss_context_menu, show_context_menu};
use crate::components::{HexEditor, HexJumpTarget};
use crate::domain::virtual_scroll::{TEXT_MAX_VIRTUAL_SCROLL_HEIGHT, TEXT_OVERSCAN_ROWS};
use crate::domain::{
    ByteDocument, EditorMode, HexEdit, IdentityArc, TEXT_DEFAULT_VIEWPORT_HEIGHT, TEXT_ROW_HEIGHT,
    TabTaskState, TextBuffer, TextContentState, TextRangeReplacement,
    measured_text_viewport_height, text_virtual_row_top, text_virtual_scroll,
};
use crate::i18n::I18n;
use crate::platform;
use std::sync::Arc;

const TEXT_WRAP_CHAR_WIDTH: usize = 8;
const TEXT_WRAP_LINE_NUMBER_WIDTH: usize = 76;
const TEXT_WRAP_HORIZONTAL_PADDING: usize = 30;
const TEXT_DEFAULT_VIEWPORT_WIDTH: usize = 960;
const TEXT_WRAP_LAYOUT_BATCH_LINES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextWrapLayout {
    line_tops: Arc<Vec<usize>>,
    line_heights: Arc<Vec<usize>>,
    logical_height: usize,
}

struct TextWrapLayoutBuilder {
    chars_per_row: usize,
    line_tops: Vec<usize>,
    line_heights: Vec<usize>,
    logical_height: usize,
}

impl TextWrapLayoutBuilder {
    fn new(line_count: usize, viewport_width: usize) -> Self {
        Self {
            chars_per_row: text_wrap_chars_per_row(viewport_width),
            line_tops: Vec::with_capacity(line_count),
            line_heights: Vec::with_capacity(line_count),
            logical_height: 0,
        }
    }

    fn push_line(&mut self, line_len: usize) {
        self.line_tops.push(self.logical_height);
        let visual_rows = line_len.max(1).div_ceil(self.chars_per_row).max(1);
        let height = visual_rows.saturating_mul(TEXT_ROW_HEIGHT);
        self.line_heights.push(height);
        self.logical_height = self.logical_height.saturating_add(height);
    }

    fn finish(self) -> TextWrapLayout {
        TextWrapLayout {
            line_tops: Arc::new(self.line_tops),
            line_heights: Arc::new(self.line_heights),
            logical_height: self.logical_height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WrappedTextScroll {
    content_height: usize,
    logical_scroll_top: usize,
    display_scroll_top: usize,
    start_row: usize,
    end_row: usize,
    scaled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleTextLine {
    top: i64,
    height: usize,
    line_index: usize,
    line_number: usize,
    line_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualTextDrag {
    anchor: VirtualTextPoint,
    active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualTextContextMenu {
    x: i32,
    y: i32,
}

#[derive(Clone, PartialEq)]
pub(super) struct EditorStageTab {
    pub(super) id: usize,
    pub(super) mode: EditorMode,
    pub(super) worker_document_id: Option<u64>,
    pub(super) text_buffer: Option<IdentityArc<TextBuffer>>,
    pub(super) text_state: TextContentState,
    pub(super) task_state: Option<TabTaskState>,
}

#[component]
pub(super) fn EditorStage(
    i18n: I18n,
    active_tab: Option<EditorStageTab>,
    active_byte_doc: Option<ByteDocument>,
    hex_jump_target: Option<HexJumpTarget>,
    text_jump_target: Option<TextJumpTarget>,
    line_wrapping: bool,
    editor_search_panel_visible: bool,
    editor_search_text: String,
    editor_replace_text: String,
    editor_search_case_sensitive: bool,
    editor_search_controls_disabled: bool,
    editor_search_status_text: String,
    on_hex_change: EventHandler<Vec<HexEdit>>,
    on_virtual_text_range_replace: EventHandler<TextRangeReplacement>,
    on_undo: EventHandler<()>,
    on_redo: EventHandler<()>,
    on_search_visible_change: EventHandler<bool>,
    on_find_input: EventHandler<String>,
    on_case_sensitive_change: EventHandler<bool>,
    on_replace_input: EventHandler<String>,
    on_previous_match: EventHandler<()>,
    on_next_match: EventHandler<()>,
    on_replace_current: EventHandler<()>,
    on_replace_all: EventHandler<()>,
    on_find_key: EventHandler<KeyboardEvent>,
    on_replace_key: EventHandler<KeyboardEvent>,
    suppress_resize_observer: bool,
) -> Element {
    rsx! {
        section { class: "rton-editor-stage",
            if let Some(active_tab) = active_tab.as_ref() {
                if let Some(task) = active_tab.task_state.as_ref() {
                    div { class: "editor-surface",
                        div { class: "empty-state",
                            strong { {i18n.t_args("editor-converting-title", &[("mode", task.target_mode.label().to_string())])} }
                            p { {i18n.t("editor-converting-subtitle")} }
                        }
                    }
                } else if active_tab.mode == EditorMode::RtonHex {
                    if let Some(byte_doc) = active_byte_doc.clone() {
                        HexEditor {
                            key: "{active_tab.id}-hex",
                            bytes: byte_doc,
                            worker_document_id: active_tab.worker_document_id,
                            jump_target: hex_jump_target,
                            search_panel_visible: editor_search_panel_visible,
                            i18n,
                            on_change: on_hex_change,
                            on_undo,
                            on_redo,
                            on_search_visible_change,
                            suppress_resize_observer
                        }
                    } else {
                        div { class: "editor-surface",
                            div { class: "empty-state",
                                strong { {i18n.t("hex-invalid-title")} }
                                p { {i18n.t("hex-empty")} }
                            }
                        }
                    }
                } else if active_tab.text_state.text_format().is_some() {
                    div {
                        class: "editor-surface",
                        if let Some(buffer) = active_tab.text_buffer.clone() {
                            VirtualTextEditor {
                                key: "{active_tab.id}-{active_tab.mode.label()}-virtual-text",
                                i18n,
                                buffer,
                                jump_target: text_jump_target,
                                line_wrapping,
                                on_change: on_virtual_text_range_replace,
                                on_undo,
                                on_redo,
                                on_search_visible_change,
                                suppress_resize_observer
                            }
                        } else {
                            div { class: "empty-state",
                                strong { {i18n.t("hex-empty")} }
                                p { {i18n.t("hex-empty")} }
                            }
                        }
                        if editor_search_panel_visible {
                            div { class: "editor-search-panel",
                                input {
                                    id: "rton-editor-find-input",
                                    class: "editor-search-field",
                                    r#type: "search",
                                    placeholder: i18n.t("editor-find-placeholder"),
                                    value: "{editor_search_text}",
                                    spellcheck: "false",
                                    onmounted: move |_| focus_editor_find_input(),
                                    oninput: move |event| on_find_input.call(event.value()),
                                    onkeydown: move |event| {
                                        event.stop_propagation();
                                        on_find_key.call(event);
                                    }
                                }
                                button {
                                    r#type: "button",
                                    class: "editor-search-button",
                                    disabled: editor_search_controls_disabled,
                                    onclick: move |_| on_previous_match.call(()),
                                    {i18n.t("editor-previous")}
                                }
                                button {
                                    r#type: "button",
                                    class: "editor-search-button",
                                    disabled: editor_search_controls_disabled,
                                    onclick: move |_| on_next_match.call(()),
                                    {i18n.t("editor-next")}
                                }
                                label { class: "editor-search-check",
                                    input {
                                        r#type: "checkbox",
                                        checked: editor_search_case_sensitive,
                                        onchange: move |event| on_case_sensitive_change.call(event.checked())
                                    }
                                    span { {i18n.t("editor-case")} }
                                }
                                input {
                                    class: "editor-search-field editor-replace-field",
                                    value: "{editor_replace_text}",
                                    placeholder: i18n.t("editor-replace-placeholder"),
                                    spellcheck: "false",
                                    oninput: move |event| on_replace_input.call(event.value()),
                                    onkeydown: move |event| {
                                        event.stop_propagation();
                                        on_replace_key.call(event);
                                    }
                                }
                                button {
                                    r#type: "button",
                                    class: "editor-search-button",
                                    disabled: editor_search_controls_disabled,
                                    onclick: move |_| on_replace_current.call(()),
                                    {i18n.t("editor-replace")}
                                }
                                button {
                                    r#type: "button",
                                    class: "editor-search-button",
                                    disabled: editor_search_controls_disabled,
                                    onclick: move |_| on_replace_all.call(()),
                                    {i18n.t("editor-replace-all")}
                                }
                                span { class: "editor-search-status", "{editor_search_status_text}" }
                                button {
                                    r#type: "button",
                                    class: "editor-search-close",
                                    title: i18n.t("editor-close-search"),
                                    onclick: move |_| on_search_visible_change.call(false),
                                    "×"
                                }
                            }
                        }
                    }
                }
            } else {
                EmptyDropStage { i18n }
            }
        }
    }
}

#[component]
fn VirtualTextEditor(
    i18n: I18n,
    buffer: IdentityArc<TextBuffer>,
    jump_target: Option<TextJumpTarget>,
    line_wrapping: bool,
    on_change: EventHandler<TextRangeReplacement>,
    on_undo: EventHandler<()>,
    on_redo: EventHandler<()>,
    on_search_visible_change: EventHandler<bool>,
    suppress_resize_observer: bool,
) -> Element {
    let mut scroll_top = use_signal(|| 0_f64);
    let drag_selection = use_signal(|| None::<VirtualTextDrag>);
    let mut caret = use_signal(|| VirtualTextPoint {
        line_index: 0,
        column_utf16: 0,
    });
    let mut selection = use_signal(|| None::<VirtualTextSelection>);
    let mut input_sink_value = use_signal(String::new);
    let context_menu = use_signal(|| None::<VirtualTextContextMenu>);
    let context_menu_closing = use_signal(|| false);
    let context_menu_generation = use_signal(|| 0_u64);
    let viewport_height = use_signal(|| TEXT_DEFAULT_VIEWPORT_HEIGHT);
    let viewport_width = use_signal(|| TEXT_DEFAULT_VIEWPORT_WIDTH);
    let mut wrap_layout = use_signal(|| None::<Arc<TextWrapLayout>>);
    let mut wrap_layout_generation = use_signal(|| 0_u64);
    let mut mounted = use_signal(|| None::<MountedEvent>);
    let mut last_jump_id = use_signal(|| None::<u64>);
    let line_count = buffer.line_count();
    let scroll_top_snapshot = *scroll_top.read();
    let viewport_height_snapshot = *viewport_height.read();
    let viewport_width_snapshot = *viewport_width.read();
    let selection_snapshot = *selection.read();
    let caret_snapshot = *caret.read();
    let context_menu_snapshot = *context_menu.read();
    let context_menu_closing_snapshot = *context_menu_closing.read();

    {
        let buffer = buffer.clone();
        use_effect(use_reactive(&buffer, move |buffer| {
            let next_caret = clamp_virtual_text_point(*caret.peek(), &buffer);
            if next_caret != *caret.peek() {
                caret.set(next_caret);
            }
            let current_selection = *selection.peek();
            let next_selection =
                current_selection.and_then(|value| clamp_virtual_text_selection(value, &buffer));
            if next_selection != *selection.peek() {
                selection.set(next_selection);
            }
        }));
    }

    use_effect(use_reactive(&suppress_resize_observer, move |suppressed| {
        if suppressed {
            return;
        }
        let Some(event) = mounted.peek().clone() else {
            return;
        };
        spawn(async move {
            if let Ok(rect) = event.get_client_rect().await {
                update_text_buffer_viewport_size(
                    viewport_height,
                    viewport_width,
                    rect.height(),
                    rect.width(),
                );
            }
        });
    }));

    {
        let buffer = buffer.clone();
        use_effect(use_reactive(
            &(jump_target, buffer),
            move |(target, buffer)| {
                let Some(target) = target else {
                    return;
                };
                if Some(target.id) == *last_jump_id.peek() {
                    return;
                }
                last_jump_id.set(Some(target.id));
                let start =
                    virtual_text_point_from_jump_target(&buffer, target.line, target.column);
                let end = virtual_text_point_from_jump_target(
                    &buffer,
                    target.line,
                    target.selection_end_column,
                );
                caret.set(end);
                selection.set(virtual_text_selection_from_distinct_points(start, end));
                scroll_virtual_text_to_target(target);
            },
        ));
    }

    {
        let buffer = buffer.clone();
        use_effect(use_reactive(
            &(buffer, line_wrapping, viewport_width_snapshot),
            move |(buffer, line_wrapping, viewport_width)| {
                let generation = (*wrap_layout_generation.peek()).wrapping_add(1);
                wrap_layout_generation.set(generation);
                wrap_layout.set(None);
                if !line_wrapping {
                    return;
                }

                spawn(async move {
                    let Some(layout) = responsive_text_wrap_layout(
                        buffer,
                        viewport_width,
                        wrap_layout_generation,
                        generation,
                    )
                    .await
                    else {
                        return;
                    };
                    if *wrap_layout_generation.peek() == generation {
                        wrap_layout.set(Some(Arc::new(layout)));
                    }
                });
            },
        ));
    }
    let wrap_layout_snapshot = wrap_layout.read().clone();
    let wrapped_scroll = wrap_layout_snapshot
        .as_deref()
        .map(|layout| wrapped_text_scroll(layout, scroll_top_snapshot, viewport_height_snapshot));
    let virtual_scroll = wrap_layout_snapshot
        .is_none()
        .then(|| text_virtual_scroll(line_count, scroll_top_snapshot, viewport_height_snapshot));
    let content_height = wrapped_scroll
        .map(|scroll| scroll.content_height)
        .or_else(|| virtual_scroll.map(|scroll| scroll.content_height))
        .unwrap_or(0);
    let effective_line_wrapping = line_wrapping && wrap_layout_snapshot.is_some();
    let visible_lines = text_buffer_visible_lines(
        &buffer,
        scroll_top_snapshot,
        virtual_scroll,
        wrap_layout_snapshot.as_deref(),
        wrapped_scroll,
    );
    let preview_class = match (effective_line_wrapping, selection_snapshot.is_some()) {
        (true, true) => "virtual-text-preview wrap has-logical-selection",
        (true, false) => "virtual-text-preview wrap",
        (false, true) => "virtual-text-preview has-logical-selection",
        (false, false) => "virtual-text-preview",
    };

    rsx! {
        div {
            class: preview_class,
            onmousedown: move |_| {
                dismiss_context_menu(
                    context_menu,
                    context_menu_closing,
                    context_menu_generation,
                );
            },
            div {
                class: "virtual-text-preview-content",
                tabindex: "0",
                style: "--virtual-text-row-height: {TEXT_ROW_HEIGHT}px",
                onmounted: move |event| {
                    mounted.set(Some(event.clone()));
                    async move {
                        focus_virtual_text_input_layer();
                        if let Ok(rect) = event.get_client_rect().await {
                            update_text_buffer_viewport_size(
                                viewport_height,
                                viewport_width,
                                rect.height(),
                                rect.width(),
                            );
                        }
                    }
                },
                onresize: move |event| {
                    if suppress_resize_observer {
                        return;
                    }
                    if let Ok(size) = event.get_content_box_size() {
                        update_text_buffer_viewport_size(
                            viewport_height,
                            viewport_width,
                            size.height,
                            size.width,
                        );
                    }
                },
                onscroll: move |event| {
                    dismiss_context_menu(
                        context_menu,
                        context_menu_closing,
                        context_menu_generation,
                    );
                    scroll_top.set(event.scroll_top());
                },
                onmouseup: move |_| finish_virtual_text_drag(drag_selection),
                onmouseleave: move |_| finish_virtual_text_drag(drag_selection),
                textarea {
                    id: "rton-virtual-text-input-sink",
                    class: "virtual-text-input-sink",
                    aria_label: "Text editor input",
                    spellcheck: "false",
                    autocapitalize: "off",
                    autocomplete: "off",
                    value: "{input_sink_value}",
                    onblur: move |_| {
                        dismiss_context_menu(
                            context_menu,
                            context_menu_closing,
                            context_menu_generation,
                        );
                    },
                    onkeydown: {
                        let buffer = buffer.clone();
                        move |event| {
                            if event.key().to_string() == "Escape" && context_menu.peek().is_some() {
                                event.prevent_default();
                                dismiss_context_menu(
                                    context_menu,
                                    context_menu_closing,
                                    context_menu_generation,
                                );
                                return;
                            }
                            dismiss_context_menu(
                                context_menu,
                                context_menu_closing,
                                context_menu_generation,
                            );
                            handle_virtual_text_key(
                                event,
                                &buffer,
                                caret,
                                selection,
                                input_sink_value,
                                on_change,
                                on_undo,
                                on_redo,
                                on_search_visible_change,
                            );
                        }
                    },
                    oninput: {
                        let buffer = buffer.clone();
                        move |event| {
                            dismiss_context_menu(
                                context_menu,
                                context_menu_closing,
                                context_menu_generation,
                            );
                            let text = event.value();
                            input_sink_value.set(String::new());
                            if text.is_empty() {
                                return;
                            }
                            replace_virtual_text_selection_or_caret(
                                &buffer,
                                caret,
                                selection,
                                on_change,
                                text,
                            );
                        }
                    },
                    oncopy: {
                        let buffer = buffer.clone();
                        move |event| {
                            event.prevent_default();
                            copy_virtual_text_selection_to_clipboard(&buffer, *selection.read());
                        }
                    },
                    oncut: {
                        let buffer = buffer.clone();
                        move |event| {
                            event.prevent_default();
                            cut_virtual_text_selection_to_clipboard(
                                &buffer,
                                caret,
                                selection,
                                on_change,
                            );
                        }
                    },
                    onpaste: {
                        let buffer = buffer.clone();
                        move |event| {
                            if let Some(text) = virtual_text_clipboard_event_text(&event) {
                                event.prevent_default();
                                replace_virtual_text_selection_or_caret(
                                    &buffer,
                                    caret,
                                    selection,
                                    on_change,
                                    text,
                                );
                            } else {
                                input_sink_value.set(String::new());
                            }
                        }
                    }
                }
                div {
                    class: "virtual-text-virtual-space",
                    style: "height: {content_height}px",
                    for line in visible_lines {
                        div {
                            class: "virtual-text-virtual-row",
                            style: "transform: translateY({line.top}px); height: {line.height}px",
                            span {
                                class: "virtual-text-line-number",
                                "data-line-number": "{line.line_number}",
                                aria_hidden: "true"
                            }
                            {
                                let drag_selection_for_start = drag_selection;
                                let caret_for_start = caret;
                                let selection_for_start = selection;
                            let drag_selection_for_update = drag_selection;
                            let caret_for_update = caret;
                            let selection_for_update = selection;
                            let mounted_for_context_menu = mounted;
                            let context_menu_for_open = context_menu;
                            let context_menu_closing_for_open = context_menu_closing;
                            let context_menu_generation_for_open = context_menu_generation;
                            rsx! {
                                VirtualTextLineView {
                                        key: "{line.line_index}-view",
                                        line_index: line.line_index,
                                        line_number: line.line_number,
                                        line_text: line.line_text,
                                        caret: caret_snapshot,
                                        selection: selection_snapshot,
                                        on_drag_start: EventHandler::new(move |(point, extend)| {
                                            start_virtual_text_drag(
                                                drag_selection_for_start,
                                                caret_for_start,
                                                selection_for_start,
                                                point,
                                                extend,
                                            );
                                        }),
                                        on_drag_update: EventHandler::new(move |point| {
                                            update_virtual_text_drag(
                                                drag_selection_for_update,
                                                caret_for_update,
                                                selection_for_update,
                                                point,
                                            );
                                        }),
                                        on_context_menu: EventHandler::new(move |menu| {
                                            let mounted = mounted_for_context_menu.peek().clone();
                                            spawn(async move {
                                                show_context_menu(
                                                    context_menu_for_open,
                                                    context_menu_closing_for_open,
                                                    context_menu_generation_for_open,
                                                    virtual_text_context_menu_from_client_position(
                                                        menu,
                                                        mounted,
                                                    )
                                                    .await,
                                                );
                                            });
                                        })
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(menu) = context_menu_snapshot {
                    div {
                        class: if context_menu_closing_snapshot { "virtual-text-context-menu closing" } else { "virtual-text-context-menu" },
                        role: "menu",
                        style: "left: {menu.x}px; top: {menu.y}px",
                        onmousedown: move |event| {
                            event.prevent_default();
                            event.stop_propagation();
                        },
                        button {
                            r#type: "button",
                            role: "menuitem",
                            disabled: selection_snapshot.is_none(),
                            onclick: {
                                let buffer = buffer.clone();
                                move |_| {
                                    copy_virtual_text_selection_to_clipboard(&buffer, *selection.read());
                                    dismiss_context_menu(
                                        context_menu,
                                        context_menu_closing,
                                        context_menu_generation,
                                    );
                                    focus_virtual_text_input_layer();
                                }
                            },
                            {i18n.t("editor-context-copy")}
                        }
                        button {
                            r#type: "button",
                            role: "menuitem",
                            disabled: selection_snapshot.is_none(),
                            onclick: {
                                let buffer = buffer.clone();
                                move |_| {
                                    cut_virtual_text_selection_to_clipboard(
                                        &buffer,
                                        caret,
                                        selection,
                                        on_change,
                                    );
                                    dismiss_context_menu(
                                        context_menu,
                                        context_menu_closing,
                                        context_menu_generation,
                                    );
                                }
                            },
                            {i18n.t("editor-context-cut")}
                        }
                        button {
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                dismiss_context_menu(
                                    context_menu,
                                    context_menu_closing,
                                    context_menu_generation,
                                );
                                paste_virtual_text_from_clipboard();
                            },
                            {i18n.t("editor-context-paste")}
                        }
                        div { class: "virtual-text-context-menu-separator", role: "separator" }
                        button {
                            r#type: "button",
                            role: "menuitem",
                            onclick: {
                                let buffer = buffer.clone();
                                move |_| {
                                    select_all_virtual_text(&buffer, caret, selection);
                                    dismiss_context_menu(
                                        context_menu,
                                        context_menu_closing,
                                        context_menu_generation,
                                    );
                                }
                            },
                            {i18n.t("editor-context-select-all")}
                        }
                    }
                }
        }
    }
}

fn update_text_buffer_viewport_size(
    viewport_height: Signal<usize>,
    viewport_width: Signal<usize>,
    height: f64,
    width: f64,
) {
    update_text_buffer_viewport_height(viewport_height, height);
    update_text_buffer_viewport_width(viewport_width, width);
}

async fn virtual_text_context_menu_from_client_position(
    menu: VirtualTextContextMenu,
    mounted: Option<MountedEvent>,
) -> VirtualTextContextMenu {
    let Some(event) = mounted else {
        return menu;
    };
    let Ok(rect) = event.get_client_rect().await else {
        return menu;
    };
    VirtualTextContextMenu {
        x: (menu.x as f64 - rect.origin.x).round().max(0.0) as i32,
        y: (menu.y as f64 - rect.origin.y).round().max(0.0) as i32,
    }
}

fn update_text_buffer_viewport_height(mut viewport_height: Signal<usize>, height: f64) {
    let next = measured_text_viewport_height(height);
    if next != *viewport_height.read() {
        viewport_height.set(next);
    }
}

fn update_text_buffer_viewport_width(mut viewport_width: Signal<usize>, width: f64) {
    let next = measured_text_viewport_width(width);
    if next != *viewport_width.read() {
        viewport_width.set(next);
    }
}

fn measured_text_viewport_width(width: f64) -> usize {
    if width.is_finite() && width > 0.0 {
        let measured_width = width.ceil().min(usize::MAX as f64) as usize;
        text_wrap_width_bucket(measured_width)
    } else {
        TEXT_DEFAULT_VIEWPORT_WIDTH
    }
}

fn text_wrap_width_bucket(viewport_width: usize) -> usize {
    text_wrap_chars_per_row(viewport_width)
        .saturating_mul(TEXT_WRAP_CHAR_WIDTH)
        .saturating_add(TEXT_WRAP_LINE_NUMBER_WIDTH)
        .saturating_add(TEXT_WRAP_HORIZONTAL_PADDING)
}

fn focus_editor_find_input() {
    dioxus::document::eval(
        r#"
        (() => {
            const focusInput = () => {
                const input = document.getElementById("rton-editor-find-input");
                if (!input) return false;
                try {
                    input.focus({ preventScroll: true });
                } catch (_error) {
                    input.focus();
                }
                input.select?.();
                return true;
            };
            requestAnimationFrame(() => {
                if (!focusInput()) {
                    requestAnimationFrame(focusInput);
                }
            });
        })();
        "#,
    );
}

fn text_buffer_visible_lines(
    buffer: &TextBuffer,
    scroll_top: f64,
    virtual_scroll: Option<crate::domain::virtual_scroll::TextVirtualScroll>,
    wrap_layout: Option<&TextWrapLayout>,
    wrapped_scroll: Option<WrappedTextScroll>,
) -> Vec<VisibleTextLine> {
    if let (Some(layout), Some(scroll)) = (wrap_layout, wrapped_scroll) {
        return wrapped_text_buffer_visible_lines(buffer, layout, scroll);
    }

    let Some(virtual_scroll) = virtual_scroll else {
        return Vec::new();
    };

    fixed_text_buffer_visible_lines(
        buffer,
        virtual_scroll.start_row,
        virtual_scroll.end_row,
        scroll_top,
        virtual_scroll,
    )
}

fn fixed_text_buffer_visible_lines(
    buffer: &TextBuffer,
    start_row: usize,
    end_row: usize,
    scroll_top: f64,
    virtual_scroll: crate::domain::virtual_scroll::TextVirtualScroll,
) -> Vec<VisibleTextLine> {
    (start_row..end_row)
        .filter_map(|row_index| {
            let line_text = buffer.line_text(row_index)?;
            Some(VisibleTextLine {
                top: text_virtual_row_top(row_index, scroll_top, virtual_scroll),
                height: TEXT_ROW_HEIGHT,
                line_index: row_index,
                line_number: row_index + 1,
                line_text,
            })
        })
        .collect()
}

fn wrapped_text_buffer_visible_lines(
    buffer: &TextBuffer,
    layout: &TextWrapLayout,
    scroll: WrappedTextScroll,
) -> Vec<VisibleTextLine> {
    (scroll.start_row..scroll.end_row)
        .filter_map(|row_index| {
            let line_top = *layout.line_tops.get(row_index)?;
            let line_height = *layout.line_heights.get(row_index)?;
            let top = wrapped_text_row_top(line_top, scroll);
            let line_text = buffer.line_text(row_index)?;
            Some(VisibleTextLine {
                top,
                height: line_height,
                line_index: row_index,
                line_number: row_index + 1,
                line_text,
            })
        })
        .collect()
}

#[cfg(test)]
fn text_wrap_layout(buffer: &TextBuffer, viewport_width: usize) -> TextWrapLayout {
    let line_count = buffer.line_count();
    let mut builder = TextWrapLayoutBuilder::new(line_count, viewport_width);
    for row_index in 0..line_count {
        builder.push_line(buffer.line_byte_len(row_index).unwrap_or(0));
    }
    builder.finish()
}

async fn responsive_text_wrap_layout(
    buffer: IdentityArc<TextBuffer>,
    viewport_width: usize,
    generation_signal: Signal<u64>,
    generation: u64,
) -> Option<TextWrapLayout> {
    let line_count = buffer.line_count();
    let mut builder = TextWrapLayoutBuilder::new(line_count, viewport_width);
    for row_index in 0..line_count {
        if row_index > 0 && row_index % TEXT_WRAP_LAYOUT_BATCH_LINES == 0 {
            platform::sleep_ms(0).await;
            if *generation_signal.peek() != generation {
                return None;
            }
        }
        builder.push_line(buffer.line_byte_len(row_index).unwrap_or(0));
    }
    Some(builder.finish())
}

fn text_wrap_chars_per_row(viewport_width: usize) -> usize {
    viewport_width
        .saturating_sub(TEXT_WRAP_LINE_NUMBER_WIDTH)
        .saturating_sub(TEXT_WRAP_HORIZONTAL_PADDING)
        .checked_div(TEXT_WRAP_CHAR_WIDTH)
        .unwrap_or(1)
        .max(1)
}

fn wrapped_text_scroll(
    layout: &TextWrapLayout,
    scroll_top: f64,
    viewport_height: usize,
) -> WrappedTextScroll {
    let line_count = layout.line_tops.len();
    let content_height = layout.logical_height.min(TEXT_MAX_VIRTUAL_SCROLL_HEIGHT);
    if line_count == 0 {
        return WrappedTextScroll {
            content_height,
            logical_scroll_top: 0,
            display_scroll_top: 0,
            start_row: 0,
            end_row: 0,
            scaled: false,
        };
    }

    let safe_scroll_top = safe_text_scroll_top(scroll_top);
    let scaled = layout.logical_height > TEXT_MAX_VIRTUAL_SCROLL_HEIGHT;
    let display_scroll_top = safe_scroll_top
        .round()
        .min(content_height.saturating_sub(viewport_height) as f64)
        .max(0.0) as usize;
    let logical_scroll_top = if scaled {
        let display_scrollable = content_height.saturating_sub(viewport_height).max(1);
        let logical_scrollable = layout.logical_height.saturating_sub(viewport_height).max(1);
        let ratio = (safe_scroll_top / display_scrollable as f64).clamp(0.0, 1.0);
        (ratio * logical_scrollable as f64).round() as usize
    } else {
        safe_scroll_top
            .round()
            .min(layout.logical_height.saturating_sub(viewport_height) as f64)
            .max(0.0) as usize
    };
    let overscan_px = TEXT_OVERSCAN_ROWS.saturating_mul(TEXT_ROW_HEIGHT);
    let start_px = logical_scroll_top.saturating_sub(overscan_px);
    let end_px = logical_scroll_top
        .saturating_add(viewport_height)
        .saturating_add(overscan_px)
        .min(layout.logical_height);
    let start_row = line_for_logical_top(&layout.line_tops, start_px);
    let end_row = layout
        .line_tops
        .partition_point(|top| *top < end_px)
        .saturating_add(1)
        .min(line_count);

    WrappedTextScroll {
        content_height,
        logical_scroll_top,
        display_scroll_top,
        start_row,
        end_row,
        scaled,
    }
}

fn line_for_logical_top(line_tops: &[usize], logical_top: usize) -> usize {
    line_tops
        .partition_point(|top| *top <= logical_top)
        .saturating_sub(1)
}

fn wrapped_text_row_top(line_top: usize, scroll: WrappedTextScroll) -> i64 {
    if !scroll.scaled {
        return line_top.min(i64::MAX as usize) as i64;
    }
    let top =
        scroll.display_scroll_top as i128 + line_top as i128 - scroll.logical_scroll_top as i128;
    top.clamp(0, i64::MAX as i128) as i64
}

fn safe_text_scroll_top(scroll_top: f64) -> f64 {
    if scroll_top.is_finite() && scroll_top > 0.0 {
        scroll_top
    } else {
        0.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VirtualTextLinePiece {
    Text(String),
    Selected(String),
    Caret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VirtualTextDeleteDirection {
    Backward,
    Forward,
}

#[component]
fn VirtualTextLineView(
    line_index: usize,
    line_number: usize,
    line_text: String,
    caret: VirtualTextPoint,
    selection: Option<VirtualTextSelection>,
    on_drag_start: EventHandler<(VirtualTextPoint, bool)>,
    on_drag_update: EventHandler<VirtualTextPoint>,
    on_context_menu: EventHandler<VirtualTextContextMenu>,
) -> Element {
    let line_text_for_drag_start = line_text.clone();
    let line_text_for_drag_update = line_text.clone();
    let line_text_for_drag_enter = line_text.clone();
    let pieces = virtual_text_line_pieces(selection, caret, line_index, &line_text);
    let line_class = if selection.is_none() && caret.line_index == line_index {
        "virtual-text-line-view active-line"
    } else {
        "virtual-text-line-view"
    };

    rsx! {
        span {
            class: line_class,
            "data-line-number": "{line_number}",
            onmousedown: move |event| {
                if !is_primary_mouse_button(&event) {
                    focus_virtual_text_input_layer();
                    return;
                }
                event.prevent_default();
                focus_virtual_text_input_layer();
                on_drag_start.call((
                    VirtualTextPoint {
                        line_index,
                        column_utf16: virtual_text_click_column_utf16(
                            event.element_coordinates().x,
                            &line_text_for_drag_start,
                        ),
                    },
                    event.modifiers().shift(),
                ));
            },
            oncontextmenu: move |event| {
                event.prevent_default();
                focus_virtual_text_input_layer();
                let coordinates = event.client_coordinates();
                on_context_menu.call(VirtualTextContextMenu {
                    x: coordinates.x.round() as i32,
                    y: coordinates.y.round() as i32,
                });
            },
            onmousemove: move |event| {
                on_drag_update.call(VirtualTextPoint {
                    line_index,
                    column_utf16: virtual_text_click_column_utf16(
                        event.element_coordinates().x,
                        &line_text_for_drag_update,
                    ),
                });
            },
            onmouseenter: move |event| {
                on_drag_update.call(VirtualTextPoint {
                    line_index,
                    column_utf16: virtual_text_click_column_utf16(
                        event.element_coordinates().x,
                        &line_text_for_drag_enter,
                    ),
                });
            },
            for (piece_index, piece) in pieces.into_iter().enumerate() {
                match piece {
                    VirtualTextLinePiece::Text(text) => rsx! {
                        span { key: "{piece_index}", "{text}" }
                    },
                    VirtualTextLinePiece::Selected(text) => rsx! {
                        span { key: "{piece_index}", class: "virtual-text-selection-fragment", "{text}" }
                    },
                    VirtualTextLinePiece::Caret => rsx! {
                        span { key: "{piece_index}", class: "virtual-text-caret", aria_hidden: "true" }
                    },
                }
            }
        }
    }
}

fn start_virtual_text_drag(
    mut drag_selection: Signal<Option<VirtualTextDrag>>,
    mut caret: Signal<VirtualTextPoint>,
    mut selection: Signal<Option<VirtualTextSelection>>,
    point: VirtualTextPoint,
    extend: bool,
) {
    let anchor = if extend {
        let current_selection = *selection.read();
        current_selection
            .map(virtual_text_selection_anchor)
            .unwrap_or(*caret.read())
    } else {
        point
    };
    drag_selection.set(Some(VirtualTextDrag {
        anchor,
        active: false,
    }));
    caret.set(point);
    selection.set(if extend {
        virtual_text_selection_from_distinct_points(anchor, point)
    } else {
        None
    });
}

fn update_virtual_text_drag(
    mut drag_selection: Signal<Option<VirtualTextDrag>>,
    mut caret: Signal<VirtualTextPoint>,
    mut selection: Signal<Option<VirtualTextSelection>>,
    point: VirtualTextPoint,
) {
    let Some(mut drag) = *drag_selection.read() else {
        return;
    };
    if drag.anchor == point {
        return;
    }

    drag.active = true;
    drag_selection.set(Some(drag));
    caret.set(point);
    selection.set(virtual_text_selection_from_distinct_points(
        drag.anchor,
        point,
    ));
}

fn finish_virtual_text_drag(mut drag_selection: Signal<Option<VirtualTextDrag>>) {
    drag_selection.set(None);
}

fn is_primary_mouse_button(event: &MouseEvent) -> bool {
    matches!(event.trigger_button(), None | Some(MouseButton::Primary))
}

fn virtual_text_click_column_utf16(x: f64, line_text: &str) -> usize {
    const LINE_PADDING_LEFT: f64 = 12.0;
    const MONOSPACE_CHAR_WIDTH: f64 = 7.8;

    if !x.is_finite() {
        return 0;
    }

    let column = ((x - LINE_PADDING_LEFT) / MONOSPACE_CHAR_WIDTH).round();
    column
        .max(0.0)
        .min(virtual_text_utf16_len(line_text) as f64) as usize
}

fn focus_virtual_text_input_layer() {
    dioxus::document::eval(
        r#"
        (() => {
            const focusInput = () => {
                const input = document.getElementById("rton-virtual-text-input-sink");
                if (!input) return false;
                input.value = "";
                try {
                    input.focus({ preventScroll: true });
                } catch (_error) {
                    input.focus();
                }
                input.setSelectionRange?.(0, 0);
                return true;
            };
            if (!focusInput()) {
                requestAnimationFrame(focusInput);
            }
        })();
        "#,
    );
}

fn scroll_virtual_text_to_target(target: TextJumpTarget) {
    let line = target.line.max(1);
    let column = target.column;
    let line_count = target.line_count.max(line);
    let focus = target.focus;
    let row_height = TEXT_ROW_HEIGHT;
    let max_scroll_height = TEXT_MAX_VIRTUAL_SCROLL_HEIGHT;
    dioxus::document::eval(&format!(
        r#"
        (() => {{
            const targetLine = {line};
            const targetColumn = {column};
            const rowCount = Math.max(1, {line_count});
            const shouldFocus = {focus};
            const fallbackRowHeight = {row_height};
            const maxScrollHeight = {max_scroll_height};
            const virtualText = document.querySelector('.virtual-text-preview-content');
            if (!virtualText) return;

            const rowHeight = Number.parseFloat(getComputedStyle(virtualText).getPropertyValue('--virtual-text-row-height')) || fallbackRowHeight;
            const viewportRows = Math.max(1, Math.ceil(virtualText.clientHeight / rowHeight) || 1);
            const rowIndex = Math.max(0, Math.min(targetLine - 1, rowCount - 1));
            const maxStartRow = Math.max(0, rowCount - viewportRows);
            const targetStartRow = Math.max(0, Math.min(maxStartRow, rowIndex - Math.floor(viewportRows / 2)));
            const logicalHeight = rowCount * rowHeight;
            let targetScrollTop = 0;
            if (logicalHeight <= maxScrollHeight) {{
                targetScrollTop = targetStartRow * rowHeight;
            }} else {{
                const viewportContentHeight = viewportRows * rowHeight;
                const scrollableHeight = Math.max(1, maxScrollHeight - viewportContentHeight);
                const scaledMaxStartRow = Math.max(1, rowCount - viewportRows);
                targetScrollTop = (targetStartRow / scaledMaxStartRow) * scrollableHeight;
            }}

            virtualText.scrollTop = Math.max(0, targetScrollTop);
            virtualText.scrollLeft = Math.max(0, targetColumn * 8 - virtualText.clientWidth / 2);
            if (!shouldFocus) return;
            requestAnimationFrame(() => {{
                const input = document.getElementById("rton-virtual-text-input-sink");
                if (!input) return;
                try {{
                    input.focus({{ preventScroll: true }});
                }} catch (_error) {{
                    input.focus();
                }}
                input.setSelectionRange?.(0, 0);
            }});
        }})();
        "#
    ));
}

fn virtual_text_selection_delete_replacement(
    selection: VirtualTextSelection,
) -> TextRangeReplacement {
    virtual_text_selection_replacement(selection, String::new())
}

fn virtual_text_selection_replacement(
    selection: VirtualTextSelection,
    replacement: String,
) -> TextRangeReplacement {
    TextRangeReplacement {
        start_line: selection.start_line,
        start_column_utf16: selection.start_column_utf16,
        end_line: selection.end_line,
        end_column_utf16: selection.end_column_utf16,
        replacement,
    }
}

fn virtual_text_caret_replacement(
    caret: VirtualTextPoint,
    replacement: String,
) -> TextRangeReplacement {
    TextRangeReplacement {
        start_line: caret.line_index,
        start_column_utf16: caret.column_utf16,
        end_line: caret.line_index,
        end_column_utf16: caret.column_utf16,
        replacement,
    }
}

fn replace_virtual_text_selection_or_caret(
    buffer: &TextBuffer,
    caret: Signal<VirtualTextPoint>,
    selection: Signal<Option<VirtualTextSelection>>,
    on_change: EventHandler<TextRangeReplacement>,
    replacement_text: String,
) {
    let current_selection = *selection.read();
    let replacement = current_selection
        .map(|selection| virtual_text_selection_replacement(selection, replacement_text.clone()))
        .unwrap_or_else(|| virtual_text_caret_replacement(*caret.read(), replacement_text));
    commit_virtual_text_replacement(buffer, caret, selection, on_change, replacement);
}

fn commit_virtual_text_replacement(
    buffer: &TextBuffer,
    mut caret: Signal<VirtualTextPoint>,
    mut selection: Signal<Option<VirtualTextSelection>>,
    on_change: EventHandler<TextRangeReplacement>,
    replacement: TextRangeReplacement,
) {
    if virtual_text_replacement_is_noop(&replacement) {
        return;
    }
    let next_caret = virtual_text_caret_after_replacement(&replacement);
    caret.set(clamp_virtual_text_point(next_caret, buffer));
    selection.set(None);
    on_change.call(replacement);
    focus_virtual_text_input_layer();
}

fn virtual_text_replacement_is_noop(replacement: &TextRangeReplacement) -> bool {
    replacement.replacement.is_empty()
        && replacement.start_line == replacement.end_line
        && replacement.start_column_utf16 == replacement.end_column_utf16
}

fn virtual_text_caret_after_replacement(replacement: &TextRangeReplacement) -> VirtualTextPoint {
    let start = virtual_text_replacement_start_point(replacement);
    if replacement.replacement.is_empty() {
        return start;
    }
    let newline_count = replacement
        .replacement
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    if newline_count == 0 {
        return VirtualTextPoint {
            line_index: start.line_index,
            column_utf16: start
                .column_utf16
                .saturating_add(virtual_text_utf16_len(&replacement.replacement)),
        };
    }
    let tail = replacement
        .replacement
        .rsplit_once('\n')
        .map(|(_, tail)| tail)
        .unwrap_or_default();
    VirtualTextPoint {
        line_index: start.line_index.saturating_add(newline_count),
        column_utf16: virtual_text_utf16_len(tail.strip_suffix('\r').unwrap_or(tail)),
    }
}

fn virtual_text_replacement_start_point(replacement: &TextRangeReplacement) -> VirtualTextPoint {
    let start = (replacement.start_line, replacement.start_column_utf16);
    let end = (replacement.end_line, replacement.end_column_utf16);
    if start <= end {
        VirtualTextPoint {
            line_index: replacement.start_line,
            column_utf16: replacement.start_column_utf16,
        }
    } else {
        VirtualTextPoint {
            line_index: replacement.end_line,
            column_utf16: replacement.end_column_utf16,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_virtual_text_key(
    event: KeyboardEvent,
    buffer: &TextBuffer,
    caret: Signal<VirtualTextPoint>,
    mut selection: Signal<Option<VirtualTextSelection>>,
    mut input_sink_value: Signal<String>,
    on_change: EventHandler<TextRangeReplacement>,
    on_undo: EventHandler<()>,
    on_redo: EventHandler<()>,
    on_search_visible_change: EventHandler<bool>,
) {
    let key = event.key().to_string();
    let modifiers = event.modifiers();
    let ctrl_or_meta = modifiers.ctrl() || modifiers.meta();
    let shift = modifiers.shift();

    if ctrl_or_meta && key.eq_ignore_ascii_case("f") {
        event.prevent_default();
        on_search_visible_change.call(true);
        focus_editor_find_input();
        return;
    }
    if ctrl_or_meta && key.eq_ignore_ascii_case("z") {
        event.prevent_default();
        if shift {
            on_redo.call(());
        } else {
            on_undo.call(());
        }
        return;
    }
    if ctrl_or_meta && key.eq_ignore_ascii_case("y") {
        event.prevent_default();
        on_redo.call(());
        return;
    }
    if ctrl_or_meta && key.eq_ignore_ascii_case("a") {
        event.prevent_default();
        select_all_virtual_text(buffer, caret, selection);
        return;
    }
    if ctrl_or_meta && key.eq_ignore_ascii_case("c") {
        event.prevent_default();
        copy_virtual_text_selection_to_clipboard(buffer, *selection.read());
        return;
    }
    if ctrl_or_meta && key.eq_ignore_ascii_case("x") {
        event.prevent_default();
        cut_virtual_text_selection_to_clipboard(buffer, caret, selection, on_change);
        return;
    }
    if ctrl_or_meta && key.eq_ignore_ascii_case("v") {
        input_sink_value.set(String::new());
        return;
    }
    if ctrl_or_meta || modifiers.alt() {
        return;
    }

    match key.as_str() {
        "Backspace" => {
            event.prevent_default();
            delete_virtual_text_selection_or_adjacent(
                buffer,
                caret,
                selection,
                on_change,
                VirtualTextDeleteDirection::Backward,
            );
        }
        "Delete" => {
            event.prevent_default();
            delete_virtual_text_selection_or_adjacent(
                buffer,
                caret,
                selection,
                on_change,
                VirtualTextDeleteDirection::Forward,
            );
        }
        "Enter" => {
            event.prevent_default();
            replace_virtual_text_selection_or_caret(
                buffer,
                caret,
                selection,
                on_change,
                "\n".to_string(),
            );
        }
        "Tab" => {
            event.prevent_default();
            replace_virtual_text_selection_or_caret(
                buffer,
                caret,
                selection,
                on_change,
                "\t".to_string(),
            );
        }
        "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" => {
            event.prevent_default();
            move_virtual_text_caret(buffer, caret, selection, key.as_str(), shift);
        }
        "Escape" => selection.set(None),
        _ => {}
    }
}

fn delete_virtual_text_selection_or_adjacent(
    buffer: &TextBuffer,
    caret: Signal<VirtualTextPoint>,
    selection: Signal<Option<VirtualTextSelection>>,
    on_change: EventHandler<TextRangeReplacement>,
    direction: VirtualTextDeleteDirection,
) {
    let replacement = if let Some(selection) = *selection.read() {
        virtual_text_selection_delete_replacement(selection)
    } else {
        let caret_point = clamp_virtual_text_point(*caret.read(), buffer);
        let Some((start, end)) = virtual_text_adjacent_delete_range(buffer, caret_point, direction)
        else {
            return;
        };
        TextRangeReplacement {
            start_line: start.line_index,
            start_column_utf16: start.column_utf16,
            end_line: end.line_index,
            end_column_utf16: end.column_utf16,
            replacement: String::new(),
        }
    };
    commit_virtual_text_replacement(buffer, caret, selection, on_change, replacement);
}

fn virtual_text_adjacent_delete_range(
    buffer: &TextBuffer,
    caret: VirtualTextPoint,
    direction: VirtualTextDeleteDirection,
) -> Option<(VirtualTextPoint, VirtualTextPoint)> {
    match direction {
        VirtualTextDeleteDirection::Backward => {
            let previous = virtual_text_previous_point(buffer, caret)?;
            Some((previous, caret))
        }
        VirtualTextDeleteDirection::Forward => {
            let next = virtual_text_next_point(buffer, caret)?;
            Some((caret, next))
        }
    }
}

fn move_virtual_text_caret(
    buffer: &TextBuffer,
    mut caret: Signal<VirtualTextPoint>,
    mut selection: Signal<Option<VirtualTextSelection>>,
    key: &str,
    extend: bool,
) {
    let current_caret = clamp_virtual_text_point(*caret.read(), buffer);
    let current_selection = *selection.read();
    if !extend && let Some(current_selection) = current_selection {
        let collapsed = match key {
            "ArrowLeft" | "ArrowUp" | "Home" => {
                let (start, _, _, _) = normalized_virtual_text_selection_points(current_selection);
                start
            }
            "ArrowRight" | "ArrowDown" | "End" => {
                let (_, _, end, _) = normalized_virtual_text_selection_points(current_selection);
                end
            }
            _ => current_caret,
        };
        caret.set(clamp_virtual_text_point(collapsed, buffer));
        selection.set(None);
        focus_virtual_text_input_layer();
        return;
    }

    let next = virtual_text_moved_point(buffer, current_caret, key);
    if extend {
        let current_selection = *selection.read();
        let anchor = current_selection
            .map(virtual_text_selection_anchor)
            .unwrap_or(current_caret);
        caret.set(next);
        selection.set(virtual_text_selection_from_distinct_points(anchor, next));
    } else {
        caret.set(next);
        selection.set(None);
    }
    focus_virtual_text_input_layer();
}

fn virtual_text_moved_point(
    buffer: &TextBuffer,
    point: VirtualTextPoint,
    key: &str,
) -> VirtualTextPoint {
    match key {
        "ArrowLeft" => virtual_text_previous_point(buffer, point).unwrap_or(point),
        "ArrowRight" => virtual_text_next_point(buffer, point).unwrap_or(point),
        "ArrowUp" => clamp_virtual_text_point(
            VirtualTextPoint {
                line_index: point.line_index.saturating_sub(1),
                column_utf16: point.column_utf16,
            },
            buffer,
        ),
        "ArrowDown" => clamp_virtual_text_point(
            VirtualTextPoint {
                line_index: point.line_index.saturating_add(1),
                column_utf16: point.column_utf16,
            },
            buffer,
        ),
        "Home" => VirtualTextPoint {
            line_index: point.line_index,
            column_utf16: 0,
        },
        "End" => VirtualTextPoint {
            line_index: point.line_index,
            column_utf16: virtual_text_line_utf16_len(buffer, point.line_index),
        },
        _ => point,
    }
}

fn virtual_text_previous_point(
    buffer: &TextBuffer,
    point: VirtualTextPoint,
) -> Option<VirtualTextPoint> {
    let point = clamp_virtual_text_point(point, buffer);
    if point.column_utf16 > 0 {
        let line = buffer.line_text(point.line_index)?;
        return Some(VirtualTextPoint {
            line_index: point.line_index,
            column_utf16: virtual_text_previous_utf16_column(&line, point.column_utf16),
        });
    }
    if point.line_index == 0 {
        return None;
    }
    let previous_line = point.line_index - 1;
    Some(VirtualTextPoint {
        line_index: previous_line,
        column_utf16: virtual_text_line_utf16_len(buffer, previous_line),
    })
}

fn virtual_text_next_point(
    buffer: &TextBuffer,
    point: VirtualTextPoint,
) -> Option<VirtualTextPoint> {
    let point = clamp_virtual_text_point(point, buffer);
    let line_len = virtual_text_line_utf16_len(buffer, point.line_index);
    if point.column_utf16 < line_len {
        let line = buffer.line_text(point.line_index)?;
        return Some(VirtualTextPoint {
            line_index: point.line_index,
            column_utf16: virtual_text_next_utf16_column(&line, point.column_utf16),
        });
    }
    if point.line_index + 1 >= buffer.line_count() {
        return None;
    }
    Some(VirtualTextPoint {
        line_index: point.line_index + 1,
        column_utf16: 0,
    })
}

fn virtual_text_previous_utf16_column(text: &str, column_utf16: usize) -> usize {
    let mut current = 0_usize;
    for ch in text.chars() {
        let next = current.saturating_add(ch.len_utf16());
        if next >= column_utf16 {
            return current;
        }
        current = next;
    }
    current
}

fn virtual_text_next_utf16_column(text: &str, column_utf16: usize) -> usize {
    let mut current = 0_usize;
    for ch in text.chars() {
        let next = current.saturating_add(ch.len_utf16());
        if current >= column_utf16 || next > column_utf16 {
            return next;
        }
        current = next;
    }
    current
}

fn select_all_virtual_text(
    buffer: &TextBuffer,
    mut caret: Signal<VirtualTextPoint>,
    mut selection: Signal<Option<VirtualTextSelection>>,
) {
    let start = VirtualTextPoint {
        line_index: 0,
        column_utf16: 0,
    };
    let end = virtual_text_document_end_point(buffer);
    caret.set(end);
    selection.set(virtual_text_selection_from_distinct_points(start, end));
    focus_virtual_text_input_layer();
}

fn copy_virtual_text_selection_to_clipboard(
    buffer: &TextBuffer,
    selection: Option<VirtualTextSelection>,
) {
    let Some(text) = selection.and_then(|selection| virtual_text_selected_text(buffer, selection))
    else {
        return;
    };
    write_virtual_text_clipboard(&text);
}

#[cfg(target_arch = "wasm32")]
fn virtual_text_clipboard_event_text(event: &dioxus_html::ClipboardEvent) -> Option<String> {
    use wasm_bindgen::JsCast;

    let web_event = event.data.downcast::<web_sys::Event>()?;
    let clipboard_event = web_event.dyn_ref::<web_sys::ClipboardEvent>()?;
    clipboard_event
        .clipboard_data()?
        .get_data("text/plain")
        .ok()
        .filter(|text| !text.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn virtual_text_clipboard_event_text(_event: &dioxus_html::ClipboardEvent) -> Option<String> {
    None
}

fn paste_virtual_text_from_clipboard() {
    dioxus::document::eval(
        r#"
        (() => {
            const input = document.getElementById("rton-virtual-text-input-sink");
            if (!input) return;
            try {
                input.focus({ preventScroll: true });
            } catch (_error) {
                input.focus();
            }

            const dispatchText = (text) => {
                if (typeof text !== "string" || text.length === 0) return;
                input.value = text;
                let event;
                try {
                    event = new InputEvent("input", {
                        bubbles: true,
                        data: text,
                        inputType: "insertFromPaste",
                    });
                } catch (_error) {
                    event = new Event("input", { bubbles: true });
                }
                input.dispatchEvent(event);
            };

            if (navigator.clipboard?.readText) {
                navigator.clipboard.readText().then(dispatchText).catch(() => {
                    try {
                        document.execCommand?.("paste");
                    } catch (_error) {}
                });
            } else {
                try {
                    document.execCommand?.("paste");
                } catch (_error) {}
            }
        })();
        "#,
    );
}

fn cut_virtual_text_selection_to_clipboard(
    buffer: &TextBuffer,
    caret: Signal<VirtualTextPoint>,
    selection: Signal<Option<VirtualTextSelection>>,
    on_change: EventHandler<TextRangeReplacement>,
) {
    let Some(current_selection) = *selection.read() else {
        return;
    };
    let Some(text) = virtual_text_selected_text(buffer, current_selection) else {
        return;
    };
    write_virtual_text_clipboard(&text);
    commit_virtual_text_replacement(
        buffer,
        caret,
        selection,
        on_change,
        virtual_text_selection_delete_replacement(current_selection),
    );
}

fn write_virtual_text_clipboard(text: &str) {
    let Ok(text_json) = serde_json::to_string(text) else {
        return;
    };
    dioxus::document::eval(&format!(
        r#"
        (() => {{
            const text = {text_json};
            const fallbackCopy = () => {{
                const previous = document.activeElement;
                const textarea = document.createElement("textarea");
                textarea.value = text;
                textarea.setAttribute("readonly", "true");
                textarea.style.position = "fixed";
                textarea.style.left = "-10000px";
                textarea.style.top = "0";
                document.body.appendChild(textarea);
                textarea.focus();
                textarea.select();
                try {{
                    document.execCommand("copy");
                }} catch (_error) {{}}
                textarea.remove();
                previous?.focus?.({{ preventScroll: true }});
            }};
            if (navigator.clipboard?.writeText) {{
                navigator.clipboard.writeText(text).catch(fallbackCopy);
            }} else {{
                fallbackCopy();
            }}
        }})();
        "#
    ));
}

fn virtual_text_line_pieces(
    selection: Option<VirtualTextSelection>,
    caret: VirtualTextPoint,
    line_index: usize,
    line_text: &str,
) -> Vec<VirtualTextLinePiece> {
    if let Some((start, end)) =
        virtual_text_line_selection_columns(selection, line_index, line_text)
    {
        return virtual_text_selected_line_pieces(line_text, start, end);
    }
    if caret.line_index == line_index {
        return virtual_text_caret_line_pieces(line_text, caret.column_utf16);
    }
    if line_text.is_empty() {
        Vec::new()
    } else {
        vec![VirtualTextLinePiece::Text(line_text.to_string())]
    }
}

fn virtual_text_selected_line_pieces(
    line_text: &str,
    start: usize,
    end: usize,
) -> Vec<VirtualTextLinePiece> {
    let mut pieces = Vec::new();
    push_virtual_text_piece(
        &mut pieces,
        VirtualTextLinePiece::Text(line_text[..start].to_string()),
    );
    push_virtual_text_piece(
        &mut pieces,
        VirtualTextLinePiece::Selected(line_text[start..end].to_string()),
    );
    push_virtual_text_piece(
        &mut pieces,
        VirtualTextLinePiece::Text(line_text[end..].to_string()),
    );
    pieces
}

fn virtual_text_caret_line_pieces(
    line_text: &str,
    column_utf16: usize,
) -> Vec<VirtualTextLinePiece> {
    let caret = virtual_text_utf16_column_to_byte(line_text, column_utf16);
    let mut pieces = Vec::new();
    push_virtual_text_piece(
        &mut pieces,
        VirtualTextLinePiece::Text(line_text[..caret].to_string()),
    );
    pieces.push(VirtualTextLinePiece::Caret);
    push_virtual_text_piece(
        &mut pieces,
        VirtualTextLinePiece::Text(line_text[caret..].to_string()),
    );
    pieces
}

fn push_virtual_text_piece(pieces: &mut Vec<VirtualTextLinePiece>, piece: VirtualTextLinePiece) {
    match &piece {
        VirtualTextLinePiece::Text(text) | VirtualTextLinePiece::Selected(text)
            if text.is_empty() => {}
        _ => pieces.push(piece),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::TEXT_ROW_HEIGHT;

    use super::{
        TextRangeReplacement, VirtualTextDeleteDirection, VirtualTextLinePiece, VirtualTextPoint,
        VirtualTextSelection, measured_text_viewport_width, text_wrap_layout,
        virtual_text_adjacent_delete_range, virtual_text_caret_after_replacement,
        virtual_text_line_pieces, virtual_text_line_selection_columns,
        virtual_text_point_from_jump_target, virtual_text_selected_text,
    };
    use crate::domain::TextBuffer;

    #[test]
    fn text_wrap_layout_expands_long_lines_for_narrow_viewports() {
        let text = format!("short\n{}", "a".repeat(200));
        let buffer = TextBuffer::new(text);
        let layout = text_wrap_layout(&buffer, 220);

        assert_eq!(layout.line_tops.as_slice(), &[0, TEXT_ROW_HEIGHT]);
        assert_eq!(layout.line_heights[0], TEXT_ROW_HEIGHT);
        assert!(layout.line_heights[1] > TEXT_ROW_HEIGHT);
        assert!(layout.logical_height > TEXT_ROW_HEIGHT * 2);
    }

    #[test]
    fn text_wrap_layout_covers_documents_above_the_previous_limit() {
        let buffer = TextBuffer::new("line\n".repeat(20_001));
        let layout = text_wrap_layout(&buffer, 320);

        assert_eq!(layout.line_tops.len(), 20_001);
        assert_eq!(layout.line_heights.len(), 20_001);
        assert_eq!(layout.logical_height, TEXT_ROW_HEIGHT * 20_001);
    }

    #[test]
    fn measured_text_viewport_width_is_bucketed_by_wrap_column() {
        assert_eq!(
            measured_text_viewport_width(220.0),
            measured_text_viewport_width(225.0)
        );
        assert_ne!(
            measured_text_viewport_width(220.0),
            measured_text_viewport_width(226.0)
        );
    }

    #[test]
    fn virtual_text_selection_columns_span_middle_line() {
        let selection = Some(VirtualTextSelection {
            start_line: 1,
            start_column_utf16: 2,
            end_line: 3,
            end_column_utf16: 4,
        });

        assert_eq!(
            virtual_text_line_selection_columns(selection, 2, "middle"),
            Some((0, 6))
        );
    }

    #[test]
    fn virtual_text_selection_columns_preserve_utf8_boundaries() {
        let selection = Some(VirtualTextSelection {
            start_line: 0,
            start_column_utf16: 1,
            end_line: 0,
            end_column_utf16: 3,
        });

        assert_eq!(
            virtual_text_line_selection_columns(selection, 0, "a😀b"),
            Some((1, 5))
        );
    }

    #[test]
    fn virtual_text_line_pieces_render_caret_without_native_input() {
        assert_eq!(
            virtual_text_line_pieces(
                None,
                VirtualTextPoint {
                    line_index: 0,
                    column_utf16: 1
                },
                0,
                "ab"
            ),
            vec![
                VirtualTextLinePiece::Text("a".to_string()),
                VirtualTextLinePiece::Caret,
                VirtualTextLinePiece::Text("b".to_string())
            ]
        );
    }

    #[test]
    fn virtual_text_selected_text_crosses_lines() {
        let buffer = TextBuffer::new("one\ntwo\nthree".to_string());
        let selection = VirtualTextSelection {
            start_line: 0,
            start_column_utf16: 1,
            end_line: 2,
            end_column_utf16: 2,
        };

        assert_eq!(
            virtual_text_selected_text(&buffer, selection).as_deref(),
            Some("ne\ntwo\nth")
        );
    }

    #[test]
    fn virtual_text_backspace_range_joins_lines() {
        let buffer = TextBuffer::new("one\ntwo".to_string());
        let caret = VirtualTextPoint {
            line_index: 1,
            column_utf16: 0,
        };

        assert_eq!(
            virtual_text_adjacent_delete_range(
                &buffer,
                caret,
                VirtualTextDeleteDirection::Backward
            ),
            Some((
                VirtualTextPoint {
                    line_index: 0,
                    column_utf16: 3
                },
                caret
            ))
        );
    }

    #[test]
    fn virtual_text_delete_range_uses_utf16_boundaries() {
        let buffer = TextBuffer::new("a😀b".to_string());
        let caret = VirtualTextPoint {
            line_index: 0,
            column_utf16: 1,
        };

        assert_eq!(
            virtual_text_adjacent_delete_range(&buffer, caret, VirtualTextDeleteDirection::Forward),
            Some((
                caret,
                VirtualTextPoint {
                    line_index: 0,
                    column_utf16: 3
                }
            ))
        );
    }

    #[test]
    fn virtual_text_caret_after_multiline_replacement() {
        let replacement = TextRangeReplacement {
            start_line: 2,
            start_column_utf16: 4,
            end_line: 2,
            end_column_utf16: 4,
            replacement: "a\n中文".to_string(),
        };

        assert_eq!(
            virtual_text_caret_after_replacement(&replacement),
            VirtualTextPoint {
                line_index: 3,
                column_utf16: 2
            }
        );
    }

    #[test]
    fn virtual_text_jump_target_converts_byte_column_to_utf16() {
        let buffer = TextBuffer::new("a😀b".to_string());

        assert_eq!(
            virtual_text_point_from_jump_target(&buffer, 1, 5),
            VirtualTextPoint {
                line_index: 0,
                column_utf16: 3
            }
        );
    }
}
