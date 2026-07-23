use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdPause, LdPlay, LdSkipBack, LdSkipForward};

use crate::actions::{
    advance_frame, set_autoplay, set_boundary, set_frame, set_frame_range, set_keep_speed,
    set_loop, set_reverse, set_speed, set_speed_factor,
};
use crate::i18n::tr;
use crate::state::AppContext;

use super::super::primitives::{NumberControl, SelectControl, SelectOption, SwitchControl, icon};

#[component]
pub(super) fn PlaybackGroup() -> Element {
    let mut context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let tab = context.active_tab_snapshot();
    let disabled = tab.is_none();
    let playing = *context.playing.read();
    let frame = tab.as_ref().map(|tab| tab.current_frame).unwrap_or(0);
    let count = tab.as_ref().map(|tab| tab.frame_count()).unwrap_or(0);
    let range = tab.as_ref().map(|tab| tab.frame_range);
    let maximum = count.saturating_sub(1);
    rsx! {
        button {
            r#type: "button", class: "pam-icon-button", disabled, title: tr(locale, "previous_frame"),
            onclick: move |_| advance_frame(context, -1, true), {icon(LdSkipBack)}
        }
        button {
            r#type: "button", class: "pam-icon-button primary", disabled, title: tr(locale, "play_pause"),
            onclick: move |_| context.playing.toggle(),
            if playing { {icon(LdPause)} } else { {icon(LdPlay)} }
        }
        button {
            r#type: "button", class: "pam-icon-button", disabled, title: tr(locale, "next_frame"),
            onclick: move |_| advance_frame(context, 1, true), {icon(LdSkipForward)}
        }
        span { class: "pam-frame-counter", "{frame + usize::from(count > 0)}/{count}" }
        input {
            class: "pam-frame-slider",
            r#type: "range",
            min: range.map(|range| range.begin).unwrap_or(0),
            max: range.map(|range| range.end).unwrap_or(0),
            value: frame,
            disabled,
            oninput: move |event| {
                if let Ok(frame) = event.value().parse::<usize>() { set_frame(context, frame); }
            },
        }
        span { class: "pam-field-label",
            NumberControl {
                value: range.map(|range| range.begin as u32).unwrap_or(0),
                min: 0,
                max: maximum as u32,
                disabled,
                onchange: move |value| set_frame_range(context, Some(value as usize), None),
            }
            span { class: "pam-range-separator", "-" }
            NumberControl {
                value: range.map(|range| range.end as u32).unwrap_or(0),
                min: 0,
                max: maximum as u32,
                disabled,
                onchange: move |value| set_frame_range(context, None, Some(value as usize)),
            }
        }
    }
}

#[component]
pub(super) fn SpeedGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let preferences = context.preferences.read().clone();
    let tab = context.active_tab_snapshot();
    let disabled = tab.is_none();
    let fps = tab.as_ref().map(|tab| tab.speed_fps).unwrap_or(30);
    let factor = tab
        .as_ref()
        .and_then(|tab| {
            let base = tab
                .active_sprite_info()
                .and_then(|sprite| sprite.frame_rate)
                .unwrap_or(tab.document.pam.frame_rate as f64);
            [0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0]
                .into_iter()
                .find(|factor| (base * factor).round() as u32 == fps)
        })
        .map(|value| value.to_string())
        .unwrap_or_else(|| "custom".into());
    let factors = [
        ("custom", tr(locale, "custom")),
        ("0.25", "0.25x"),
        ("0.5", "0.5x"),
        ("1", "1x"),
        ("1.5", "1.5x"),
        ("2", "2x"),
        ("3", "3x"),
        ("4", "4x"),
    ]
    .into_iter()
    .map(|(value, label)| SelectOption::new(value, label))
    .collect();
    rsx! {
        span { class: "pam-field-label",
            span { {tr(locale, "speed")} }
            NumberControl {
                value: fps, min: 1, max: 120, disabled,
                onchange: move |value| set_speed(context, value),
            }
            span { class: "pam-unit", "FPS" }
            SelectControl {
                value: factor,
                options: factors,
                compact: true,
                disabled,
                onchange: move |value: String| {
                    if let Ok(value) = value.parse::<f64>() { set_speed_factor(context, value); }
                },
            }
        }
        SwitchControl { label: tr(locale, "loop"), checked: preferences.loop_playback, onchange: move |value| set_loop(context, value) }
        SwitchControl { label: tr(locale, "reverse"), checked: preferences.reverse, onchange: move |value| set_reverse(context, value) }
        SwitchControl { label: tr(locale, "autoplay"), checked: preferences.autoplay, onchange: move |value| set_autoplay(context, value) }
        SwitchControl { label: tr(locale, "keep_speed"), checked: preferences.keep_speed, onchange: move |value| set_keep_speed(context, value) }
        SwitchControl { label: tr(locale, "boundary"), checked: preferences.boundary, onchange: move |value| set_boundary(context, value) }
    }
}
