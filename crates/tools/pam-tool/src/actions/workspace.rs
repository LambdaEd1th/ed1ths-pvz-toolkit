use dioxus::prelude::*;
use pam_viewer_core::SpriteKey;

use crate::state::{AppContext, Status, ViewerTab};

pub fn activate_tab(mut context: AppContext, id: u64) {
    if context.tabs.read().iter().any(|tab| tab.id == id) {
        context.active_tab.set(Some(id));
        context.sync_stage();
    }
}

pub fn close_tab(mut context: AppContext, id: u64) {
    let active = *context.active_tab.read();
    let mut tabs = context.tabs.write();
    let Some(index) = tabs.iter().position(|tab| tab.id == id) else {
        return;
    };
    let removed = tabs.remove(index);
    if active == Some(id) {
        let next = tabs
            .get(index.min(tabs.len().saturating_sub(1)))
            .map(|tab| tab.id);
        context.active_tab.set(next);
    }
    drop(tabs);
    for url in removed.image_thumbnails.iter().flatten() {
        crate::platform::release_thumbnail_url(url);
    }
    if context.active_tab.read().is_none() {
        context.playing.set(false);
    }
    context.sync_stage();
    super::release_document(removed.processing_id);
}

pub fn clear_tabs(mut context: AppContext) {
    let thumbnail_urls = context
        .tabs
        .read()
        .iter()
        .flat_map(|tab| tab.image_thumbnails.iter().flatten().cloned())
        .collect::<Vec<_>>();
    let document_ids = context
        .tabs
        .read()
        .iter()
        .map(|tab| tab.processing_id)
        .collect::<Vec<_>>();
    context.tabs.write().clear();
    context.active_tab.set(None);
    context.playing.set(false);
    context.sync_stage();
    context.status.set(Status::default());
    for url in thumbnail_urls {
        crate::platform::release_thumbnail_url(&url);
    }
    for document_id in document_ids {
        super::release_document(document_id);
    }
}

pub fn reorder_tab(mut context: AppContext, dragged: u64, target: u64) {
    if dragged == target {
        return;
    }
    let mut tabs = context.tabs.write();
    let Some(source) = tabs.iter().position(|tab| tab.id == dragged) else {
        return;
    };
    let tab = tabs.remove(source);
    let Some(target_index) = tabs.iter().position(|tab| tab.id == target) else {
        let restore_index = source.min(tabs.len());
        tabs.insert(restore_index, tab);
        return;
    };
    tabs.insert(target_index, tab);
}

pub fn reorder_toolbar_group(mut context: AppContext, dragged: &str, target: &str) {
    if dragged == target {
        return;
    }
    let mut preferences = context.preferences.write();
    let Some(source) = preferences
        .toolbar_order
        .iter()
        .position(|group| group == dragged)
    else {
        return;
    };
    let group = preferences.toolbar_order.remove(source);
    let Some(target_index) = preferences
        .toolbar_order
        .iter()
        .position(|candidate| candidate == target)
    else {
        let restore_index = source.min(preferences.toolbar_order.len());
        preferences.toolbar_order.insert(restore_index, group);
        return;
    };
    preferences.toolbar_order.insert(target_index, group);
}

pub fn finish_toolbar_reorder(mut context: AppContext) {
    if context.dragged_toolbar_group.read().is_some() {
        context.save_preferences();
    }
    context.dragged_toolbar_group.set(None);
}

pub fn activate_sprite(mut context: AppContext, key: SpriteKey) {
    let reverse = context.preferences.read().reverse;
    let keep_speed = context.preferences.read().keep_speed;
    context.playing.set(false);
    context.update_active_tab(|tab| {
        tab.active_sprite = key;
        let frame_count = tab.frame_count();
        tab.frame_range.begin = 0;
        tab.frame_range.end = frame_count.saturating_sub(1);
        tab.current_frame = if reverse {
            tab.frame_range.end
        } else {
            tab.frame_range.begin
        };
        tab.selected_label = None;
        if !keep_speed {
            tab.speed_fps = tab
                .active_sprite_info()
                .and_then(|sprite| sprite.frame_rate)
                .unwrap_or(tab.document.pam.frame_rate as f64)
                .round()
                .clamp(1.0, 120.0) as u32;
        }
    });
}

pub fn select_label(mut context: AppContext, index: Option<usize>) {
    let reverse = context.preferences.read().reverse;
    context.playing.set(false);
    context.update_active_tab(|tab| {
        if let Some(label_index) = index
            && let Some(label) = tab.labels().get(label_index)
        {
            tab.frame_range.begin = label.begin;
            tab.frame_range.end = label.end;
            tab.selected_label = Some(label_index);
        } else {
            tab.frame_range.begin = 0;
            tab.frame_range.end = tab.frame_count().saturating_sub(1);
            tab.selected_label = None;
        }
        tab.current_frame = if reverse {
            tab.frame_range.end
        } else {
            tab.frame_range.begin
        };
    });
}

pub fn set_export_dimension(context: AppContext, axis: usize, value: u32) {
    context.update_active_tab(|tab| {
        tab.export_size[axis.min(1)] = value.clamp(1, 99_999);
        tab.export_scale = export_scale_for(tab);
    });
}

pub fn set_export_scale(context: AppContext, scale: Option<u32>) {
    context.update_active_tab(|tab| {
        tab.export_scale = scale;
        if let Some(scale) = scale {
            tab.export_size = [
                (tab.document.pam.size[0] * scale as f64)
                    .round()
                    .clamp(1.0, 99_999.0) as u32,
                (tab.document.pam.size[1] * scale as f64)
                    .round()
                    .clamp(1.0, 99_999.0) as u32,
            ];
        }
    });
}

fn export_scale_for(tab: &ViewerTab) -> Option<u32> {
    let [width, height] = tab.document.pam.size;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let width_scale = tab.export_size[0] as f64 / width;
    let height_scale = tab.export_size[1] as f64 / height;
    if (width_scale - height_scale).abs() > 0.01 {
        return None;
    }
    let rounded = width_scale.round() as u32;
    (1..=4)
        .contains(&rounded)
        .then_some(rounded)
        .filter(|_| (width_scale - rounded as f64).abs() <= 0.01)
}

pub fn reset_view(context: AppContext) {
    context.update_active_tab(|tab| {
        let bounds = tab.document.stage_bounds();
        tab.zoom = 1.0;
        tab.pan = [
            -bounds.x - bounds.width / 2.0,
            -bounds.y - bounds.height / 2.0,
        ];
    });
}
