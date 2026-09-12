use dioxus::prelude::*;
use std::sync::Arc;

use pam_editor_core::{FrameInfo, PamInfo, SpriteInfo, SpriteKey};

use crate::state::{AppContext, EditorTab, Status, Tone};

const HISTORY_LIMIT: usize = 100;

pub fn new_document(mut context: AppContext) {
    let pam = PamInfo {
        version: 6,
        frame_rate: 30,
        position: [0.0; 2],
        size: [640.0, 480.0],
        image: vec![],
        sprite: vec![],
        main_sprite: Some(SpriteInfo {
            name: Some("main".into()),
            frame_rate: Some(30.0),
            work_area: Some([0, 1]),
            frame: vec![FrameInfo::default()],
        }),
    };
    let id = *context.next_tab_id.read();
    let result = pam_editor_core::PamDocument::new(format!("untitled-{id}.pam"), pam, vec![])
        .map_err(|e| e.to_string())
        .and_then(|document| {
            EditorTab::new(
                id,
                pam_editor_core::LoadedPamPayload {
                    document: pam_editor_core::PamDocumentPayload::from(&document),
                    loaded_images: 0,
                    missing_images: vec![],
                    original_pam_bytes: vec![],
                },
                &context.preferences.read(),
            )
        });
    match result {
        Ok(mut tab) => {
            tab.never_saved = true;
            context.next_tab_id.set(id.wrapping_add(1).max(1));
            context.tabs.write().push(tab);
            context.active_tab.set(Some(id));
            context.playing.set(false);
            context.sync_stage();
        }
        Err(error) => context.set_status(Status::new(error, Tone::Error)),
    }
}

/// Apply one atomic, undoable semantic edit to the current PAM document.
pub fn edit_document(context: AppContext, update: impl FnOnce(&mut PamInfo, SpriteKey, usize)) {
    apply_edit(context, false, update);
}

/// Apply one part of a pointer gesture. The pre-gesture document is committed
/// to history once by [`finish_edit_gesture`].
pub fn edit_document_gesture(
    context: AppContext,
    update: impl FnOnce(&mut PamInfo, SpriteKey, usize),
) {
    apply_edit(context, true, update);
}

fn apply_edit(
    mut context: AppContext,
    gesture: bool,
    update: impl FnOnce(&mut PamInfo, SpriteKey, usize),
) {
    context.playing.set(false);
    let Some(index) = context.active_tab_index() else {
        return;
    };
    let result = {
        let mut tabs = context.tabs.write();
        let tab = &mut tabs[index];
        let before = tab.document.pam.clone();
        let active_sprite = tab.active_sprite;
        let current_frame = tab.current_frame;
        let document = Arc::make_mut(&mut tab.document);
        update(&mut document.pam, active_sprite, current_frame);
        if document.pam == before {
            return;
        }
        if let Err(error) = validate_edit(&document.pam) {
            document.pam = before;
            drop(tabs);
            context.set_status(Status::new(error, Tone::Error));
            return;
        }
        if let Err(error) =
            pam_editor_core::encode_pam_bytes(&document.pam).and_then(|_| document.rebuild())
        {
            document.pam = before;
            let _ = document.rebuild();
            Err(error.to_string())
        } else {
            if gesture {
                if tab.pending_edit.is_none() {
                    tab.pending_edit = Some(before);
                    tab.redo_stack.clear();
                }
            } else {
                push_history(&mut tab.undo_stack, before);
                tab.redo_stack.clear();
            }
            tab.document_revision = tab.document_revision.wrapping_add(1).max(1);
            refresh_tab_after_edit(tab);
            Ok(())
        }
    };
    match result {
        Ok(()) => context.sync_stage(),
        Err(error) => context.set_status(Status::new(error, Tone::Error)),
    }
}

