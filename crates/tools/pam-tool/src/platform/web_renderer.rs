use js_sys::{Function, Reflect};
use pam_viewer_core::{RenderScenePayload, RenderViewPayload};
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};

pub fn send_scene(scene: &RenderScenePayload) -> Result<(), String> {
    invoke("render", scene)
}

pub fn send_view(view: &RenderViewPayload) -> Result<(), String> {
    invoke("renderView", view)
}

fn invoke(method: &str, payload: &impl Serialize) -> Result<(), String> {
    let serializer =
        serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
    let payload = payload
        .serialize(&serializer)
        .map_err(|error| error.to_string())?;
    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let runtime =
        Reflect::get(window.as_ref(), &JsValue::from_str("pamStage")).map_err(js_error)?;
    if runtime.is_null() || runtime.is_undefined() {
        return Err("PAM render bridge is not ready".to_string());
    }
    let function = Reflect::get(&runtime, &JsValue::from_str(method))
        .map_err(js_error)?
        .dyn_into::<Function>()
        .map_err(|_| format!("PAM render bridge method {method} is unavailable"))?;
    function.call1(&runtime, &payload).map_err(js_error)?;
    Ok(())
}

fn js_error(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {error:?}"))
}
