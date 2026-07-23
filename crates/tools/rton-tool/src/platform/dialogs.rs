use super::NativeOpenFile;

#[cfg(not(target_arch = "wasm32"))]
pub fn open_files() -> Result<Option<Vec<NativeOpenFile>>, String> {
    let Some(paths) = rfd::FileDialog::new().pick_files() else {
        return Ok(None);
    };

    Ok(Some(
        paths
            .into_iter()
            .map(|path| {
                let display_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| path.display().to_string());
                NativeOpenFile { path, display_name }
            })
            .collect(),
    ))
}

#[cfg(target_arch = "wasm32")]
pub fn open_files() -> Result<Option<Vec<NativeOpenFile>>, String> {
    Ok(None)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_folder() -> Result<Option<Vec<NativeOpenFile>>, String> {
    let Some(directory) = rfd::FileDialog::new().pick_folder() else {
        return Ok(None);
    };

    files_in_folder(&directory).map(Some)
}

#[cfg(target_arch = "wasm32")]
pub fn open_folder() -> Result<Option<Vec<NativeOpenFile>>, String> {
    Ok(None)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn files_in_folder(directory: &std::path::Path) -> Result<Vec<NativeOpenFile>, String> {
    let root_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| directory.display().to_string());
    let mut files = Vec::new();
    collect_loadable_files(directory, directory, &root_name, &mut files)?;
    files.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(files)
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_loadable_files(
    root: &std::path::Path,
    directory: &std::path::Path,
    root_name: &str,
    output: &mut Vec<NativeOpenFile>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(directory).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            collect_loadable_files(root, &path, root_name, output)?;
        } else if file_type.is_file() {
            output.push(NativeOpenFile {
                display_name: display_path_from_root(root, &path, root_name),
                path,
            });
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn display_path_from_root(
    root: &std::path::Path,
    path: &std::path::Path,
    root_name: &str,
) -> String {
    let relative = path
        .strip_prefix(root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| path.display().to_string())
        });
    if root_name.is_empty() {
        relative
    } else {
        format!("{root_name}/{relative}")
    }
}
