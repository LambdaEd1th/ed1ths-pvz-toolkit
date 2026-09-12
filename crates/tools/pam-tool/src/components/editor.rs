use crate::actions::*;
use crate::i18n::tr;
use crate::state::{AppContext, Status, Tone};
use dioxus::prelude::*;
use pam_editor_core::{FrameInfo, SpriteKey};

#[component]
pub fn CloseConfirmation() -> Element {
    let mut context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let Some(target) = *context.pending_close.read() else {
        return rsx! {};
    };
    rsx! { div { class: "pam-editor-confirm", role: "alertdialog", aria_modal: "true", aria_label: tr(locale, "discard_question"),
        div { class: "pam-editor-confirm-card",
            p { {tr(locale, "discard_question")} }
            button { class: "pam-button", onclick: move |_| context.pending_close.set(None), {tr(locale, "cancel")} }
            button { class: "pam-button", onclick: move |_| {
                context.pending_close.set(None);
                match target { Some(id) => close_tab_confirmed(context, id), None => clear_tabs_confirmed(context) }
            }, {tr(locale, "discard")} }
        }
    } }
}

#[component]
pub(super) fn NumberField(label: String, value: f64, onchange: EventHandler<f64>) -> Element {
    let display = field_number(value);
    rsx! { label { "{label}" input { r#type: "number", step: "any", value: "{display}", title: "{value}", onchange: move |e| {
        if let Ok(value) = e.value().parse::<f64>() && value.is_finite() { onchange.call(value); }
    } } } }
}

// Display compact values without modifying the underlying PAM until the user edits a field.
fn field_number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else if value.abs() < 0.0001 || value.abs() >= 1_000_000.0 {
        format!("{value:.4e}")
    } else {
        format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into()
    }
}

