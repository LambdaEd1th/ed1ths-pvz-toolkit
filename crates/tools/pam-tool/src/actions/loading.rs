use dioxus::prelude::*;
use dioxus_html::FileData;
use pam_viewer_core::{WorkerInputFile, WorkerRequest, WorkerResponse};

use crate::i18n::tr;
use crate::state::{AppContext, Locale, Status, Tone, ViewerTab};

pub async fn input_files_from_dioxus(files: Vec<FileData>) -> Result<Vec<WorkerInputFile>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        use futures_util::stream::{self, StreamExt};

        let results = stream::iter(files.into_iter().map(|file| async move {
            let path = {
                let path = file.path().to_string_lossy().replace('\\', "/");
                if path.is_empty() { file.name() } else { path }
            };
            let bytes = file.read_bytes().await.map_err(|error| error.to_string())?;
            Ok::<_, String>(WorkerInputFile::new(path, bytes.as_ref().to_vec()))
        }))
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
        let mut output = results.into_iter().collect::<Result<Vec<_>, _>>()?;
        output.sort_by(|left, right| left.path.cmp(&right.path));
        return Ok(output);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut output = Vec::with_capacity(files.len());
        let mut paths = Vec::new();
        for file in files {
            if file.path().is_dir() {
                output.extend(crate::platform::processing::read_folder(file.path()).await?);
                continue;
            }
            let path = {
                let path = file.path().to_string_lossy().replace('\\', "/");
                if path.is_empty() { file.name() } else { path }
            };
            paths.push((path, file.path()));
        }
        output.extend(crate::platform::input_files_from_paths(&paths)?);
        output.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(output)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_folder(context: AppContext, root: std::path::PathBuf) {
    let locale = context.preferences.read().locale;
    context.set_status(Status::new(tr(locale, "loading"), Tone::Neutral));
    spawn(async move {
        match crate::platform::processing::read_folder(root).await {
            Ok(files) if !files.is_empty() => load_inputs(context, files),
            Ok(_) => {}
            Err(error) => context.set_status(Status::new(error, Tone::Error)),
        }
    });
}

pub fn load_inputs(mut context: AppContext, files: Vec<WorkerInputFile>) {
    let locale = context.preferences.read().locale;
    let id = *context.next_tab_id.read();
    context.next_tab_id.set(id.wrapping_add(1).max(1));
    context.set_status(Status::new(tr(locale, "loading"), Tone::Neutral));
    spawn(async move {
        let response = crate::platform::processing::perform(WorkerRequest::Load {
            document_id: id,
            files,
        })
        .await;
        match response {
            Ok(WorkerResponse::Loaded { loaded }) => {
                let tab = match ViewerTab::new(id, *loaded, &context.preferences.read()) {
                    Ok(tab) => tab,
                    Err(error) => {
                        release_document(id);
                        context.set_status(Status::new(error, Tone::Error));
                        return;
                    }
                };
                let display_name = tab.display_name();
                let image_count = tab.document.pam.image.len();
                let loaded_count = tab.loaded_images;
                let sprite_count = tab.document.pam.sprite.len();
                context.tabs.write().push(tab);
                context.active_tab.set(Some(id));
                context.sync_stage();
                let summary = match locale {
                    Locale::ZhCn => format!(
                        "{display_name}：已加载 {loaded_count}/{image_count} 个图像，{sprite_count} 个 Sprite"
                    ),
                    Locale::En => format!(
                        "{display_name}: {loaded_count}/{image_count} images, {sprite_count} sprites"
                    ),
                };
                context.set_status(Status::new(
                    summary,
                    if loaded_count == image_count {
                        Tone::Ok
                    } else {
                        Tone::Warning
                    },
                ));
                context.playing.set(context.preferences.read().autoplay);
            }
            Ok(_) => {
                release_document(id);
                context.set_status(Status::new(
                    "Processing Worker returned an unexpected load response",
                    Tone::Error,
                ));
            }
            Err(error) => {
                context.set_status(Status::new(error, Tone::Error));
            }
        }
    });
}

pub fn release_document(document_id: u64) {
    spawn(async move {
        let _ =
            crate::platform::processing::perform(WorkerRequest::ReleaseDocument { document_id })
                .await;
    });
}
