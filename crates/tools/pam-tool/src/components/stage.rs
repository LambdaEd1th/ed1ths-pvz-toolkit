#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
use std::sync::Arc;

use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use dioxus::web::WebEventExt;
use dioxus_free_icons::icons::ld_icons::LdFolderOpen;
use dioxus_html::HasFileData;
use dioxus_html::input_data::MouseButton;
use pam_viewer_core::Rect;
#[cfg(target_arch = "wasm32")]
use pam_viewer_core::{RenderDocumentPayload, RenderScenePayload, RenderViewPayload};
use serde::Deserialize;

use crate::actions::{input_files_from_dioxus, load_inputs};
use crate::i18n::tr;
use crate::state::{AppContext, BoundaryEdge, StageDrag, ViewerTab};

use super::page_actions::LoadButton;
use super::primitives::icon;

const CANVAS_ID: &str = "pam-stage-canvas";
const STAGE_POINTER_CAPTURE: &str = r#"
(() => {
    window.pamStagePointerCapture?.destroy?.();
    const stage = document.getElementById("pam-stage");
    if (!stage) return;

    const capture = (event) => {
        if (event.isPrimary && event.button === 0) {
            try { stage.setPointerCapture(event.pointerId); } catch (_) {}
        }
    };
    const release = (event) => {
        if (stage.hasPointerCapture(event.pointerId)) {
            try { stage.releasePointerCapture(event.pointerId); } catch (_) {}
        }
    };
    stage.addEventListener("pointerdown", capture);
    stage.addEventListener("pointerup", release);
    stage.addEventListener("pointercancel", release);

    const host = {
        destroy() {
            stage.removeEventListener("pointerdown", capture);
            stage.removeEventListener("pointerup", release);
            stage.removeEventListener("pointercancel", release);
            if (window.pamStagePointerCapture === host) {
                window.pamStagePointerCapture = null;
            }
        },
    };
    window.pamStagePointerCapture = host;
})();
"#;
#[cfg(target_arch = "wasm32")]
const WEB_STAGE_HOST: &str = include_str!("../../assets/pam/pam_stage.js");
#[cfg(not(target_arch = "wasm32"))]
const NATIVE_STAGE_HOST: &str = include_str!("../../assets/pam/native_stage_host.js");

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NativeStageMessage {
    Bounds {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        window_width: f32,
        window_height: f32,
        corner_radii: [f32; 4],
    },
    Error {
        message: String,
    },
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WebStageMessage {
    Ready { backend: String },
    Error { message: String },
}

#[component]
pub fn Stage() -> Element {
    let mut context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let tab = context.active_tab_snapshot();
    let mut dragging_files = use_signal(|| false);
    let mut hovered_edge = use_signal(|| None::<BoundaryEdge>);
    let cursor_class = match *context.stage_drag.read() {
        Some(StageDrag::Pan { .. }) => "pam-stage--panning",
        Some(StageDrag::Boundary { edge, .. }) => boundary_cursor_class(edge),
        None => match *hovered_edge.read() {
            Some(edge) => boundary_cursor_class(edge),
            None => "",
        },
    };
    let drop_class = if *dragging_files.read() {
        " drop-active"
    } else {
        ""
    };
    let stage_class = format!("pam-stage {cursor_class}{drop_class}");

    rsx! {
        section {
            id: "pam-stage",
            class: stage_class,
            onmounted: move |_| {
                let _ = document::eval(STAGE_POINTER_CAPTURE);
            },
            onresize: move |event| {
                if let Ok(size) = event.get_content_box_size() {
                    context.stage_size.set([size.width.max(1.0), size.height.max(1.0)]);
                }
            },
            onwheel: move |event| {
                event.prevent_default();
                let Some(tab) = context.active_tab_snapshot() else { return; };
                let coordinates = event.element_coordinates();
                let delta = event.delta().strip_units().y;
                zoom_at(context, &tab, [coordinates.x, coordinates.y], delta);
            },
            onpointerdown: move |event| {
                if tab.is_none() || !is_primary_pointer(&event) {
                    return;
                }
                event.prevent_default();
                let point = [event.element_coordinates().x, event.element_coordinates().y];
                hovered_edge.set(begin_stage_drag(context, point));
            },
            onpointermove: move |event| {
                let point = [event.element_coordinates().x, event.element_coordinates().y];
                update_pointer_coordinate(context, point);
                if context.stage_drag.read().is_none() {
                    hovered_edge.set(boundary_at(context, point));
                }
                update_stage_drag(context, point);
            },
            onpointerleave: move |_| {
                if context.stage_drag.read().is_none() {
                    context.pointer_coord.set(None);
                    hovered_edge.set(None);
                }
            },
            onpointerup: move |event| {
                context.stage_drag.set(None);
                let point = [event.element_coordinates().x, event.element_coordinates().y];
                hovered_edge.set(boundary_at(context, point));
            },
            onpointercancel: move |_| {
                context.stage_drag.set(None);
                context.pointer_coord.set(None);
                hovered_edge.set(None);
            },
            ondragenter: move |event| {
                event.prevent_default();
                if !event.files().is_empty() { dragging_files.set(true); }
            },
            ondragover: move |event| {
                event.prevent_default();
                if !event.files().is_empty() { dragging_files.set(true); }
            },
            ondragleave: move |_| dragging_files.set(false),
            ondrop: move |event| async move {
                event.prevent_default();
                dragging_files.set(false);
                let fallback = event.files();
                #[cfg(target_arch = "wasm32")]
                let files = match crate::platform::input_files_from_web_drop(event.as_web_event()).await {
                    Ok(files) if !files.is_empty() => Ok(files),
                    Ok(_) => input_files_from_dioxus(fallback).await,
                    Err(error) => Err(error),
                };
                #[cfg(not(target_arch = "wasm32"))]
                let files = input_files_from_dioxus(fallback).await;
                match files {
                    Ok(files) if !files.is_empty() => load_inputs(context, files),
                    Ok(_) => {}
                    Err(error) => context.set_status(crate::state::Status::new(
                        error,
                        crate::state::Tone::Error,
                    )),
                }
            },
            StageCanvas {}
            if tab.is_none() {
                div { class: "pam-drop-hint",
                    div { class: "pam-empty-stage-icon", {icon(LdFolderOpen)} }
                    h2 { {tr(locale, "drop_title")} }
                    p { {tr(locale, "drop_subtitle")} }
                    div { class: "pam-format-row", aria_hidden: "true",
                        span { "PAM" }
                        span { "JSON" }
                        span { "YAML" }
                        span { "TOML" }
                        span { "FLA / XFL" }
                    }
                    LoadButton { large: true }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn StageCanvas() -> Element {
    let context = use_context::<AppContext>();
    let mut renderer_generation = use_signal(|| 0_u64);
    let sent_scene = use_hook(|| Rc::new(RefCell::new(None::<(u64, Option<u64>)>)));

    let start_host = move |_| {
        let asset_root = serde_json::to_string(&super::APP_ASSETS.to_string())
            .unwrap_or_else(|_| "\"/assets\"".to_string());
        let runtime = WEB_STAGE_HOST.replace("__PAM_ASSET_ROOT__", &asset_root);
        let mut evaluator = document::eval(&runtime);
        spawn(async move {
            while let Ok(message) = evaluator.recv::<WebStageMessage>().await {
                match message {
                    WebStageMessage::Ready { backend } => {
                        crate::platform::log_buffer::push(
                            "INFO",
                            &format!("Web renderer ready: {backend}"),
                        );
                        let next = renderer_generation.read().wrapping_add(1).max(1);
                        renderer_generation.set(next);
                    }
                    WebStageMessage::Error { message } => {
                        context.set_status(crate::state::Status::new(
                            message,
                            crate::state::Tone::Error,
                        ));
                    }
                }
            }
        });
    };

    use_effect(move || {
        let generation = *renderer_generation.read();
        let preferences = context.preferences.read();
        let boundary = preferences.boundary;
        let dark_background = match preferences.theme {
            crate::state::Theme::Dark => true,
            crate::state::Theme::Light => false,
            crate::state::Theme::System => crate::platform::system_is_dark(),
        };
        drop(preferences);
        let tab = context.active_tab_snapshot();
        let identity = (generation, tab.as_ref().map(|tab| tab.id));
        let full_scene = sent_scene.borrow().as_ref() != Some(&identity);
        let view = tab
            .as_ref()
            .map(|tab| tab.render_view(boundary, dark_background))
            .unwrap_or(RenderViewPayload {
                boundary,
                dark_background,
                ..RenderViewPayload::default()
            });
        let result = if full_scene {
            let scene = RenderScenePayload {
                document: tab
                    .as_ref()
                    .map(|tab| RenderDocumentPayload::from(tab.document.as_ref())),
                view,
            };
            crate::platform::web_renderer::send_scene(&scene)
        } else {
            crate::platform::web_renderer::send_view(&view)
        };
        if result.is_ok() && full_scene {
            *sent_scene.borrow_mut() = Some(identity);
        }
    });
    rsx! {
        canvas {
            id: CANVAS_ID,
            class: "pam-stage-canvas",
            onmounted: start_host,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn StageCanvas() -> Element {
    let mut context = use_context::<AppContext>();
    let renderer = use_context::<crate::platform::native_renderer::NativeRendererContext>();
    let error_renderer = renderer.clone();
    use_effect(move || {
        if let Some(error) = error_renderer.error() {
            context
                .status
                .set(crate::state::Status::new(error, crate::state::Tone::Error));
        }
    });
    let redraw_renderer = renderer.clone();
    use_effect(move || {
        let _ = context.tabs.read();
        let _ = context.active_tab.read();
        let _ = context.preferences.read().boundary;
        redraw_renderer.request_redraw();
    });
    let start_host = {
        let renderer = renderer.clone();
        move |_| {
            let renderer = renderer.clone();
            let mut evaluator = document::eval(NATIVE_STAGE_HOST);
            spawn(async move {
                while let Ok(message) = evaluator.recv::<NativeStageMessage>().await {
                    match message {
                        NativeStageMessage::Bounds {
                            x,
                            y,
                            width,
                            height,
                            window_width,
                            window_height,
                            corner_radii,
                        } => {
                            context
                                .stage_size
                                .set([f64::from(width.max(1.0)), f64::from(height.max(1.0))]);
                            renderer.set_viewport(pam_viewer_renderer::NativeViewport {
                                x,
                                y,
                                width,
                                height,
                                window_width,
                                window_height,
                                corner_radii,
                            });
                        }
                        NativeStageMessage::Error { message } => {
                            context.set_status(crate::state::Status::new(
                                message,
                                crate::state::Tone::Error,
                            ));
                        }
                    }
                }
            });
        }
    };
    rsx! {
        canvas {
            id: CANVAS_ID,
            class: "pam-stage-canvas",
            onmounted: start_host,
        }
    }
}

fn camera_fit(bounds: Rect, viewport: [f64; 2]) -> f64 {
    let available_width = (viewport[0] - 72.0).max(1.0);
    let available_height = (viewport[1] - 72.0).max(1.0);
    (available_width / bounds.width.max(1.0) as f64)
        .min(available_height / bounds.height.max(1.0) as f64)
        .max(0.0001)
}

fn camera_scale(tab: &ViewerTab, viewport: [f64; 2]) -> f64 {
    camera_fit(tab.document.stage_bounds(), viewport) * tab.zoom as f64
}

fn screen_to_world(tab: &ViewerTab, viewport: [f64; 2], point: [f64; 2]) -> [f64; 2] {
    let scale = camera_scale(tab, viewport);
    [
        (point[0] - viewport[0] / 2.0) / scale - tab.pan[0] as f64,
        (point[1] - viewport[1] / 2.0) / scale - tab.pan[1] as f64,
    ]
}

fn update_pointer_coordinate(mut context: AppContext, point: [f64; 2]) {
    let Some(tab) = context.active_tab_snapshot() else {
        context.pointer_coord.set(None);
        return;
    };
    let world = screen_to_world(&tab, *context.stage_size.read(), point);
    context.pointer_coord.set(Some([
        (world[0] + tab.document.pam.position[0]) as f32,
        (world[1] + tab.document.pam.position[1]) as f32,
    ]));
}

fn zoom_at(context: AppContext, tab: &ViewerTab, point: [f64; 2], delta: f64) {
    let viewport = *context.stage_size.read();
    let world = screen_to_world(tab, viewport, point);
    let factor = (-delta * 0.0015).exp().clamp(0.8, 1.25) as f32;
    context.update_active_tab(|tab| {
        tab.zoom = (tab.zoom * factor).clamp(0.05, 64.0);
        let scale = camera_scale(tab, viewport);
        tab.pan = [
            ((point[0] - viewport[0] / 2.0) / scale - world[0]) as f32,
            ((point[1] - viewport[1] / 2.0) / scale - world[1]) as f32,
        ];
    });
}

fn boundary_at(context: AppContext, point: [f64; 2]) -> Option<BoundaryEdge> {
    let tab = context.active_tab_snapshot()?;
    if !context.preferences.read().boundary {
        return None;
    }
    let viewport = *context.stage_size.read();
    let world = screen_to_world(&tab, viewport, point);
    hit_boundary(
        tab.document.pam_bounds(),
        world,
        8.0 / camera_scale(&tab, viewport),
    )
}

fn begin_stage_drag(mut context: AppContext, point: [f64; 2]) -> Option<BoundaryEdge> {
    let tab = context.active_tab_snapshot()?;
    let viewport = *context.stage_size.read();
    let edge = boundary_at(context, point);
    let drag = if let Some(edge) = edge {
        StageDrag::Boundary {
            edge,
            start: point,
            scale: camera_scale(&tab, viewport),
            size: tab.document.pam.size,
            position: tab.document.pam.position,
        }
    } else {
        StageDrag::Pan {
            start: point,
            pan: tab.pan,
        }
    };
    context.stage_drag.set(Some(drag));
    edge
}

fn update_stage_drag(context: AppContext, point: [f64; 2]) {
    let Some(drag) = *context.stage_drag.read() else {
        return;
    };
    let Some(tab) = context.active_tab_snapshot() else {
        return;
    };
    let viewport = *context.stage_size.read();
    let scale = camera_scale(&tab, viewport);
    match drag {
        StageDrag::Pan { start, pan } => {
            context.update_active_tab(|tab| {
                tab.pan = [
                    pan[0] + ((point[0] - start[0]) / scale) as f32,
                    pan[1] + ((point[1] - start[1]) / scale) as f32,
                ];
            });
        }
        StageDrag::Boundary {
            edge,
            start,
            scale,
            size,
            position,
        } => {
            let delta = [(point[0] - start[0]) / scale, (point[1] - start[1]) / scale];
            resize_boundary(context, edge, size, position, delta, viewport, scale);
        }
    }
}

fn resize_boundary(
    context: AppContext,
    edge: BoundaryEdge,
    original_size: [f64; 2],
    original_position: [f64; 2],
    delta: [f64; 2],
    viewport: [f64; 2],
    locked_scale: f64,
) {
    let (position, size) = resized_boundary(edge, original_size, original_position, delta);
    context.update_active_tab(|tab| {
        let document = Arc::make_mut(&mut tab.document);
        document.pam.position = position;
        document.pam.size = size;
        if let Some(scale) = tab.export_scale {
            tab.export_size = [
                (size[0] * scale as f64).round().max(1.0) as u32,
                (size[1] * scale as f64).round().max(1.0) as u32,
            ];
        }
        let fit = camera_fit(tab.document.stage_bounds(), viewport);
        tab.zoom = (locked_scale / fit).clamp(0.0001, 10_000.0) as f32;
    });
}

fn resized_boundary(
    edge: BoundaryEdge,
    original_size: [f64; 2],
    original_position: [f64; 2],
    delta: [f64; 2],
) -> ([f64; 2], [f64; 2]) {
    let west = matches!(
        edge,
        BoundaryEdge::West | BoundaryEdge::NorthWest | BoundaryEdge::SouthWest
    );
    let east = matches!(
        edge,
        BoundaryEdge::East | BoundaryEdge::NorthEast | BoundaryEdge::SouthEast
    );
    let north = matches!(
        edge,
        BoundaryEdge::North | BoundaryEdge::NorthEast | BoundaryEdge::NorthWest
    );
    let south = matches!(
        edge,
        BoundaryEdge::South | BoundaryEdge::SouthEast | BoundaryEdge::SouthWest
    );
    let dx = if west {
        delta[0].min(original_size[0] - 1.0)
    } else {
        delta[0]
    };
    let dy = if north {
        delta[1].min(original_size[1] - 1.0)
    } else {
        delta[1]
    };
    let mut position = original_position;
    let mut size = original_size;
    if west {
        position[0] = original_position[0] - dx;
        size[0] = (original_size[0] - dx).max(1.0);
    } else if east {
        size[0] = (original_size[0] + dx).max(1.0);
    }
    if north {
        position[1] = original_position[1] - dy;
        size[1] = (original_size[1] - dy).max(1.0);
    } else if south {
        size[1] = (original_size[1] + dy).max(1.0);
    }
    (position, size)
}

fn boundary_cursor_class(edge: BoundaryEdge) -> &'static str {
    match edge {
        BoundaryEdge::North | BoundaryEdge::South => "pam-stage--resize-ns",
        BoundaryEdge::East | BoundaryEdge::West => "pam-stage--resize-ew",
        BoundaryEdge::NorthEast | BoundaryEdge::SouthWest => "pam-stage--resize-nesw",
        BoundaryEdge::NorthWest | BoundaryEdge::SouthEast => "pam-stage--resize-nwse",
    }
}

fn is_primary_pointer(event: &PointerEvent) -> bool {
    event.is_primary() && matches!(event.trigger_button(), None | Some(MouseButton::Primary))
}

fn hit_boundary(bounds: Rect, point: [f64; 2], threshold: f64) -> Option<BoundaryEdge> {
    let left = (point[0] - bounds.x as f64).abs() <= threshold;
    let right = (point[0] - (bounds.x + bounds.width) as f64).abs() <= threshold;
    let top = (point[1] - bounds.y as f64).abs() <= threshold;
    let bottom = (point[1] - (bounds.y + bounds.height) as f64).abs() <= threshold;
    let within_x = point[0] >= bounds.x as f64 - threshold
        && point[0] <= (bounds.x + bounds.width) as f64 + threshold;
    let within_y = point[1] >= bounds.y as f64 - threshold
        && point[1] <= (bounds.y + bounds.height) as f64 + threshold;
    match (
        left && within_y,
        right && within_y,
        top && within_x,
        bottom && within_x,
    ) {
        (true, _, true, _) => Some(BoundaryEdge::NorthWest),
        (_, true, true, _) => Some(BoundaryEdge::NorthEast),
        (true, _, _, true) => Some(BoundaryEdge::SouthWest),
        (_, true, _, true) => Some(BoundaryEdge::SouthEast),
        (true, _, _, _) => Some(BoundaryEdge::West),
        (_, true, _, _) => Some(BoundaryEdge::East),
        (_, _, true, _) => Some(BoundaryEdge::North),
        (_, _, _, true) => Some(BoundaryEdge::South),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn west_resize_moves_only_the_left_edge() {
        let (position, size) = resized_boundary(
            BoundaryEdge::West,
            [100.0, 80.0],
            [20.0, 30.0],
            [25.0, 40.0],
        );
        assert_eq!(position, [-5.0, 30.0]);
        assert_eq!(size, [75.0, 80.0]);
    }

    #[test]
    fn north_west_resize_clamps_to_one_world_unit() {
        let (position, size) = resized_boundary(
            BoundaryEdge::NorthWest,
            [100.0, 80.0],
            [20.0, 30.0],
            [500.0, 500.0],
        );
        assert_eq!(position, [-79.0, -49.0]);
        assert_eq!(size, [1.0, 1.0]);
    }

    #[test]
    fn zoom_compensation_keeps_the_camera_scale_stable() {
        let viewport = [1200.0, 700.0];
        let original_scale = camera_fit(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 300.0,
            },
            viewport,
        );
        let next_fit = camera_fit(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 300.0,
            },
            viewport,
        );
        let compensated_zoom = original_scale / next_fit;
        assert!((next_fit * compensated_zoom - original_scale).abs() < f64::EPSILON);
    }
}