#[component]
pub(super) fn InstancePanel() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let mut selected = use_signal(|| None::<i32>);
    let mut resource = use_signal(|| 0usize);
    let Some(tab) = context.active_tab_snapshot() else {
        return rsx! {};
    };
    let layers = tab
        .document
        .compiled
        .timeline(tab.active_sprite)
        .and_then(|frames| frames.get(tab.current_frame))
        .cloned()
        .unwrap_or_default();
    let layer = layers
        .iter()
        .find(|layer| Some(layer.index) == selected())
        .or(layers.first())
        .cloned();
    rsx! { details { class: "pam-editor-section pam-editor-instances", open: true,
        summary { {tr(locale, "instance_settings")} }
        div { class: "pam-editor-fields",
            label { {tr(locale, "instances")}
                select { value: layer.as_ref().map(|l| l.index.to_string()).unwrap_or_default(),
                    onchange: move |e| selected.set(e.value().parse().ok()),
                    for layer in &layers {
                        {
                            let name = if layer.is_sprite {
                                tab.document.pam.sprite.get(layer.resource).and_then(|s| s.name.clone())
                            } else {
                                tab.document.pam.image.get(layer.resource).map(|image| image.name.split('|').next().unwrap_or(&image.name).to_string())
                            }.unwrap_or_else(|| layer.resource.to_string());
                            rsx! { option { value: "{layer.index}", "#{layer.index} · {name}" } }
                        }
                    }
                }
            }
        }
        details { class: "pam-instance-add",
            summary { {tr(locale, "add_instance")} }
            div { class: "pam-editor-fields",
            label { {tr(locale, "resources")}
                select { value: "{resource}", onchange: move |e| { if let Ok(v) = e.value().parse() { resource.set(v); } },
                    for (index, image) in tab.document.pam.image.iter().enumerate() {
                        option { value: "{index}", "{image.name.split('|').next().unwrap_or(&image.name)}" }
                    }
                    for (index, sprite) in tab.document.pam.sprite.iter().enumerate() {
                        if tab.active_sprite != SpriteKey::Sprite(index) {
                            option { value: "{index + tab.document.pam.image.len()}", "Sprite #{index}: {sprite.name.clone().unwrap_or_default()}" }
                        }
                    }
                }
            }
            button { class: "pam-button", disabled: tab.document.pam.image.is_empty() && tab.document.pam.sprite.is_empty(), onclick: move |_| {
                let image = resource();
                edit_document(context, move |pam, key, frame_index| {
                    let is_sprite = image >= pam.image.len();
                    let resource = if is_sprite { image - pam.image.len() } else { image };
                    if is_sprite && resource >= pam.sprite.len() { return; }
                    if let Some(sprite) = active_sprite_mut(pam, key) {
                        let index = sprite.frame.iter().flat_map(|f| &f.append).map(|a| a.index).max().unwrap_or(-1) + 1;
                        if let Some(frame) = sprite.frame.get_mut(frame_index) {
                            frame.append.push(pam_editor_core::AddsInfo { index, resource: resource as u32, sprite: is_sprite, additive: false, name: None, preload_frame: 0, time_scale: 1.0 });
                        }
                    }
                });
            }, {tr(locale, "add_instance")} }
            }
        }
        if let Some(layer) = layer.clone() {
            h4 { class: "pam-editor-subheading", {tr(locale, "transform")} }
            div { class: "pam-editor-fields pam-editor-matrix",
                for component in 0..6 {
                    NumberField { label: ["A", "B", "C", "D", "X", "Y"][component], value: layer.transform[component] as f64,
                        onchange: {
                            let layer = layer.clone();
                            move |v| {
                                let layer = layer.clone();
                                edit_document(context, move |pam, key, frame_index| {
                                    if let Some(frame) = active_sprite_mut(pam, key).and_then(|s| s.frame.get_mut(frame_index)) {
                                        let change = instance_keyframe(frame, &layer);
                                        let mut matrix = pam_editor_core::transform_to_matrix(&change.transform).unwrap_or(layer.transform).map(|v| v as f64);
                                        matrix[component] = v;
                                        change.transform = matrix.to_vec();
                                    }
                                });
                            }
                        },
                    }
                }
            }
            h4 { class: "pam-editor-subheading", {tr(locale, "color_multiplier")} }
            div { class: "pam-editor-fields pam-editor-color",
                for channel in 0..4 {
                    NumberField { label: ["R", "G", "B", "Alpha"][channel], value: [layer.color.r, layer.color.g, layer.color.b, layer.color.a][channel] as f64,
                        onchange: {
                            let layer = layer.clone();
                            move |v: f64| {
                                if !(0.0..=1.0).contains(&v) { return; }
                                let layer = layer.clone();
                                edit_document(context, move |pam, key, frame_index| {
                                    if let Some(frame) = active_sprite_mut(pam, key).and_then(|s| s.frame.get_mut(frame_index)) {
                                        let change = instance_keyframe(frame, &layer);
                                        let color = change.color.get_or_insert([layer.color.r as f64, layer.color.g as f64, layer.color.b as f64, layer.color.a as f64]);
                                        color[channel] = v;
                                    }
                                });
                            }
                        },
                    }
                }
            }
            div {
                button { class: "pam-editor-text-button is-danger", onclick: move |_| {
                    let index = layer.index;
                    edit_document(context, move |pam, key, frame_index| {
                        if let Some(sprite) = active_sprite_mut(pam, key) {
                            // Remove this lifetime only, stopping when this slot is reused.
                            for (offset, frame) in sprite.frame.iter_mut().skip(frame_index).enumerate() {
                                if offset > 0 && frame.append.iter().any(|a| a.index == index) { break; }
                                frame.change.retain(|c| c.index != index);
                                if offset == 0 {
                                    frame.append.retain(|a| a.index != index);
                                    if !frame.remove.iter().any(|r| r.index == index) { frame.remove.push(pam_editor_core::RemovesInfo { index }); }
                                }
                            }
                        }
                    });
                }, {tr(locale, "remove_instance")} }
            }
        }
    } }
}

