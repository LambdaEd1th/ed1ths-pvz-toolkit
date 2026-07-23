use dioxus::prelude::*;
use pam_viewer_core::{
    ExportRequest, PamDocumentPayload, SpriteKey, WorkerRequest, WorkerResponse,
};

pub use pam_viewer_core::ExportKind;

use crate::i18n::tr;
use crate::state::{AppContext, ExportProgress, Status, Tone, ViewerTab};

pub fn start_export(mut context: AppContext, kind: ExportKind) {
    if context.export.read().is_some() {
        return;
    }
    let Some(tab) = context.active_tab_snapshot() else {
        return;
    };
    context.playing.set(false);
    context.export.set(Some(ExportProgress {
        operation_id: tab.id,
        document_id: tab.processing_id,
        title: format!("{kind:?}"),
        detail: tr(context.preferences.read().locale, "exporting").into(),
        progress: 0.05,
        cancel_requested: false,
    }));
    spawn(async move {
        let result = export_tab(context, &tab, kind).await;
        match result {
            Ok(saved) if saved => context.set_status(Status::new(
                tr(context.preferences.read().locale, "export_complete"),
                Tone::Ok,
            )),
            Ok(_) => {}
            Err(error) => context.set_status(Status::new(error, Tone::Error)),
        }
        context.export.set(None);
    });
}

async fn export_tab(
    mut context: AppContext,
    tab: &ViewerTab,
    kind: ExportKind,
) -> Result<bool, String> {
    let base = strip_animation_extension(&tab.display_name());
    let sprite_name = match tab.active_sprite {
        SpriteKey::Main => "main".into(),
        SpriteKey::Sprite(index) => tab
            .document
            .pam
            .sprite
            .get(index)
            .and_then(|sprite| sprite.name.clone())
            .unwrap_or_else(|| format!("sprite_{index}")),
    };
    let name = match kind {
        ExportKind::Json => format!("{base}.pam.json"),
        ExportKind::Yaml => format!("{base}.pam.yaml"),
        ExportKind::Toml => format!("{base}.pam.toml"),
        ExportKind::Pam => format!("{base}.pam"),
        ExportKind::Fla => format!("{base}.fla"),
        ExportKind::Png => format!("{base}_{sprite_name}.png"),
        ExportKind::Apng => format!("{base}_{sprite_name}.apng"),
        ExportKind::Webp => format!("{base}_{sprite_name}.webp"),
    };
    let request = ExportRequest {
        document_id: tab.processing_id,
        operation_id: tab.id,
        kind,
        sprite: tab.active_sprite,
        current_frame: tab.current_frame,
        frame_range: [tab.frame_range.begin, tab.frame_range.end],
        image_filter: tab.image_filter.clone(),
        sprite_filter: tab.sprite_filter.clone(),
        size: tab.export_size,
        fps: tab.speed_fps,
    };
    let bytes = match export_bytes(tab, request).await {
        Err(error) if error.to_ascii_lowercase().contains("cancelled") => return Ok(false),
        result => result?,
    };
    if context
        .export
        .read()
        .as_ref()
        .is_some_and(|progress| progress.cancel_requested)
    {
        return Ok(false);
    }
    if let Some(progress) = context.export.write().as_mut() {
        progress.progress = 1.0;
    }
    crate::platform::save_bytes(&name, &bytes)
}

async fn export_bytes(tab: &ViewerTab, request: ExportRequest) -> Result<Vec<u8>, String> {
    let response =
        crate::platform::processing::perform(WorkerRequest::Export(request.clone())).await;
    let response = match response {
        Err(error) if error.contains("no active route") || error.contains("is not registered") => {
            match crate::platform::processing::perform(WorkerRequest::RegisterDocument {
                document_id: tab.processing_id,
                document: PamDocumentPayload::from(tab.document.as_ref()),
            })
            .await?
            {
                WorkerResponse::Registered => {}
                _ => {
                    return Err(
                        "Processing Worker returned an unexpected registration response".into(),
                    );
                }
            }
            crate::platform::processing::perform(WorkerRequest::Export(request)).await?
        }
        Ok(response) => response,
        Err(error) => return Err(error),
    };
    match response {
        WorkerResponse::Exported { bytes } => Ok(bytes),
        _ => Err("Processing Worker returned an unexpected export response".into()),
    }
}

fn strip_animation_extension(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for extension in [
        ".pam.json",
        ".pam.yaml",
        ".pam.yml",
        ".pam.toml",
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".pam",
        ".fla",
    ] {
        if lower.ends_with(extension) {
            return name[..name.len() - extension.len()].to_string();
        }
    }
    name.to_string()
}
