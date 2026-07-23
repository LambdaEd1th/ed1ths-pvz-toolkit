use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let i18n_dir = manifest_dir.join("assets").join("rton").join("i18n");
    println!("cargo:rerun-if-changed={}", i18n_dir.display());

    let mut file_names = fs::read_dir(&i18n_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| i18n_file_name(&entry.path()))
        .collect::<Vec<_>>();
    file_names.sort();

    for file_name in &file_names {
        println!(
            "cargo:rerun-if-changed={}",
            i18n_dir.join(file_name).display()
        );
    }

    let mut output = String::from("#[allow(dead_code)]\npub const I18N_FILE_NAMES: &[&str] = &[\n");
    for file_name in file_names {
        output.push_str("    \"");
        push_escaped_rust_string(&mut output, &file_name);
        output.push_str("\",\n");
    }
    output.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    fs::write(out_dir.join("i18n_manifest.rs"), output).expect("write i18n manifest");
}

fn i18n_file_name(path: &Path) -> Option<String> {
    let is_ftl = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ftl"));
    if !is_ftl {
        return None;
    }

    path.file_name()?.to_str().map(str::to_string)
}

fn push_escaped_rust_string(output: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            _ => output.push(ch),
        }
    }
}
