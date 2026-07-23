use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::LdGripVertical;

use crate::app_constants::TOOLBAR_GROUP_DROP_MIDPOINT_PX;
use crate::components::ui_helpers::lucide_icon;
use crate::domain::{DropPlacement, ToolbarDropTarget, ToolbarGroupId};
use crate::i18n::I18n;

fn toolbar_group_class(dragging: bool, drop_placement: Option<DropPlacement>) -> String {
    let mut class_name = "rton-toolbar-group-shell".to_string();
    if dragging {
        class_name.push_str(" dragging");
    }
    if let Some(drop_placement) = drop_placement {
        class_name.push(' ');
        class_name.push_str(drop_placement.class());
    }
    class_name
}

#[component]
pub(crate) fn ToolbarGroup(
    id: ToolbarGroupId,
    label: String,
    i18n: I18n,
    dragging: bool,
    drop_placement: Option<DropPlacement>,
    on_drag_start: EventHandler<ToolbarGroupId>,
    on_drop_target: EventHandler<ToolbarDropTarget>,
    on_drag_end: EventHandler<()>,
    children: Element,
) -> Element {
    let move_title = i18n.t_args("title-move-group", &[("label", label.clone())]);
    let class_name = toolbar_group_class(dragging, drop_placement);

    rsx! {
        div {
            class: "{class_name}",
            onmousemove: move |event| {
                event.stop_propagation();
                let placement = if event.element_coordinates().x < TOOLBAR_GROUP_DROP_MIDPOINT_PX {
                    DropPlacement::Before
                } else {
                    DropPlacement::After
                };
                on_drop_target.call(ToolbarDropTarget::Group { id, placement });
            },
            onmouseup: move |_| on_drag_end.call(()),
            button {
                r#type: "button",
                class: "rton-toolbar-group-drag-handle",
                title: "{move_title}",
                aria_label: "{move_title}",
                onmousedown: move |event| {
                    event.prevent_default();
                    event.stop_propagation();
                    on_drag_start.call(id);
                },
                {lucide_icon(LdGripVertical)}
            }
            {children}
        }
    }
}
