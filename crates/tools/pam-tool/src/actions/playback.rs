use dioxus::prelude::*;

use crate::state::AppContext;

pub fn use_playback_clock() {
    let context = use_context::<AppContext>();
    use_resource(move || {
        let playing = *context.playing.read();
        async move {
            if !playing {
                std::future::pending::<()>().await;
                return;
            }

            loop {
                let active_tab = *context.active_tab.peek();
                let fps = context
                    .tabs
                    .peek()
                    .iter()
                    .find(|tab| Some(tab.id) == active_tab)
                    .map(|tab| tab.speed_fps)
                    .unwrap_or(30)
                    .clamp(1, 120);
                crate::platform::sleep_ms((1000 / fps).max(1) as u64).await;
                if !*context.playing.peek() {
                    break;
                }
                advance_frame(context, 1, false);
            }
        }
    });
}

pub fn advance_frame(mut context: AppContext, amount: isize, manual: bool) {
    if manual {
        context.playing.set(false);
    }
    let reverse = context.preferences.read().reverse;
    let looping = context.preferences.read().loop_playback;
    let mut should_stop = false;
    context.update_active_tab(|tab| {
        let direction = if reverse { -amount } else { amount };
        let next = tab.current_frame as isize + direction;
        if next > tab.frame_range.end as isize {
            if looping || manual {
                tab.current_frame = tab.frame_range.begin;
            } else {
                tab.current_frame = tab.frame_range.end;
                should_stop = true;
            }
        } else if next < tab.frame_range.begin as isize {
            if looping || manual {
                tab.current_frame = tab.frame_range.end;
            } else {
                tab.current_frame = tab.frame_range.begin;
                should_stop = true;
            }
        } else {
            tab.current_frame = next as usize;
        }
    });
    if should_stop {
        context.playing.set(false);
    }
}

pub fn set_frame(mut context: AppContext, frame: usize) {
    context.playing.set(false);
    context.update_active_tab(|tab| {
        tab.current_frame = frame.clamp(tab.frame_range.begin, tab.frame_range.end);
    });
}

pub fn set_frame_range(mut context: AppContext, begin: Option<usize>, end: Option<usize>) {
    context.playing.set(false);
    context.update_active_tab(|tab| {
        let maximum = tab.frame_count().saturating_sub(1);
        let mut next_begin = begin.unwrap_or(tab.frame_range.begin).min(maximum);
        let mut next_end = end.unwrap_or(tab.frame_range.end).min(maximum);
        if next_begin > next_end {
            if begin.is_some() {
                next_end = next_begin;
            } else {
                next_begin = next_end;
            }
        }
        tab.frame_range.begin = next_begin;
        tab.frame_range.end = next_end;
        tab.current_frame = tab.current_frame.clamp(next_begin, next_end);
        tab.selected_label = None;
    });
}

pub fn set_speed(mut context: AppContext, fps: u32) {
    let fps = fps.clamp(1, 120);
    context.update_active_tab(|tab| tab.speed_fps = fps);
    if context.preferences.read().keep_speed {
        context.preferences.write().speed_fps = Some(fps);
        context.save_preferences();
    }
}

pub fn set_speed_factor(context: AppContext, factor: f64) {
    let Some(tab) = context.active_tab_snapshot() else {
        return;
    };
    let base = tab
        .active_sprite_info()
        .and_then(|sprite| sprite.frame_rate)
        .unwrap_or(tab.document.pam.frame_rate as f64);
    set_speed(context, (base * factor).round().clamp(1.0, 120.0) as u32);
}
