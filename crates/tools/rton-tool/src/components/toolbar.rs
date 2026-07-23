use dioxus::prelude::*;

use crate::app_constants::TOOLBAR_GROUP_DROP_MIDPOINT_PX;
use crate::domain::{DropPlacement, ToolbarDropTarget, ToolbarGroupId};
use crate::i18n::I18n;

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
    // Toolbar customization is intentionally not exposed in the MoeSekai
    // workbench. Keep these inputs temporarily so persisted layouts can still
    // be read while the domain model is migrated independently from the UI.
    let _ = (label, i18n, dragging, drop_placement, on_drag_start);

    rsx! {
        div {
            class: "rton-toolbar-group-shell",
            "data-toolbar-group": "{id.code()}",
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
            {children}
        }
    }
}
