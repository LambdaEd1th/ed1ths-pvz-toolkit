use crate::state::AppContext;

pub fn set_image_visible(context: AppContext, index: usize, visible: bool) {
    context.update_active_tab(|tab| {
        if let Some(value) = tab.image_filter.get_mut(index) {
            *value = visible;
        }
    });
}

pub fn set_sprite_visible(context: AppContext, index: usize, visible: bool) {
    context.update_active_tab(|tab| {
        if let Some(value) = tab.sprite_filter.get_mut(index) {
            *value = visible;
        }
    });
}

pub fn set_all_images_visible(context: AppContext, visible: bool) {
    context.update_active_tab(|tab| tab.image_filter.fill(visible));
}

pub fn set_all_sprites_visible(context: AppContext, visible: bool) {
    context.update_active_tab(|tab| tab.sprite_filter.fill(visible));
}

pub fn restore_default_sprite_visibility(context: AppContext) {
    context.update_active_tab(|tab| tab.restore_default_sprite_visibility());
}

pub fn select_exclusive_special_layer(
    context: AppContext,
    indices: Vec<usize>,
    selected: Option<usize>,
) {
    context.update_active_tab(|tab| {
        for index in indices {
            if let Some(visible) = tab.sprite_filter.get_mut(index) {
                *visible = Some(index) == selected;
            }
        }
    });
}

pub fn set_ground_swatch_visible(context: AppContext, visible: bool) {
    let indices = context
        .active_tab_snapshot()
        .map(|tab| tab.special_layers.ground_swatch_layers)
        .unwrap_or_default();
    context.update_active_tab(|tab| {
        for index in indices {
            if let Some(value) = tab.sprite_filter.get_mut(index) {
                *value = visible;
            }
        }
    });
}