fn validate_edit(pam: &PamInfo) -> Result<(), String> {
    if !(1..=255).contains(&pam.frame_rate)
        || pam
            .size
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0 || *v > 3276.75)
        || pam
            .position
            .iter()
            .any(|v| !v.is_finite() || *v < -1638.4 || *v > 1638.35)
    {
        return Err("Canvas dimensions, position or frame rate exceed PAM limits".into());
    }
    for sprite in pam.sprite.iter().chain(pam.main_sprite.iter()) {
        for frame in &sprite.frame {
            for append in &frame.append {
                let count = if append.sprite {
                    pam.sprite.len()
                } else {
                    pam.image.len()
                };
                if append.resource as usize >= count {
                    return Err(format!(
                        "Instance #{} references a missing resource",
                        append.index
                    ));
                }
            }
            for change in &frame.change {
                if change.transform.iter().any(|v| !v.is_finite())
                    || change.color.is_some_and(|rgba| {
                        rgba.iter()
                            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                    })
                {
                    return Err(
                        "Transforms must be finite and RGBA channels must be between 0 and 1"
                            .into(),
                    );
                }
            }
        }
    }
    let mut incoming = vec![0usize; pam.sprite.len()];
    let edges: Vec<Vec<usize>> = pam
        .sprite
        .iter()
        .map(|sprite| {
            sprite
                .frame
                .iter()
                .flat_map(|f| &f.append)
                .filter(|a| a.sprite)
                .map(|a| a.resource as usize)
                .collect()
        })
        .collect();
    for targets in &edges {
        for &target in targets {
            incoming[target] += 1;
        }
    }
    let mut ready: Vec<usize> = incoming
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(index) = ready.pop() {
        visited += 1;
        for &target in &edges[index] {
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.push(target);
            }
        }
    }
    if visited != pam.sprite.len() {
        return Err("Sprite references cannot form a cycle".into());
    }
    Ok(())
}

pub fn finish_edit_gesture(mut context: AppContext) {
    let Some(index) = context.active_tab_index() else {
        return;
    };
    let mut tabs = context.tabs.write();
    let tab = &mut tabs[index];
    let Some(before) = tab.pending_edit.take() else {
        return;
    };
    if before != tab.document.pam {
        push_history(&mut tab.undo_stack, before);
    }
}

pub fn undo(mut context: AppContext) {
    context.playing.set(false);
    let Some(index) = context.active_tab_index() else {
        return;
    };
    let result = {
        let mut tabs = context.tabs.write();
        let tab = &mut tabs[index];
        tab.pending_edit = None;
        let Some(previous) = tab.undo_stack.pop() else {
            return;
        };
        let current = tab.document.pam.clone();
        let document = Arc::make_mut(&mut tab.document);
        document.pam = previous;
        match document.rebuild() {
            Ok(()) => {
                push_history(&mut tab.redo_stack, current);
                tab.document_revision = tab.document_revision.wrapping_add(1).max(1);
                refresh_tab_after_edit(tab);
                Ok(())
            }
            Err(error) => {
                document.pam = current;
                let _ = document.rebuild();
                Err(error.to_string())
            }
        }
    };
    match result {
        Ok(()) => context.sync_stage(),
        Err(error) => context.set_status(Status::new(error, Tone::Error)),
    }
}

pub fn redo(mut context: AppContext) {
    context.playing.set(false);
    let Some(index) = context.active_tab_index() else {
        return;
    };
    let result = {
        let mut tabs = context.tabs.write();
        let tab = &mut tabs[index];
        tab.pending_edit = None;
        let Some(next) = tab.redo_stack.pop() else {
            return;
        };
        let current = tab.document.pam.clone();
        let document = Arc::make_mut(&mut tab.document);
        document.pam = next;
        match document.rebuild() {
            Ok(()) => {
                push_history(&mut tab.undo_stack, current);
                tab.document_revision = tab.document_revision.wrapping_add(1).max(1);
                refresh_tab_after_edit(tab);
                Ok(())
            }
            Err(error) => {
                document.pam = current;
                let _ = document.rebuild();
                Err(error.to_string())
            }
        }
    };
    match result {
        Ok(()) => context.sync_stage(),
        Err(error) => context.set_status(Status::new(error, Tone::Error)),
    }
}

pub fn add_frame(context: AppContext, duplicate_current: bool) {
    edit_document(context, move |pam, key, frame_index| {
        let Some(sprite) = active_sprite_mut(pam, key) else {
            return;
        };
        let insert_at = frame_index.min(sprite.frame.len().saturating_sub(1)) + 1;
        let frame = duplicate_current
            .then(|| sprite.frame.get(frame_index).cloned())
            .flatten()
            .unwrap_or_default();
        sprite
            .frame
            .insert(insert_at.min(sprite.frame.len()), frame);
        if let Some(area) = &mut sprite.work_area {
            area[1] = sprite.frame.len().min(i16::MAX as usize) as i32;
        }
    });
    context.update_active_tab(|tab| {
        tab.current_frame = (tab.current_frame + 1).min(tab.frame_count().saturating_sub(1));
        tab.frame_range.end = tab.frame_count().saturating_sub(1);
    });
}

