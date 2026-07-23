use dioxus_html::FileData;

use crate::domain::{is_loadable_display_name, normalize_display_path};

use super::state::{LoadedFileDraft, LoadedFileSource};

#[derive(Debug)]
pub(crate) struct WebDroppedFile {
    display_name: String,
    file: web_sys::File,
    size: Option<usize>,
}

pub(crate) fn loaded_file_draft_from_web_dropped(file: WebDroppedFile) -> LoadedFileDraft {
    LoadedFileDraft {
        display_name: file.display_name,
        source: LoadedFileSource::WebFile(file.file),
        size: file.size,
    }
}

pub(crate) async fn collect_web_dropped_directory_files(
    event: &dioxus_html::DragEvent,
) -> Result<Option<Vec<WebDroppedFile>>, String> {
    let Some(entries) = web_dropped_entries(event)? else {
        return Ok(None);
    };

    let mut files = Vec::new();
    for entry in entries {
        collect_web_entry_files(entry, String::new(), &mut files).await?;
    }
    files.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(Some(files))
}

fn web_dropped_entries(
    event: &dioxus_html::DragEvent,
) -> Result<Option<Vec<wasm_bindgen::JsValue>>, String> {
    let data = event.data();
    let Some(web_event) = data.downcast::<web_sys::DragEvent>() else {
        return Ok(None);
    };
    let Some(data_transfer) = web_event.data_transfer() else {
        return Ok(None);
    };

    let items = data_transfer.items();
    if items.length() == 0 {
        return Ok(None);
    }

    let mut entries = Vec::new();
    let mut has_directory = false;
    for index in 0..items.length() {
        let Some(item) = items.get(index) else {
            continue;
        };
        if item.kind() != "file" {
            continue;
        }
        let Some(entry) = item.webkit_get_as_entry().map_err(js_value_to_string)? else {
            continue;
        };
        let entry = wasm_bindgen::JsValue::from(entry);
        has_directory |= web_entry_is_directory(&entry);
        entries.push(entry);
    }

    if entries.is_empty() || !has_directory {
        Ok(None)
    } else {
        Ok(Some(entries))
    }
}

async fn collect_web_entry_files(
    root: wasm_bindgen::JsValue,
    root_parent: String,
    output: &mut Vec<WebDroppedFile>,
) -> Result<(), String> {
    let mut stack = vec![(root, root_parent)];
    while let Some((entry, parent_path)) = stack.pop() {
        let name = normalize_display_path(&web_entry_name(&entry));
        if name.is_empty() {
            continue;
        }
        let display_path = if parent_path.is_empty() {
            name
        } else {
            format!("{parent_path}/{name}")
        };

        if web_entry_is_file(&entry) {
            if !is_loadable_display_name(&display_path) {
                continue;
            }
            let file = read_web_file_entry(&entry).await?;
            let size = web_file_size(&file);
            output.push(WebDroppedFile {
                display_name: display_path,
                file,
                size,
            });
        } else if web_entry_is_directory(&entry) {
            let mut children = read_web_directory_entries(&entry).await?;
            children.reverse();
            for child in children {
                stack.push((child, display_path.clone()));
            }
        }
    }

    Ok(())
}

fn web_entry_name(entry: &wasm_bindgen::JsValue) -> String {
    js_sys::Reflect::get(entry, &wasm_bindgen::JsValue::from_str("name"))
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_default()
}

fn web_entry_is_file(entry: &wasm_bindgen::JsValue) -> bool {
    web_entry_bool(entry, "isFile")
}

fn web_entry_is_directory(entry: &wasm_bindgen::JsValue) -> bool {
    web_entry_bool(entry, "isDirectory")
}

fn web_entry_bool(entry: &wasm_bindgen::JsValue, property: &str) -> bool {
    js_sys::Reflect::get(entry, &wasm_bindgen::JsValue::from_str(property))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn web_object_method(
    object: &wasm_bindgen::JsValue,
    name: &str,
) -> Result<js_sys::Function, String> {
    use wasm_bindgen::JsCast;

    js_sys::Reflect::get(object, &wasm_bindgen::JsValue::from_str(name))
        .map_err(js_value_to_string)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| format!("Dropped entry method {name} is unavailable"))
}

async fn read_web_file_entry(entry: &wasm_bindgen::JsValue) -> Result<web_sys::File, String> {
    use wasm_bindgen::JsCast;

    let file_method = web_object_method(entry, "file")?;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(error) = file_method.call2(entry, &resolve, &reject) {
            let _ = reject.call1(&wasm_bindgen::JsValue::UNDEFINED, &error);
        }
    });
    let file_value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(js_value_to_string)?;
    file_value
        .dyn_into::<web_sys::File>()
        .map_err(|_| "Dropped file handle did not return a File".to_string())
}

pub(super) fn file_data_web_file(file: &FileData) -> Option<web_sys::File> {
    file.inner().downcast_ref::<web_sys::File>().cloned()
}

pub(super) fn web_file_size(file: &web_sys::File) -> Option<usize> {
    let size = file.size();
    (size.is_finite() && size >= 0.0).then_some(size as usize)
}

pub(crate) async fn read_web_file_bytes(file: &web_sys::File) -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast;

    let blob = wasm_bindgen::JsValue::from(file.clone())
        .dyn_into::<web_sys::Blob>()
        .map_err(|_| "File handle did not expose a Blob".to_string())?;
    let buffer = wasm_bindgen_futures::JsFuture::from(blob.array_buffer())
        .await
        .map_err(js_value_to_string)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

async fn read_web_directory_entries(
    entry: &wasm_bindgen::JsValue,
) -> Result<Vec<wasm_bindgen::JsValue>, String> {
    let reader = web_object_method(entry, "createReader")?
        .call0(entry)
        .map_err(js_value_to_string)?;
    let mut entries = Vec::new();
    loop {
        let batch = read_web_directory_batch(&reader).await?;
        if batch.length() == 0 {
            break;
        }

        for value in batch.iter() {
            entries.push(value);
        }
    }

    Ok(entries)
}

async fn read_web_directory_batch(reader: &wasm_bindgen::JsValue) -> Result<js_sys::Array, String> {
    let read_entries = web_object_method(reader, "readEntries")?;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(error) = read_entries.call2(reader, &resolve, &reject) {
            let _ = reject.call1(&wasm_bindgen::JsValue::UNDEFINED, &error);
        }
    });
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(js_value_to_string)?;
    Ok(js_sys::Array::from(&value))
}

fn js_value_to_string(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {value:?}"))
}
