use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdImages, LdPlus, LdRedo2, LdSave, LdSlidersHorizontal, LdTrash2, LdUndo2, LdX,
};

use crate::actions::*;
use crate::i18n::tr;
use crate::state::AppContext;

use super::editor::{FrameSource, InstancePanel, NumberField};
use super::page_actions::PlaybackDock;
use super::primitives::icon;

#[component]
pub fn EditorToolbar(on_properties: EventHandler<()>, properties_open: bool) -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let tab = context.active_tab_snapshot();
    let dirty = tab.as_ref().is_some_and(|tab| tab.is_dirty());
    rsx! {
        header { class: "pam-editor-toolbar",
            div { class: "pam-editor-identity",
                if tab.is_some() {
                    span { class: if dirty { "pam-save-state is-dirty" } else { "pam-save-state" },
                        span { class: "pam-save-state-dot" }
                        {tr(locale, if dirty { "unsaved" } else { "saved" })}
                    }
                }
            }
            div { class: "pam-editor-command-group",
                button { class: "pam-icon-button", title: tr(locale, "new_animation"), aria_label: tr(locale, "new_animation"),
                    onclick: move |_| new_document(context), {icon(LdPlus)}
                }
                if let Some(tab) = tab {
                    span { class: "pam-editor-divider", aria_hidden: "true" }
                    button { class: "pam-icon-button", title: tr(locale, "undo"), aria_label: tr(locale, "undo"),
                        disabled: tab.undo_stack.is_empty(), onclick: move |_| undo(context), {icon(LdUndo2)}
                    }
                    button { class: "pam-icon-button", title: tr(locale, "redo"), aria_label: tr(locale, "redo"),
                        disabled: tab.redo_stack.is_empty(), onclick: move |_| redo(context), {icon(LdRedo2)}
                    }
                    button { class: "pam-icon-button pam-editor-inspector-toggle", title: tr(locale, "properties"), aria_label: tr(locale, "properties"),
                        aria_pressed: properties_open,
                        onclick: move |_| on_properties.call(()), {icon(LdSlidersHorizontal)}
                    }
                    button { class: "pam-button pam-editor-export-sprites",
                        title: tr(locale, "export_all_sprites_hint"), aria_label: tr(locale, "export_all_sprites"),
                        disabled: context.export.read().is_some() || !(0..tab.document.pam.image.len()).any(|index| tab.document.images.get(index).is_some_and(Option::is_some)),
                        onclick: move |_| start_export(context, ExportKind::ImagesZip),
                        {icon(LdImages)} span { {tr(locale, "export_all_sprites")} }
                    }
                    button { class: "pam-button primary pam-editor-save", disabled: context.export.read().is_some(),
                        onclick: move |_| start_export(context, ExportKind::Pam), {icon(LdSave)} {tr(locale, "save_pam")}
                    }
                }
            }
        }
    }
}

