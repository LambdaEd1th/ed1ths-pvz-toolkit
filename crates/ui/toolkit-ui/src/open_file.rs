//! In-memory handoff between tools. No temporary files or native path access.
use dioxus::prelude::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Rsb,
    RsbPatch,
    Pak,
    Dzip,
    Smf,
    CompiledText,
    CryptData,
    Rton,
    Pam,
    Particle,
    Reanim,
    Wem,
    Bnk,
    Newton,
}

impl ToolKind {
    pub const ALL: [Self; 14] = [
        Self::Rsb,
        Self::RsbPatch,
        Self::Pak,
        Self::Dzip,
        Self::Smf,
        Self::CompiledText,
        Self::CryptData,
        Self::Rton,
        Self::Pam,
        Self::Particle,
        Self::Reanim,
        Self::Wem,
        Self::Bnk,
        Self::Newton,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rsb => "RSB",
            Self::RsbPatch => "RSB Patch",
            Self::Pak => "PAK",
            Self::Dzip => "DZIP",
            Self::Smf => "SMF",
            Self::CompiledText => "Compiled Text",
            Self::CryptData => "CryptData",
            Self::Rton => "RTON",
            Self::Pam => "PAM",
            Self::Particle => "Particle",
            Self::Reanim => "Reanim",
            Self::Wem => "WEM",
            Self::Bnk => "BNK",
            Self::Newton => "NEWTON",
        }
    }

    pub fn from_path(path: &str) -> Option<Self> {
        let path = path.to_ascii_lowercase();
        if [".pam.json", ".pam.yaml", ".pam.yml", ".pam.toml"]
            .iter()
            .any(|extension| path.ends_with(extension))
        {
            return Some(Self::Pam);
        }
        if path.ends_with(".reanim.compiled") {
            return Some(Self::Reanim);
        }
        if path.ends_with(".xml.compiled") {
            return Some(Self::Particle);
        }
        if path.ends_with(".compiled.txt") {
            return Some(Self::CompiledText);
        }
        match path.rsplit('.').next()? {
            "rsb" | "obb" => Some(Self::Rsb),
            "patch" | "rsbp" | "rsbpatch" => Some(Self::RsbPatch),
            "pak" => Some(Self::Pak),
            "dz" | "dzip" => Some(Self::Dzip),
            "smf" => Some(Self::Smf),
            "ctxt" | "compiledtext" | "compiled" | "txt" => Some(Self::CompiledText),
            "crypt" | "cryptdata" | "cdat" => Some(Self::CryptData),
            "rton" => Some(Self::Rton),
            "pam" => Some(Self::Pam),
            "particle" => Some(Self::Particle),
            "reanim" => Some(Self::Reanim),
            "wem" => Some(Self::Wem),
            "bnk" => Some(Self::Bnk),
            "newton" => Some(Self::Newton),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolFile {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
}

impl ToolFile {
    pub fn new(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            bytes: Arc::new(bytes),
        }
    }

    pub fn into_file_data(self) -> dioxus_html::FileData {
        dioxus_html::FileData::new(dioxus_html::SerializedFileData {
            path: self.name.into(),
            size: self.bytes.len() as u64,
            last_modified: 0,
            content_type: None,
            contents: Some(self.bytes.as_ref().clone().into()),
        })
    }
}

#[derive(Clone)]
pub struct ToolOpenRequest {
    pub id: u64,
    pub kind: ToolKind,
    pub files: Vec<ToolFile>,
}

#[derive(Clone, Copy)]
pub struct ToolOpenBus(pub Signal<Option<ToolOpenRequest>>);

#[derive(Clone, Copy)]
pub struct ToolOpenHandler(pub EventHandler<(ToolKind, Vec<ToolFile>)>);

/// Each destination consumes its own request once, waiting while it is busy.
/// Standalone tools work without a shell context.
pub fn use_tool_open(
    kind: ToolKind,
    busy: Signal<bool>,
    mut open: impl FnMut(Vec<ToolFile>) + 'static,
) {
    let bus = use_hook(try_consume_context::<ToolOpenBus>);
    use_effect(move || {
        let Some(ToolOpenBus(mut requests)) = bus else {
            return;
        };
        if busy() {
            return;
        }
        let request = requests
            .read()
            .as_ref()
            .filter(|request| request.kind == kind)
            .cloned();
        if let Some(request) = request {
            requests.set(None);
            open(request.files);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_compound_extensions_before_generic_containers() {
        assert_eq!(ToolKind::from_path("PLANT.PAM"), Some(ToolKind::Pam));
        assert_eq!(ToolKind::from_path("plant.pam.yaml"), Some(ToolKind::Pam));
        assert_eq!(
            ToolKind::from_path("zombie.reanim.compiled"),
            Some(ToolKind::Reanim)
        );
        assert_eq!(
            ToolKind::from_path("effect.xml.compiled"),
            Some(ToolKind::Particle)
        );
        assert_eq!(
            ToolKind::from_path("lawnstrings.txt.compiled"),
            Some(ToolKind::CompiledText)
        );
        assert_eq!(ToolKind::from_path("patch.rsbp"), Some(ToolKind::RsbPatch));
        assert_eq!(ToolKind::from_path("data.cdat"), Some(ToolKind::CryptData));
        assert_eq!(
            ToolKind::from_path("unknown.json"),
            None,
            "ambiguous schemas require Open with"
        );
        assert_eq!(
            ToolKind::from_path("atlas.ptx"),
            None,
            "textures use resource preview"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn in_memory_handoff_does_not_require_a_native_file() {
        let file = ToolFile::new(
            "archive-only/folder/nonexistent.rton",
            b"RTON-data".to_vec(),
        )
        .into_file_data();
        assert_eq!(file.name(), "nonexistent.rton");
        assert_eq!(file.size(), 9);
        assert_eq!(
            pollster::block_on(file.read_bytes()).unwrap().as_ref(),
            b"RTON-data"
        );
        assert_eq!(pollster::block_on(file.read_string()).unwrap(), "RTON-data");
    }
}
