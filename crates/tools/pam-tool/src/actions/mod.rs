mod editor;
mod export;
mod loading;
mod playback;
mod preferences;
mod visibility;
mod workspace;

pub use editor::{
    active_sprite_mut, add_frame, add_sprite, delete_current_frame, edit_document,
    edit_document_gesture, finish_edit_gesture, new_document, redo, undo,
};
pub use export::{ExportKind, start_export};
pub use loading::input_files_from_dioxus;
#[cfg(not(target_arch = "wasm32"))]
pub use loading::load_folder;
pub use loading::load_inputs;
pub(crate) use loading::release_document;
pub use playback::{
    advance_frame, set_frame, set_frame_range, set_speed, set_speed_factor, use_playback_clock,
};
pub use preferences::{
    set_autoplay, set_boundary, set_keep_speed, set_loop, set_resource_sheet_open, set_reverse,
};
pub use visibility::{
    restore_default_sprite_visibility, select_exclusive_special_layer, set_all_images_visible,
    set_all_sprites_visible, set_ground_swatch_visible, set_image_visible, set_sprite_visible,
};
pub use workspace::{
    activate_sprite, activate_tab, clear_tabs, clear_tabs_confirmed, close_tab,
    close_tab_confirmed, reorder_tab, reset_view, select_label, set_export_dimension,
    set_export_scale,
};