#[component]
pub fn EditorInspector(on_close: EventHandler<()>) -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let Some(tab) = context.active_tab_snapshot() else {
        return rsx! {};
    };
    rsx! {
        aside { class: "pam-editor-inspector", aria_label: tr(locale, "properties"),
            header { class: "pam-inspector-heading",
                strong { {tr(locale, "properties")} }
                span { class: "pam-editor-badge", "PAM v{tab.document.pam.version}" }
                button { class: "pam-icon-button pam-editor-inspector-toggle", title: tr(locale, "close_menu"), aria_label: tr(locale, "close_menu"),
                    onclick: move |_| on_close.call(()), {icon(LdX)}
                }
            }
            div { class: "pam-inspector-scroll",
                details { class: "pam-editor-section", open: true,
                    summary { {tr(locale, "document_settings")} }
                    div { class: "pam-editor-fields pam-editor-grid-3",
                        NumberField { label: tr(locale, "width"), value: tab.document.pam.size[0], onchange: move |v: f64| {
                            if v > 0.0 { edit_document(context, move |pam, _, _| pam.size[0] = v); }
                        } }
                        NumberField { label: tr(locale, "height"), value: tab.document.pam.size[1], onchange: move |v: f64| {
                            if v > 0.0 { edit_document(context, move |pam, _, _| pam.size[1] = v); }
                        } }
                        NumberField { label: "FPS", value: tab.document.pam.frame_rate as f64, onchange: move |v: f64| {
                            if (1.0..=255.0).contains(&v) { edit_document(context, move |pam, _, _| pam.frame_rate = v.round() as i32); }
                        } }
                    }
                }
                if let Some(sprite) = tab.active_sprite_info() {
                    details { class: "pam-editor-section", open: true,
                        summary { {tr(locale, "sprite_settings")} }
                        if tab.document.pam.version >= 4 {
                            div { class: "pam-editor-fields pam-editor-sprite-fields",
                                label { {tr(locale, "sprite_name")}
                                    input { value: sprite.name.clone().unwrap_or_default(), onchange: move |e| {
                                        let name = e.value();
                                        edit_document(context, move |pam, key, _| { if let Some(s) = active_sprite_mut(pam, key) { s.name = Some(name); } });
                                    } }
                                }
                                NumberField { label: "FPS", value: sprite.frame_rate.unwrap_or(30.0), onchange: move |v: f64| {
                                    if v > 0.0 { edit_document(context, move |pam, key, _| { if let Some(s) = active_sprite_mut(pam, key) { s.frame_rate = Some(v); } }); }
                                } }
                            }
                        }
                        button { class: "pam-editor-text-button", onclick: move |_| add_sprite(context), {icon(LdPlus)} {tr(locale, "add_sprite")} }
                    }
                    InstancePanel {}
                    if let Some(frame) = sprite.frame.get(tab.current_frame) {
                        details { class: "pam-editor-section", open: true,
                            summary { {tr(locale, "frame_settings")} span { class: "pam-editor-badge", "{tab.current_frame + 1}" } }
                            div { class: "pam-editor-fields",
                                label { {tr(locale, "label")}
                                    input { value: frame.label.clone().unwrap_or_default(), placeholder: tr(locale, "no_label"), onchange: move |e| {
                                        let text = e.value();
                                        edit_document(context, move |pam, key, f| { if let Some(frame) = active_sprite_mut(pam, key).and_then(|s| s.frame.get_mut(f)) { frame.label = (!text.is_empty()).then_some(text); } });
                                    } }
                                }
                                label { class: "pam-editor-check",
                                    input { r#type: "checkbox", checked: frame.stop, onchange: move |e| {
                                        let stop = e.checked();
                                        edit_document(context, move |pam, key, f| { if let Some(frame) = active_sprite_mut(pam, key).and_then(|s| s.frame.get_mut(f)) { frame.stop = stop; } });
                                    } }
                                    {tr(locale, "stop_frame")}
                                }
                            }
                        }
                        for identity in [format!("{}-{}-{}-{:?}", tab.id, tab.document_revision, tab.current_frame, tab.active_sprite)] {
                            FrameSource { key: "{identity}", frame: frame.clone() }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn EditorTimeline() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    use_effect(move || {
        if let Some(tab) = context.active_tab_snapshot() {
            let _frame = tab.current_frame;
            let _ = document::eval(
                r#"requestAnimationFrame(() => {
                const rail = document.querySelector('.pam-editor-frame-rail');
                const frame = rail?.querySelector('[aria-pressed="true"]');
                if (!rail || !frame) return;
                const left = frame.offsetLeft;
                if (left < rail.scrollLeft) rail.scrollLeft = left;
                else if (left + frame.offsetWidth > rail.scrollLeft + rail.clientWidth)
                    rail.scrollLeft = left + frame.offsetWidth - rail.clientWidth;
            });"#,
            );
        }
    });
    let Some(tab) = context.active_tab_snapshot() else {
        return rsx! {};
    };
    let Some(sprite) = tab.active_sprite_info() else {
        return rsx! {};
    };
    let name = sprite
        .name
        .clone()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| tr(locale, "main_timeline").into());
    rsx! {
        section { class: "pam-editor-timeline", aria_label: tr(locale, "timeline"),
            header { class: "pam-timeline-heading",
                strong { {tr(locale, "timeline")} }
                span { class: "pam-timeline-name", title: "{name}", "{name}" }
                span { class: "pam-timeline-legend", span { class: "pam-keyframe-dot" } {tr(locale, "keyframe")} }
                div { class: "pam-editor-command-group",
                    button { class: "pam-icon-button", title: tr(locale, "add_frame"), aria_label: tr(locale, "add_frame"),
                        onclick: move |_| add_frame(context, false), {icon(LdPlus)}
                    }
                    button { class: "pam-icon-button", title: tr(locale, "delete_frame"), aria_label: tr(locale, "delete_frame"),
                        disabled: sprite.frame.len() < 2, onclick: move |_| delete_current_frame(context), {icon(LdTrash2)}
                    }
                }
            }
            div { class: "pam-editor-frame-rail", role: "group", aria_label: tr(locale, "frames"),
                for (index, frame) in sprite.frame.iter().enumerate() {
                    {
                        let active = tab.current_frame == index;
                        let keyframe = !frame.change.is_empty() || !frame.append.is_empty() || !frame.remove.is_empty();
                        let event = frame.stop || frame.label.is_some() || !frame.command.is_empty();
                        rsx! {
                            button { key: "{index}", class: if active { "pam-editor-frame is-active" } else { "pam-editor-frame" },
                                aria_pressed: active, aria_label: format!("{} {}", tr(locale, "frame_settings"), index + 1),
                                title: format!("{} {}{}", tr(locale, "frame_settings"), index + 1, frame.label.as_ref().map(|s| format!(" · {s}")).unwrap_or_default()),
                                onclick: move |_| set_frame(context, index),
                                span { class: if active || index == 0 || (index + 1) % 5 == 0 { "pam-frame-number" } else { "pam-frame-number is-minor" }, "{index + 1}" }
                                span { class: if keyframe { "pam-frame-cell is-keyframe" } else { "pam-frame-cell" },
                                    if keyframe { span { class: "pam-keyframe-dot" } }
                                    if event { span { class: "pam-frame-event" } }
                                }
                            }
                        }
                    }
                }
            }
            PlaybackDock { compact: true }
        }
    }
}