pub fn delete_current_frame(context: AppContext) {
    edit_document(context, |pam, key, frame_index| {
        let Some(sprite) = active_sprite_mut(pam, key) else {
            return;
        };
        if sprite.frame.len() > 1 && frame_index < sprite.frame.len() {
            sprite.frame.remove(frame_index);
            if let Some(area) = &mut sprite.work_area {
                area[0] = area[0].min(sprite.frame.len().saturating_sub(1) as i32);
                area[1] = sprite.frame.len().min(i16::MAX as usize) as i32;
            }
        }
    });
}

pub fn add_sprite(context: AppContext) {
    let version = context
        .active_tab_snapshot()
        .map(|tab| tab.document.pam.version)
        .unwrap_or(6);
    edit_document(context, move |pam, _, _| {
        let index = pam.sprite.len();
        pam.sprite.push(SpriteInfo {
            name: (version >= 4).then(|| format!("sprite_{index}")),
            frame_rate: (version >= 4).then_some(pam.frame_rate as f64),
            work_area: (version >= 5).then_some([0, 1]),
            frame: vec![FrameInfo::default()],
        });
    });
    context.update_active_tab(|tab| {
        if !tab.document.pam.sprite.is_empty() {
            tab.active_sprite = SpriteKey::Sprite(tab.document.pam.sprite.len() - 1);
            refresh_tab_after_edit(tab);
        }
    });
}

#[allow(dead_code)] // Reserved for the sprite-management UI.
pub fn delete_active_sprite(context: AppContext) {
    let Some(tab) = context.active_tab_snapshot() else {
        return;
    };
    let SpriteKey::Sprite(removed_index) = tab.active_sprite else {
        return;
    };
    edit_document(context, move |pam, _, _| {
        if removed_index >= pam.sprite.len() {
            return;
        }
        pam.sprite.remove(removed_index);
        visit_sprites_mut(pam, |sprite| {
            for frame in &mut sprite.frame {
                frame
                    .append
                    .retain(|append| !(append.sprite && append.resource as usize == removed_index));
                for append in &mut frame.append {
                    if append.sprite && append.resource as usize > removed_index {
                        append.resource -= 1;
                    }
                }
            }
        });
    });
    context.update_active_tab(|tab| {
        tab.active_sprite = if tab.document.pam.sprite.is_empty() {
            SpriteKey::Main
        } else {
            SpriteKey::Sprite(removed_index.min(tab.document.pam.sprite.len() - 1))
        };
        refresh_tab_after_edit(tab);
    });
}

#[allow(dead_code)] // Reserved for synchronous save integrations.
pub fn mark_saved(mut context: AppContext, tab_id: u64, bytes: Vec<u8>) {
    let mut tabs = context.tabs.write();
    let Some(tab) = tabs.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };
    tab.saved_pam = tab.document.pam.clone();
    tab.original_pam_bytes = Some(Arc::<[u8]>::from(bytes));
}

#[allow(dead_code)] // Reserved for explicit PAM version conversion.
pub fn migrate_version(pam: &mut PamInfo, version: i32) {
    let version = version.clamp(1, 6);
    if version == pam.version {
        return;
    }
    for image in &mut pam.image {
        if version == 1 {
            image.transform = matrix_to_rotation(&image.transform);
        } else {
            image.transform = transform_to_matrix(&image.transform);
        }
        if version >= 4 && image.size.is_none() {
            image.size = Some([0, 0]);
        }
    }
    let default_fps = pam.frame_rate as f64;
    visit_sprites_mut(pam, |sprite| {
        if version >= 4 {
            if sprite.name.is_none() {
                sprite.name = Some("sprite".to_string());
            }
            if sprite.frame_rate.is_none() {
                sprite.frame_rate = Some(default_fps);
            }
        }
        if version >= 5 && sprite.work_area.is_none() {
            sprite.work_area = Some([0, sprite.frame.len().min(i16::MAX as usize) as i32]);
        }
    });
    if version <= 3 && pam.main_sprite.is_none() {
        pam.main_sprite = Some(SpriteInfo {
            frame: vec![FrameInfo::default()],
            ..SpriteInfo::default()
        });
    }
    pam.version = version;
}

