use dioxus::prelude::*;
use dioxus_html::InteractionLocation;

use crate::app_actions::{finish_tab_drag_state, finish_toolbar_drag_state};
use crate::app_constants::{PANEL_MAX_WIDTH, PANEL_MIN_WIDTH};
use crate::components::{PanelResizeDrag, PanelSide, clamp_panel_width};
use crate::domain::{
    DropMarker, EditorTabState, ToolbarDropTarget, ToolbarGroupId, apply_toolbar_drop_target,
};

pub(crate) fn commit_panel_resize_width(
    event: MouseEvent,
    panel_resize_drag: Signal<Option<PanelResizeDrag>>,
    mut left_panel_width: Signal<i32>,
    mut right_panel_width: Signal<i32>,
) {
    let Some(drag) = *panel_resize_drag.read() else {
        return;
    };

    event.prevent_default();
    let next_width = panel_resize_width_for_x(drag, event.client_coordinates().x);
    match drag.side {
        PanelSide::Left => left_panel_width.set(next_width),
        PanelSide::Right => right_panel_width.set(next_width),
    }
}

fn panel_resize_width_for_x(drag: PanelResizeDrag, x: f64) -> i32 {
    let delta = match drag.side {
        PanelSide::Left => x - drag.start_x,
        PanelSide::Right => drag.start_x - x,
    };
    clamp_panel_width(drag.start_width as f64 + delta)
}

fn panel_resize_css_var(side: PanelSide) -> &'static str {
    match side {
        PanelSide::Left => "--rton-left-panel-width",
        PanelSide::Right => "--rton-right-panel-width",
    }
}

pub(crate) fn start_workspace_panel_resize_preview(drag: PanelResizeDrag) {
    let side = match drag.side {
        PanelSide::Left => "left",
        PanelSide::Right => "right",
    };
    let css_var = panel_resize_css_var(drag.side);
    let start_x = drag.start_x;
    let start_width = drag.start_width;
    dioxus::document::eval(&format!(
        r#"
        (() => {{
            const side = "{side}";
            const cssVar = "{css_var}";
            const startX = {start_x};
            const startWidth = {start_width};
            const minWidth = {PANEL_MIN_WIDTH};
            const maxWidth = {PANEL_MAX_WIDTH};
            const stateKey = "__rtonPanelResizeState";
            const target = document.querySelector(".rton-workspace-shell");
            if (!target) return;

            const stopCurrentDrag = () => {{
                const state = window[stateKey];
                if (!state) {{
                    target.classList.remove("resizing-panel");
                    return;
                }}
                document.removeEventListener("mousemove", state.onMove, true);
                document.removeEventListener("mouseup", state.onUp, true);
                if (state.frame) {{
                    cancelAnimationFrame(state.frame);
                }}
                target.classList.remove("resizing-panel");
                window[stateKey] = null;
            }};
            stopCurrentDrag();
            target.classList.add("resizing-panel");

            const applyWidth = (clientX) => {{
                const rawDelta = side === "left" ? clientX - startX : startX - clientX;
                const width = Math.round(Math.min(maxWidth, Math.max(minWidth, startWidth + rawDelta)));
                const state = window[stateKey];
                if (state?.frame) {{
                    cancelAnimationFrame(state.frame);
                }}
                const frame = requestAnimationFrame(() => {{
                    target.style.setProperty(cssVar, `${{width}}px`);
                    const latest = window[stateKey];
                    if (latest) {{
                        latest.frame = 0;
                    }}
                }});
                const latest = window[stateKey];
                if (latest) {{
                    latest.frame = frame;
                }}
            }};

            const onMove = (event) => {{
                event.preventDefault();
                applyWidth(event.clientX);
            }};
            const onUp = () => stopCurrentDrag();
            window[stateKey] = {{ onMove, onUp, frame: 0 }};
            applyWidth(startX);
            document.addEventListener("mousemove", onMove, true);
            document.addEventListener("mouseup", onUp, true);
        }})();
        "#
    ));
}

fn stop_workspace_panel_resize_preview() {
    dioxus::document::eval(
        r#"
        (() => {
            const stateKey = "__rtonPanelResizeState";
            const state = window[stateKey];
            const target = document.querySelector(".rton-workspace-shell");
            if (!state) {
                target?.classList.remove("resizing-panel");
                return;
            }
            document.removeEventListener("mousemove", state.onMove, true);
            document.removeEventListener("mouseup", state.onUp, true);
            if (state.frame) {
                cancelAnimationFrame(state.frame);
            }
            target?.classList.remove("resizing-panel");
            window[stateKey] = null;
        })();
        "#,
    );
}

