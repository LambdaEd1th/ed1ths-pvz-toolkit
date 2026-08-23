use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let i18n_dir = manifest_dir.join("assets").join("i18n");
    println!("cargo:rerun-if-changed={}", i18n_dir.display());

    let mut file_names = fs::read_dir(&i18n_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            let path = entry.path();
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ftl"))
                .then(|| path.file_name()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect::<Vec<_>>();
    file_names.sort();

    let mut output = String::from("pub const I18N_FILE_NAMES: &[&str] = &[\n");
    for file_name in file_names {
        println!(
            "cargo:rerun-if-changed={}",
            i18n_dir.join(&file_name).display()
        );
        output.push_str(&format!("    {file_name:?},\n"));
    }
    output.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    fs::write(out_dir.join("i18n_manifest.rs"), output).expect("write i18n manifest");
}