pub fn active_sprite_mut(pam: &mut PamInfo, key: SpriteKey) -> Option<&mut SpriteInfo> {
    match key {
        SpriteKey::Main => pam.main_sprite.as_mut(),
        SpriteKey::Sprite(index) => pam.sprite.get_mut(index),
    }
}

fn visit_sprites_mut(pam: &mut PamInfo, mut visit: impl FnMut(&mut SpriteInfo)) {
    for sprite in &mut pam.sprite {
        visit(sprite);
    }
    if let Some(main) = pam.main_sprite.as_mut() {
        visit(main);
    }
}

fn matrix_to_rotation(values: &[f64]) -> Vec<f64> {
    match values {
        [angle, x, y] => vec![*angle, *x, *y],
        [x, y] => vec![0.0, *x, *y],
        [a, b, _, _, x, y] => vec![b.atan2(*a), *x, *y],
        _ => vec![0.0, 0.0, 0.0],
    }
}

fn transform_to_matrix(values: &[f64]) -> Vec<f64> {
    match values {
        [a, b, c, d, x, y] => vec![*a, *b, *c, *d, *x, *y],
        [angle, x, y] => {
            let (sin, cos) = angle.sin_cos();
            vec![cos, sin, -sin, cos, *x, *y]
        }
        [x, y] => vec![1.0, 0.0, 0.0, 1.0, *x, *y],
        _ => vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    }
}

fn push_history(stack: &mut Vec<PamInfo>, value: PamInfo) {
    if stack.len() == HISTORY_LIMIT {
        stack.remove(0);
    }
    stack.push(value);
}

pub(crate) fn refresh_tab_after_edit(tab: &mut EditorTab) {
    if matches!(tab.active_sprite, SpriteKey::Main) && tab.document.pam.main_sprite.is_none() {
        tab.active_sprite = SpriteKey::Sprite(0);
    }
    if let SpriteKey::Sprite(index) = tab.active_sprite
        && index >= tab.document.pam.sprite.len()
    {
        tab.active_sprite = if tab.document.pam.main_sprite.is_some() {
            SpriteKey::Main
        } else {
            SpriteKey::Sprite(tab.document.pam.sprite.len().saturating_sub(1))
        };
    }
    let frame_count = tab.frame_count();
    tab.current_frame = tab.current_frame.min(frame_count.saturating_sub(1));
    tab.frame_range.begin = tab.frame_range.begin.min(frame_count.saturating_sub(1));
    tab.frame_range.end = tab.frame_range.end.min(frame_count.saturating_sub(1));
    if tab.frame_range.begin > tab.frame_range.end {
        tab.frame_range.begin = tab.frame_range.end;
    }
    tab.image_filter.resize(tab.document.pam.image.len(), true);
    tab.sprite_filter
        .resize(tab.document.pam.sprite.len(), true);
    tab.selected_image = tab
        .selected_image
        .filter(|index| *index < tab.document.pam.image.len())
        .or_else(|| (!tab.document.pam.image.is_empty()).then_some(0));
    tab.special_layers =
        pam_editor_core::special_layer_indices(&tab.document.pam, &tab.document.source_name);
    tab.selected_label = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_migration_produces_required_v6_fields() {
        let mut pam = PamInfo {
            version: 1,
            frame_rate: 30,
            position: [0.0; 2],
            size: [100.0; 2],
            image: vec![pam_editor_core::ImageInfo {
                name: "image".into(),
                size: None,
                transform: vec![0.0, 1.0, 2.0],
            }],
            sprite: vec![SpriteInfo {
                frame: vec![FrameInfo::default()],
                ..SpriteInfo::default()
            }],
            main_sprite: Some(SpriteInfo {
                frame: vec![FrameInfo::default()],
                ..SpriteInfo::default()
            }),
        };
        migrate_version(&mut pam, 6);
        assert_eq!(pam.image[0].transform.len(), 6);
        assert!(pam.image[0].size.is_some());
        assert!(pam.sprite[0].name.is_some());
        assert!(pam.sprite[0].frame_rate.is_some());
        assert!(pam.sprite[0].work_area.is_some());
        pam_editor_core::encode_pam_bytes(&pam).unwrap();
    }
}