pub(crate) fn finish_workspace_drag_state(
    mut panel_resize_drag: Signal<Option<PanelResizeDrag>>,
    tabs: Signal<Vec<EditorTabState>>,
    dragged_tab_id: Signal<Option<usize>>,
    tab_drop_marker: Signal<Option<DropMarker<usize>>>,
    toolbar_rows: Signal<Vec<Vec<ToolbarGroupId>>>,
    dragged_toolbar_group_id: Signal<Option<ToolbarGroupId>>,
    toolbar_drop_marker: Signal<Option<DropMarker<ToolbarGroupId>>>,
) {
    if panel_resize_drag.read().is_some() {
        stop_workspace_panel_resize_preview();
    }
    panel_resize_drag.set(None);
    finish_tab_drag_state(tabs, dragged_tab_id, tab_drop_marker);
    finish_toolbar_drag_state(toolbar_rows, dragged_toolbar_group_id, toolbar_drop_marker);
}

pub(crate) fn start_tab_drag_if_needed(
    id: usize,
    tabs: Signal<Vec<EditorTabState>>,
    mut dragged_tab_id: Signal<Option<usize>>,
    mut tab_drop_marker: Signal<Option<DropMarker<usize>>>,
) {
    if tabs.read().len() > 1 {
        dragged_tab_id.set(Some(id));
        tab_drop_marker.set(None);
    }
}

pub(crate) fn update_tab_drop_marker_for_drag(
    marker: DropMarker<usize>,
    dragged_tab_id: Signal<Option<usize>>,
    mut tab_drop_marker: Signal<Option<DropMarker<usize>>>,
) {
    if matches!(*dragged_tab_id.read(), Some(id) if id != marker.id) {
        tab_drop_marker.set(Some(marker));
    }
}

pub(crate) fn start_toolbar_group_drag_state(
    id: ToolbarGroupId,
    mut dragged_toolbar_group_id: Signal<Option<ToolbarGroupId>>,
    mut toolbar_drop_marker: Signal<Option<DropMarker<ToolbarGroupId>>>,
) {
    dragged_toolbar_group_id.set(Some(id));
    toolbar_drop_marker.set(None);
}

pub(crate) fn update_toolbar_drop_marker_for_drag(
    target: ToolbarDropTarget,
    mut toolbar_rows: Signal<Vec<Vec<ToolbarGroupId>>>,
    dragged_toolbar_group_id: Signal<Option<ToolbarGroupId>>,
    mut toolbar_drop_marker: Signal<Option<DropMarker<ToolbarGroupId>>>,
) {
    let Some(dragged_id) = *dragged_toolbar_group_id.read() else {
        return;
    };
    let marker = {
        let mut rows = toolbar_rows.write();
        apply_toolbar_drop_target(&mut rows, dragged_id, target)
    };
    toolbar_drop_marker.set(marker);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_panel_resize_tracks_pointer_delta() {
        let drag = PanelResizeDrag {
            side: PanelSide::Left,
            start_x: 100.0,
            start_width: 260,
        };

        assert_eq!(panel_resize_width_for_x(drag, 140.0), 300);
    }

    #[test]
    fn right_panel_resize_inverts_pointer_delta() {
        let drag = PanelResizeDrag {
            side: PanelSide::Right,
            start_x: 400.0,
            start_width: 260,
        };

        assert_eq!(panel_resize_width_for_x(drag, 360.0), 300);
    }

    #[test]
    fn panel_resize_width_is_clamped() {
        let too_small = PanelResizeDrag {
            side: PanelSide::Left,
            start_x: 100.0,
            start_width: 260,
        };
        let too_large = PanelResizeDrag {
            side: PanelSide::Right,
            start_x: 100.0,
            start_width: 260,
        };

        assert_eq!(
            panel_resize_width_for_x(too_small, -1000.0),
            PANEL_MIN_WIDTH
        );
        assert_eq!(
            panel_resize_width_for_x(too_large, -1000.0),
            PANEL_MAX_WIDTH
        );
    }
}