fn instance_keyframe<'a>(
    frame: &'a mut FrameInfo,
    layer: &pam_editor_core::LayerSnapshot,
) -> &'a mut pam_editor_core::MovesInfo {
    let index = frame
        .change
        .iter()
        .rposition(|change| change.index == layer.index)
        .unwrap_or_else(|| {
            frame.change.push(pam_editor_core::MovesInfo {
                index: layer.index,
                transform: layer.transform.map(|v| v as f64).to_vec(),
                color: None,
                source_rectangle: None,
                sprite_frame_number: None,
            });
            frame.change.len() - 1
        });
    &mut frame.change[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pam_editor_core::{Color, LayerSnapshot, MovesInfo, Rectangle};

    #[test]
    fn property_numbers_are_compact_without_hiding_small_values() {
        assert_eq!(field_number(390.0), "390");
        assert_eq!(field_number(0.6509804129600525), "0.651");
        assert_eq!(field_number(-0.0), "0");
        assert_eq!(field_number(0.000001), "1.0000e-6");
    }

    fn layer() -> LayerSnapshot {
        LayerSnapshot {
            index: 7,
            resource: 0,
            is_sprite: false,
            additive: false,
            first_frame: 0,
            time_scale: 1.0,
            preload_frame: 0,
            transform: [1.0, 0.0, 0.0, 1.0, 25.0, -10.0],
            color: Color::WHITE,
            source_rectangle: None,
        }
    }

    #[test]
    fn hold_frame_edit_creates_one_keyframe_with_inherited_transform() {
        let mut frame = FrameInfo::default();
        instance_keyframe(&mut frame, &layer()).color = Some([1.0, 1.0, 1.0, 0.5]);
        instance_keyframe(&mut frame, &layer()).transform[4] = 40.0;
        assert_eq!(frame.change.len(), 1);
        assert_eq!(
            frame.change[0].transform,
            vec![1.0, 0.0, 0.0, 1.0, 40.0, -10.0]
        );
        assert_eq!(frame.change[0].color.unwrap()[3], 0.5);
    }

    #[test]
    fn updating_an_existing_keyframe_preserves_crop_and_sprite_frame() {
        let crop = Rectangle {
            position: [2.0, 3.0],
            size: [10.0, 20.0],
        };
        let mut frame = FrameInfo {
            change: vec![MovesInfo {
                index: 7,
                transform: vec![5.0, 6.0],
                color: None,
                source_rectangle: Some(crop),
                sprite_frame_number: Some(4),
            }],
            ..Default::default()
        };
        instance_keyframe(&mut frame, &layer()).color = Some([0.5, 1.0, 1.0, 1.0]);
        assert_eq!(frame.change[0].source_rectangle, Some(crop));
        assert_eq!(frame.change[0].sprite_frame_number, Some(4));
        assert_eq!(frame.change[0].transform, vec![5.0, 6.0]);
    }
}

#[component]
pub(super) fn FrameSource(frame: FrameInfo) -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let mut source = use_signal(|| serde_json::to_string_pretty(&frame).unwrap_or_default());
    rsx! { details { class: "pam-editor-section pam-editor-source",
        summary { {tr(locale, "frame_source")} }
        p { {tr(locale, "frame_source_hint")} }
        textarea { aria_label: tr(locale, "frame_source"), value: "{source}", oninput: move |e| source.set(e.value()) }
        button { class: "pam-button", onclick: move |_| {
            match serde_json::from_str::<FrameInfo>(&source()) {
                Ok(frame) => edit_document(context, move |pam, key, index| {
                    if let Some(slot) = active_sprite_mut(pam, key).and_then(|s| s.frame.get_mut(index)) { *slot = frame; }
                }),
                Err(error) => context.set_status(Status::new(error.to_string(), Tone::Error)),
            }
        }, {tr(locale, "apply_frame")} }
    } }
}
