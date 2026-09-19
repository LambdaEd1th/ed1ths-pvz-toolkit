//! Read-only logical resource mapping. Physical paths remain authoritative;
//! manifest IDs and atlas children are never treated as independent files.

use crate::domain::ArchiveDocument;
use crate::editing::{PacketEdits, RemovedPackets};
use rsb_archive::{ResourcesDescription, Rsb};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Cursor;
use std::sync::Arc;

const MAX_MANIFEST_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceLocation {
    pub packet_index: usize,
    pub packet_name: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AtlasRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceEntry {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub group: String,
    pub subgroup: String,
    pub resolution: String,
    pub language: String,
    pub parent: String,
    pub atlas: bool,
    pub region: Option<AtlasRegion>,
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ManifestData {
    pub resources: Vec<ResourceEntry>,
    pub sources: Vec<String>,
    pub warnings: Vec<String>,
}

impl ManifestData {
    pub fn merge(&mut self, next: Self) {
        // RTON, NEWTON and embedded descriptions are complementary, not a
        // precedence list. Never replace a complete definition with a sparse one.
        // Conflicting definitions remain separate, with their own provenance.
        let mut indices = self.resources.iter().enumerate().fold(
            HashMap::<_, Vec<usize>>::new(),
            |mut indices, (i, entry)| {
                indices.entry(entry_key(entry)).or_default().push(i);
                indices
            },
        );
        let mut conflicts = 0;
        for entry in next.resources {
            let key = entry_key(&entry);
            let compatible = indices
                .get(&key)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&index| compatible_definition(&self.resources[index], &entry))
                .collect::<Vec<_>>();
            if !compatible.is_empty() {
                // A sparse record can enrich several already distinct variants;
                // it must not collapse those variants into a single definition.
                for index in compatible {
                    complement_definition(&mut self.resources[index], &entry);
                }
            } else {
                if indices.contains_key(&key) {
                    conflicts += 1;
                }
                indices.entry(key).or_default().push(self.resources.len());
                self.resources.push(entry);
            }
        }
        for source in next.sources {
            if !self.sources.contains(&source) {
                self.sources.push(source);
            }
        }
        self.warnings.extend(next.warnings);
        if conflicts > 0 {
            self.warnings.push(format!("清单合并：保留了 {conflicts} 个同 ID 的不同定义，未覆盖其他清单；可在详情中查看各自来源。"));
        }
    }
}

fn compatible_text(left: &str, right: &str) -> bool {
    left.is_empty() || right.is_empty() || left.eq_ignore_ascii_case(right)
}

fn concrete_kind(kind: &str) -> bool {
    !kind.is_empty() && kind != "Unknown" && !kind.starts_with("Type ")
}

fn explicit_group(entry: &ResourceEntry) -> &str {
    if entry.group.eq_ignore_ascii_case(&entry.subgroup) {
        ""
    } else {
        &entry.group
    }
}

fn compatible_definition(left: &ResourceEntry, right: &ResourceEntry) -> bool {
    compatible_text(&normalize_path(&left.path), &normalize_path(&right.path))
        && (!concrete_kind(&left.kind) || !concrete_kind(&right.kind) || left.kind == right.kind)
        && compatible_text(&left.parent, &right.parent)
        && compatible_text(explicit_group(left), explicit_group(right))
        && (left.region.is_none() || right.region.is_none() || left.region == right.region)
        && !(left.atlas && !right.parent.is_empty() || right.atlas && !left.parent.is_empty())
}

fn prefer_spelling(left: &mut String, right: &str) {
    if right.is_empty() {
        return;
    }
    let rank = |value: &str| {
        (
            value == value.to_ascii_lowercase(),
            value != value.to_ascii_uppercase(),
        )
    };
    if left.is_empty()
        || rank(right) > rank(left)
        || (rank(right) == rank(left) && right < left.as_str())
    {
        *left = right.into();
    }
}

fn complement_definition(current: &mut ResourceEntry, incoming: &ResourceEntry) {
    prefer_spelling(&mut current.path, &incoming.path);
    if !concrete_kind(&current.kind) {
        current.kind = incoming.kind.clone();
    }
    if explicit_group(current).is_empty() && !explicit_group(incoming).is_empty() {
        current.group = incoming.group.clone();
    }
    if current.parent.is_empty() {
        current.parent = incoming.parent.clone();
    }
    current.region = current.region.clone().or_else(|| incoming.region.clone());
    current.atlas |= incoming.atlas;
    let sources = current
        .source
        .lines()
        .chain(incoming.source.lines())
        .filter(|source| !source.is_empty())
        .collect::<BTreeSet<_>>();
    current.source = sources.into_iter().collect::<Vec<_>>().join("\n");
}

fn entry_key(entry: &ResourceEntry) -> (String, String, String, String) {
    (
        entry.subgroup.to_ascii_uppercase(),
        entry.id.to_ascii_uppercase(),
        entry.resolution.clone(),
        entry.language.to_ascii_uppercase(),
    )
}

pub fn normalize_path(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_ascii_uppercase()
}

pub fn physical_files(
    archive: &ArchiveDocument,
    edits: &PacketEdits,
    removed: &RemovedPackets,
) -> Vec<ResourceLocation> {
    let pools = archive
        .packets
        .iter()
        .map(|packet| (packet.info.pool_index, packet))
        .collect::<HashMap<_, _>>();
    let mut files = Vec::new();
    for entry in archive.file_index.iter() {
        let Some(packet) = pools.get(&entry.pool_index) else {
            continue;
        };
        if removed.contains(&packet.index) || edits.contains_key(&packet.index) {
            continue;
        }
        files.push(ResourceLocation {
            packet_index: packet.index,
            packet_name: packet.info.name.clone(),
            path: entry.name_path.clone(),
        });
    }
    for (&index, edit) in edits {
        if removed.contains(&index) {
            continue;
        }
        files.extend(edit.document.files.iter().map(|file| ResourceLocation {
            packet_index: index,
            packet_name: edit.document.record.info.name.clone(),
            path: file.path.clone(),
        }));
    }
    files.sort();
    files.dedup();
    files
}

pub fn is_manifest_path(path: &str) -> bool {
    let path = normalize_path(path);
    let name = path.rsplit('/').next().unwrap_or(&path);
    (name.starts_with("RESOURCES") || name.starts_with("RESOURCEGROUP"))
        && [".RTON", ".NEWTON", ".JSON"]
            .iter()
            .any(|extension| name.ends_with(extension))
}

pub fn embedded_manifest(archive: &ArchiveDocument) -> Result<ManifestData, String> {
    let metadata = archive.metadata_bytes()?;
    let metadata_len = metadata.len();
    let mut reader = Rsb::open(Cursor::new(metadata)).map_err(|error| error.to_string())?;
    let header = reader.header.clone();
    let [structure, detail, strings] = [
        header.part1_begin_offset,
        header.part2_begin_offset,
        header.part3_begin_offset,
    ];
    if [structure, detail, strings] == [0, 0, 0] {
        return Ok(ManifestData::default());
    }
    let length = header.information_section_size as usize;
    if structure == 0
        || structure >= detail
        || detail > strings
        || strings as usize >= length
        || length > MAX_MANIFEST_BYTES
        || length > metadata_len
    {
        return Err("内嵌资源描述的区段范围无效或超过 128 MiB 限制".into());
    }
    // Keep all parser seeks within the metadata, never the packet payloads.
    let description = reader
        .read_resources_description("")
        .map_err(|error| error.to_string())?;
    let source = format!("RSB v{} · 内嵌资源描述", header.version);
    Ok(from_description(description, &source))
}

fn from_description(description: ResourcesDescription, source: &str) -> ManifestData {
    let mut resources = Vec::new();
    for (group, composite) in description.groups {
        for (subgroup, simple) in composite.subgroups {
            for (id, item) in simple.resources {
                let mut entry = ResourceEntry {
                    id,
                    path: item.path,
                    // The embedded descriptor uses a different numeric type
                    // namespace from NEWTON (in particular Image is 0, not 1).
                    kind: if item.res_type == 0 {
                        "Image".into()
                    } else {
                        format!("Type {}", item.res_type)
                    },
                    group: group.trim_end_matches("_CompositeShell").into(),
                    subgroup: subgroup.clone(),
                    resolution: simple.res.clone(),
                    language: simple.language.clone(),
                    source: source.into(),
                    ..Default::default()
                };
                if let Some(texture) = item.ptx_info {
                    entry.parent = texture.parent;
                    entry.atlas = entry.parent.is_empty();
                    if !entry.parent.is_empty() {
                        entry.region = texture
                            .ax
                            .parse()
                            .ok()
                            .zip(texture.ay.parse().ok())
                            .zip(texture.aw.parse().ok().zip(texture.ah.parse().ok()))
                            .map(|((x, y), (width, height))| AtlasRegion {
                                x,
                                y,
                                width,
                                height,
                            });
                    }
                }
                resources.push(entry);
            }
        }
    }
    resources.sort_by_key(entry_key);
    ManifestData {
        resources,
        sources: vec![source.into()],
        warnings: Vec::new(),
    }
}

fn resource_kind(value: i32) -> &'static str {
    match value {
        1 => "Image",
        2 => "PopAnim",
        3 => "SoundBank",
        4 => "File",
        5 => "PrimeFont",
        6 => "RenderEffect",
        7 => "DecodedSoundBank",
        _ => "Unknown",
    }
}

fn string(value: &Value, field: &str) -> String {
    match &value[field] {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => String::new(),
    }
}

fn path_string(value: &Value) -> String {
    match value {
        Value::String(path) => path.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("/"),
        _ => String::new(),
    }
}

fn unsigned(value: &Value, field: &str) -> Option<u32> {
    value[field]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value[field].as_str()?.parse().ok())
}

pub fn parse_manifest(name: &str, bytes: &[u8]) -> Result<ManifestData, String> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err("资源清单超过 128 MiB 限制".into());
    }
    let value: Value = if bytes.starts_with(b"RTON") {
        serde_rton::from_bytes(bytes).map_err(|error| format!("RTON 解码失败：{error}"))?
    } else if name.to_ascii_lowercase().ends_with(".newton") {
        let manifest = newton_manifest::from_bytes(bytes)
            .map_err(|error| format!("NEWTON 解码失败：{error}"))?;
        serde_json::to_value(manifest).map_err(|error| error.to_string())?
    } else {
        serde_json::from_slice(bytes)
            .map_err(|error| format!("清单不是可识别的 RTON / NEWTON / JSON：{error}"))?
    };
    parse_manifest_json(name, value)
}

pub(crate) fn parse_manifest_json(name: &str, value: Value) -> Result<ManifestData, String> {
    // Also accept the v3 description.json shape emitted by the RSB codec.
    if value["groups"].is_object() {
        let description: ResourcesDescription = serde_json::from_value(value)
            .map_err(|error| format!("资源描述 JSON 无效：{error}"))?;
        return Ok(from_description(description, name));
    }
    let groups = value["groups"]
        .as_array()
        .ok_or_else(|| "清单缺少 groups 数组；请选择资源清单，而不是普通配置文件".to_string())?;
    let mut parents = HashMap::<String, (String, String)>::new();
    for group in groups {
        if let Some(subgroups) = group["subgroups"].as_array() {
            for subgroup in subgroups {
                parents.insert(
                    string(subgroup, "id"),
                    (string(group, "id"), string(subgroup, "res")),
                );
            }
        }
    }
    let mut output = ManifestData {
        sources: vec![name.into()],
        ..Default::default()
    };
    for group in groups {
        let Some(items) = group["resources"].as_array() else {
            continue;
        };
        let subgroup = string(group, "id");
        if subgroup.is_empty() {
            output
                .warnings
                .push(format!("{name}：跳过没有 ID 的资源组"));
            continue;
        }
        let parent = string(group, "parent");
        let composite = if !parent.is_empty() {
            parent
        } else {
            parents
                .get(&subgroup)
                .map(|p| p.0.clone())
                .unwrap_or_else(|| subgroup.clone())
        };
        let resolution = string(group, "res");
        let resolution = if resolution.is_empty() {
            parents
                .get(&subgroup)
                .map(|p| p.1.clone())
                .unwrap_or_default()
        } else {
            resolution
        };
        for item in items {
            let id = string(item, "id");
            if id.is_empty() {
                output
                    .warnings
                    .push(format!("{name} / {subgroup}：跳过没有 ID 的资源"));
                continue;
            }
            let kind = string(item, "type");
            let kind = kind
                .parse::<i32>()
                .map(|kind| resource_kind(kind).to_string())
                .unwrap_or(kind);
            let parent = string(item, "parent");
            let region = if !parent.is_empty() {
                // NEWTON's optional-nonzero fields omit a coordinate when it
                // equals zero. Width/height are still required; invalid explicit
                // coordinates must not be silently converted to zero.
                (if item["ax"].is_null() {
                    Some(0)
                } else {
                    unsigned(item, "ax")
                })
                .zip(if item["ay"].is_null() {
                    Some(0)
                } else {
                    unsigned(item, "ay")
                })
                .zip(unsigned(item, "aw").zip(unsigned(item, "ah")))
                .map(|((x, y), (width, height))| AtlasRegion {
                    x,
                    y,
                    width,
                    height,
                })
            } else {
                None
            };
            output.resources.push(ResourceEntry {
                id,
                path: path_string(&item["path"]),
                kind,
                group: composite.clone(),
                subgroup: subgroup.clone(),
                resolution: resolution.clone(),
                language: string(group, "loc"),
                parent,
                atlas: item["atlas"].as_bool().unwrap_or(false),
                region,
                source: name.into(),
            });
        }
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingState {
    File,
    AtlasChild,
    Missing,
    Ambiguous,
    Unlisted,
    Program,
}

impl MappingState {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "已映射",
            Self::AtlasChild => "图集子图",
            Self::Missing => "未找到",
            Self::Ambiguous => "多重匹配",
            Self::Unlisted => "未列入清单",
            Self::Program => "程序资源",
        }
    }
    pub fn class(self) -> &'static str {
        match self {
            Self::File | Self::AtlasChild => "mapped",
            Self::Missing | Self::Ambiguous => "warning",
            Self::Unlisted | Self::Program => "unlisted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappedResource {
    pub entry: ResourceEntry,
    pub state: MappingState,
    pub locations: Vec<ResourceLocation>,
    pub explanation: String,
    pub search: String,
}

impl MappedResource {
    fn new(
        entry: ResourceEntry,
        state: MappingState,
        locations: Vec<ResourceLocation>,
        explanation: String,
    ) -> Self {
        let search = format!(
            "{} {} {} {} {} {} {}",
            entry.id,
            entry.path,
            entry.kind,
            entry.group,
            entry.subgroup,
            entry.parent,
            locations
                .iter()
                .map(|location| format!("{} {}", location.packet_name, location.path))
                .collect::<Vec<_>>()
                .join(" ")
        )
        .to_lowercase();
        Self {
            entry,
            state,
            locations,
            explanation,
            search,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceCatalog {
    pub rows: Vec<MappedResource>,
    pub groups: BTreeMap<String, usize>,
    pub kinds: BTreeSet<String>,
    pub mapped: usize,
    pub missing: usize,
    pub ambiguous: usize,
    pub unlisted: usize,
    pub program: usize,
}

fn path_candidates(entry: &ResourceEntry) -> Vec<String> {
    let path = normalize_path(&entry.path);
    if path.is_empty() {
        return Vec::new();
    }
    let mut candidates = vec![path.clone()];
    let extensions: &[&str] = match entry.kind.as_str() {
        "Image" => &["PTX", "PNG", "JPG"],
        "PopAnim" => &["PAM"],
        "SoundBank" | "DecodedSoundBank" => &["BNK"],
        "PrimeFont" => &["TXT", "FNT"],
        "RenderEffect" => &["POPFX"],
        _ => &[],
    };
    let base = if entry.kind == "Image" {
        path.strip_suffix(".PNG").unwrap_or(&path)
    } else {
        &path
    };
    for extension in extensions {
        let candidate = format!("{base}.{extension}");
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    candidates
}

pub fn build_catalog(manifest: &ManifestData, files: &[ResourceLocation]) -> ResourceCatalog {
    let mut paths = HashMap::<String, Vec<&ResourceLocation>>::new();
    let mut stems = HashMap::<String, Vec<&ResourceLocation>>::new();
    for file in files {
        paths
            .entry(normalize_path(&file.path))
            .or_default()
            .push(file);
        let path = normalize_path(&file.path);
        if let Some((stem, extension)) = path.rsplit_once('.')
            && !extension.contains('/')
        {
            stems.entry(stem.to_string()).or_default().push(file);
        }
    }
    let mut ids = HashMap::<String, Vec<&ResourceEntry>>::new();
    for entry in &manifest.resources {
        ids.entry(entry.id.to_ascii_uppercase())
            .or_default()
            .push(entry);
    }
    let mut covered = BTreeSet::new();
    let mut output = ResourceCatalog::default();
    for entry in &manifest.resources {
        if normalize_path(&entry.path) == "!PROGRAM" {
            output.program += 1;
            output.rows.push(MappedResource::new(
                entry.clone(),
                MappingState::Program,
                Vec::new(),
                "清单声明由程序生成，没有独立的包内文件".into(),
            ));
            continue;
        }
        let mut resolved = vec![entry];
        let mut parent_error = None;
        if !entry.parent.is_empty() {
            let parents = ids
                .get(&entry.parent.to_ascii_uppercase())
                .cloned()
                .unwrap_or_default();
            let scoped = parents
                .iter()
                .copied()
                .filter(|parent| parent.subgroup.eq_ignore_ascii_case(&entry.subgroup))
                .filter(|parent| {
                    compatible_text(&parent.resolution, &entry.resolution)
                        && compatible_text(&parent.language, &entry.language)
                })
                .collect::<Vec<_>>();
            // Never silently choose another resolution variant from a different subgroup.
            if scoped.is_empty() {
                parent_error = Some("资源清单中未找到同一子组的父图集");
            } else {
                resolved = scoped
                    .into_iter()
                    .filter(|parent| parent.kind == "Image" && parent.parent.is_empty())
                    .collect();
                if resolved.is_empty() {
                    parent_error = Some("父资源不是独立图像或图集，不能据此定位子图");
                }
            }
        }
        let mut locations = Vec::new();
        if parent_error.is_none() {
            for definition in resolved {
                let start = locations.len();
                for candidate in path_candidates(definition) {
                    locations.extend(
                        paths
                            .get(&candidate)
                            .into_iter()
                            .flatten()
                            .map(|file| (*file).clone()),
                    );
                }
                // Equivalent parent definitions from several manifests may
                // resolve to the same physical atlas; deduplicate below, not by ID.
                if locations.len() == start && definition.kind.starts_with("Type ") {
                    locations.extend(
                        stems
                            .get(&normalize_path(&definition.path))
                            .into_iter()
                            .flatten()
                            .map(|file| (*file).clone()),
                    );
                }
            }
            let scoped = locations
                .iter()
                .filter(|location| location.packet_name.eq_ignore_ascii_case(&entry.subgroup))
                .cloned()
                .collect::<Vec<_>>();
            if !scoped.is_empty() {
                locations = scoped;
            }
            locations.sort();
            locations.dedup();
        }
        let state = match locations.len() {
            0 => MappingState::Missing,
            1 if entry.parent.is_empty() => MappingState::File,
            1 => MappingState::AtlasChild,
            _ => MappingState::Ambiguous,
        };
        let explanation = if let Some(error) = parent_error {
            error.to_string()
        } else {
            match state {
                MappingState::File => {
                    "按完整路径匹配（忽略大小写与分隔符，并补全资源类型对应的扩展名）".into()
                }
                MappingState::AtlasChild => format!(
                    "通过父图集 {} 映射到纹理；子图不是包内独立文件",
                    entry.parent
                ),
                MappingState::Missing => {
                    "清单中的完整路径未匹配到当前包内文件；可能属于其他下载包".into()
                }
                MappingState::Ambiguous => "匹配到多个文件，请在详情中选择；未自动猜测".into(),
                MappingState::Unlisted | MappingState::Program => unreachable!(),
            }
        };
        for location in &locations {
            covered.insert(location.clone());
        }
        match state {
            MappingState::File | MappingState::AtlasChild => output.mapped += 1,
            MappingState::Missing => output.missing += 1,
            MappingState::Ambiguous => output.ambiguous += 1,
            MappingState::Unlisted | MappingState::Program => {}
        }
        output.rows.push(MappedResource::new(
            entry.clone(),
            state,
            locations,
            explanation,
        ));
    }
    for file in files {
        if covered.contains(file) {
            continue;
        }
        output.unlisted += 1;
        let entry = ResourceEntry {
            path: file.path.clone(),
            kind: "File".into(),
            group: "未列入资源清单".into(),
            subgroup: file.packet_name.clone(),
            source: "RSB 文件索引".into(),
            ..Default::default()
        };
        output.rows.push(MappedResource::new(
            entry,
            MappingState::Unlisted,
            vec![file.clone()],
            "包内确实存在，但当前资源清单没有对应条目".into(),
        ));
    }
    output.rows.sort_by(|left, right| {
        (
            &left.entry.group,
            &left.entry.subgroup,
            &left.entry.id,
            &left.entry.path,
        )
            .cmp(&(
                &right.entry.group,
                &right.entry.subgroup,
                &right.entry.id,
                &right.entry.path,
            ))
    });
    for row in &output.rows {
        *output.groups.entry(row.entry.group.clone()).or_default() += 1;
        output.kinds.insert(row.entry.kind.clone());
    }
    output
}

pub async fn load_manifests(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed: RemovedPackets,
) -> ManifestData {
    let mut output = match crate::processing::load_embedded_manifest(archive.clone()).await {
        Ok(data) => data,
        Err(error) => ManifestData {
            warnings: vec![format!("内嵌资源描述读取失败：{error}")],
            ..Default::default()
        },
    };
    let files = physical_files(&archive, &edits, &removed);
    let mut candidates = BTreeMap::<usize, BTreeSet<String>>::new();
    for file in files
        .into_iter()
        .filter(|file| is_manifest_path(&file.path))
    {
        candidates
            .entry(file.packet_index)
            .or_default()
            .insert(normalize_path(&file.path));
    }
    // Some archives omit or damage the top-level file index. Only scan named
    // manifest packets in that case, never decompress the entire RSB eagerly.
    for packet in archive.packets.iter() {
        if packet.info.name.to_ascii_uppercase().contains("MANIFEST")
            && !removed.contains(&packet.index)
        {
            candidates.entry(packet.index).or_default();
        }
    }
    for (packet_index, selected_paths) in candidates {
        let packet = if let Some(edit) = edits.get(&packet_index) {
            Ok(edit.document.clone())
        } else {
            crate::processing::load_manifest_packet(archive.clone(), packet_index)
                .await
                .map(Arc::new)
        };
        match packet {
            Ok(packet) => {
                for file in packet.files.iter().filter(|file| {
                    is_manifest_path(&file.path)
                        || selected_paths.contains(&normalize_path(&file.path))
                }) {
                    match crate::processing::parse_resource_manifest(crate::editing::AddedFile {
                        name: file.path.clone(),
                        data: file.data.clone(),
                    })
                    .await
                    {
                        Ok(data) => output.merge(data),
                        Err(error) => output.warnings.push(format!("{}：{error}", file.path)),
                    }
                }
            }
            Err(error) => output
                .warnings
                .push(format!("RSG #{packet_index}：{error}")),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({ "groups": [
            { "type": "composite", "id": "Plant", "subgroups": [{"id":"Plant_768", "res":"768"}] },
            { "type":"simple", "id":"Plant_768", "resources":[
                {"type":"Image", "id":"ATLAS", "path":["atlases","plant"], "atlas":true},
                {"type":"Image", "id":"LEAF", "path":"images/leaf", "parent":"ATLAS", "ax":2,"ay":3,"aw":8,"ah":9},
                {"type":"PopAnim", "id":"ANIM", "path":"anim/plant"}
            ]}
        ]})
    }
    fn file(packet: &str, path: &str) -> ResourceLocation {
        ResourceLocation {
            packet_index: if packet == "Plant_768" { 0 } else { 1 },
            packet_name: packet.into(),
            path: path.into(),
        }
    }

    #[test]
    fn maps_manifest_ids_extensions_and_atlas_children() {
        let manifest = parse_manifest_json("resources.json", fixture()).unwrap();
        let catalog = build_catalog(
            &manifest,
            &[
                file("Plant_768", "ATLASES\\PLANT.PTX"),
                file("Plant_768", "ANIM/PLANT.PAM"),
                file("Other", "extra.txt"),
            ],
        );
        assert_eq!(
            (catalog.mapped, catalog.missing, catalog.unlisted),
            (3, 0, 1)
        );
        let leaf = catalog
            .rows
            .iter()
            .find(|row| row.entry.id == "LEAF")
            .unwrap();
        assert_eq!(leaf.state, MappingState::AtlasChild);
        assert_eq!(leaf.entry.group, "Plant");
        assert_eq!(leaf.entry.resolution, "768");
        assert_eq!(leaf.entry.region.as_ref().unwrap().width, 8);
        assert_eq!(leaf.locations[0].path, "ATLASES\\PLANT.PTX");
    }

    #[test]
    fn respects_subgroup_and_does_not_guess_ambiguous_or_basename_matches() {
        let manifest = parse_manifest_json("resources.json", fixture()).unwrap();
        let catalog = build_catalog(
            &manifest,
            &[
                file("Other", "ATLASES/PLANT.PTX"),
                file("Third", "ATLASES/PLANT.PTX"),
                file("Other", "elsewhere/PLANT.PAM"),
            ],
        );
        assert_eq!(catalog.ambiguous, 2);
        assert_eq!(catalog.missing, 1);
        let catalog = build_catalog(
            &manifest,
            &[
                file("Plant_768", "ATLASES/PLANT.PTX"),
                file("Other", "ATLASES/PLANT.PTX"),
            ],
        );
        assert_eq!(catalog.mapped, 2);
        assert_eq!(catalog.unlisted, 1);
    }

    #[test]
    fn parses_binary_rton_and_rejects_non_manifests() {
        let bytes = serde_rton::to_bytes(&fixture()).unwrap();
        assert_eq!(
            parse_manifest("RESOURCES.RTON", &bytes)
                .unwrap()
                .resources
                .len(),
            3
        );
        assert!(parse_manifest("config.json", br#"{"objects":[]}"#).is_err());
        assert!(parse_manifest("bad.newton", &[255]).is_err());
    }

    #[test]
    fn missing_parent_does_not_fall_back_to_a_fake_independent_sprite_file() {
        let mut manifest = parse_manifest_json("resources.json", fixture()).unwrap();
        manifest.resources.retain(|entry| entry.id == "LEAF");
        let catalog = build_catalog(&manifest, &[file("Plant_768", "IMAGES/LEAF.PNG")]);
        assert_eq!(
            (catalog.mapped, catalog.missing, catalog.unlisted),
            (0, 1, 1)
        );
    }

    #[test]
    fn rton_and_newton_are_complementary_in_both_orders() {
        let mut rton_json = fixture();
        rton_json["groups"][1]["resources"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"RTON_ONLY","type":"File","path":"config/rton"}));
        let rton =
            parse_manifest("resources.rton", &serde_rton::to_bytes(&rton_json).unwrap()).unwrap();
        let newton: newton_manifest::ResourceManifest = serde_json::from_value(json!({
            "slot_count": 4,
            "groups":[{"type":"simple","id":"Plant_768","parent":"Plant","res":768,"resources":[
                {"type":"Image","slot":0,"id":"ATLAS","path":"atlases/plant","atlas":true},
                {"type":"Image","slot":1,"id":"LEAF","path":"images/leaf","parent":"ATLAS"},
                {"type":"File","slot":2,"id":"NEWTON_ONLY","path":"config/newton"},
                {"type":"Image","slot":3,"id":"NEWTON_CHILD","path":"images/newton","parent":"ATLAS","aw":8,"ah":9}
            ]}]
        })).unwrap();
        let newton = parse_manifest(
            "resources.newton",
            &newton_manifest::to_bytes(&newton).unwrap(),
        )
        .unwrap();
        let mut forward = rton.clone();
        forward.merge(newton.clone());
        let mut reverse = newton.clone();
        reverse.merge(rton);
        forward.resources.sort_by_key(entry_key);
        reverse.resources.sort_by_key(entry_key);
        assert_eq!(forward.resources, reverse.resources);
        assert_eq!(forward.resources.len(), 6);
        assert!(
            forward
                .resources
                .iter()
                .any(|entry| entry.id == "RTON_ONLY")
        );
        assert!(
            forward
                .resources
                .iter()
                .any(|entry| entry.id == "NEWTON_ONLY")
        );
        let leaf = forward
            .resources
            .iter()
            .find(|entry| entry.id == "LEAF")
            .unwrap();
        assert_eq!(
            leaf.region.as_ref().unwrap().x,
            2,
            "sparse NEWTON must not erase RTON crop metadata"
        );
        assert!(leaf.source.contains("resources.rton") && leaf.source.contains("resources.newton"));
        let child = forward
            .resources
            .iter()
            .find(|entry| entry.id == "NEWTON_CHILD")
            .unwrap();
        assert_eq!(
            child.region,
            Some(AtlasRegion {
                x: 0,
                y: 0,
                width: 8,
                height: 9
            })
        );
        forward.merge(newton);
        assert_eq!(
            forward.resources.len(),
            6,
            "repeated scans/imports are idempotent"
        );
        let catalog = build_catalog(&forward, &[file("Plant_768", "ATLASES/PLANT.PTX")]);
        assert_eq!(
            catalog
                .rows
                .iter()
                .find(|row| row.entry.id == "NEWTON_CHILD")
                .unwrap()
                .state,
            MappingState::AtlasChild
        );
    }

    #[test]
    fn conflicting_definitions_are_preserved_and_parent_paths_are_resolved() {
        let mut first = parse_manifest_json("resources.rton", fixture()).unwrap();
        let mut second = parse_manifest_json("resources.newton", fixture()).unwrap();
        second
            .resources
            .iter_mut()
            .find(|entry| entry.id == "ATLAS")
            .unwrap()
            .path = "atlases/other".into();
        first.merge(second.clone());
        assert_eq!(first.resources.len(), 4);
        assert!(!first.warnings.is_empty());
        let catalog = build_catalog(&first, &[file("Plant_768", "ATLASES/PLANT.PTX")]);
        assert_eq!(
            catalog
                .rows
                .iter()
                .find(|row| row.entry.id == "LEAF")
                .unwrap()
                .state,
            MappingState::AtlasChild
        );
        let catalog = build_catalog(
            &first,
            &[
                file("Plant_768", "ATLASES/PLANT.PTX"),
                file("Plant_768", "ATLASES/OTHER.PTX"),
            ],
        );
        assert_eq!(
            catalog
                .rows
                .iter()
                .find(|row| row.entry.id == "LEAF")
                .unwrap()
                .state,
            MappingState::Ambiguous
        );
        first.merge(second);
        assert_eq!(first.resources.len(), 4);
    }

    #[test]
    fn absent_zero_crop_coordinates_are_valid_but_invalid_explicit_values_are_not() {
        let data = parse_manifest_json(
            "resources.json",
            json!({"groups":[{"id":"Images","resources":[
                {"id":"ZERO","type":"Image","parent":"ATLAS","aw":10,"ah":20},
                {"id":"BAD","type":"Image","parent":"ATLAS","ax":-1,"aw":10,"ah":20},
                {"id":"MISSING_SIZE","type":"Image","parent":"ATLAS"}
            ]}]}),
        )
        .unwrap();
        assert_eq!(
            data.resources[0].region,
            Some(AtlasRegion {
                x: 0,
                y: 0,
                width: 10,
                height: 20
            })
        );
        assert!(data.resources[1].region.is_none());
        assert!(data.resources[2].region.is_none());
    }

    #[test]
    fn manifest_merge_preserves_resolution_variants() {
        let mut first = parse_manifest_json("first", fixture()).unwrap();
        let mut second = first.clone();
        for entry in &mut second.resources {
            entry.subgroup = "Plant_1536".into();
            entry.resolution = "1536".into();
        }
        first.merge(second.clone());
        assert_eq!(first.resources.len(), 6);
        first.merge(second);
        assert_eq!(first.resources.len(), 6);
    }

    #[test]
    fn program_resources_are_not_missing_files() {
        let data = parse_manifest_json("resources.json", json!({"groups":[{"id":"Common","resources":[{"id":"DUMMY","type":"Image","path":["!program"]}]}]})).unwrap();
        let catalog = build_catalog(&data, &[]);
        assert_eq!((catalog.program, catalog.missing), (1, 0));
    }

    #[test]
    fn physical_index_uses_pool_ids_and_reflects_pending_edits_and_removals() {
        use crate::domain::{ArchiveChannelOrderMode, PacketDocument, PacketRecord};
        use crate::editing::EditedPacket;
        use crate::loader::ArchiveSource;
        use rsb_archive::{FileListInfo, RsbHeader, RsgInfo, UnpackedFile};
        let record = PacketRecord {
            index: 0,
            header: None,
            error: None,
            info: RsgInfo {
                name: "Group".into(),
                rsg_offset: 0,
                rsg_length: 0,
                pool_index: 42,
                ptx_number: 0,
                ptx_before_number: 0,
                packet_head_info: None,
            },
        };
        let archive = ArchiveDocument {
            display_name: "test".into(),
            byte_len: 0,
            header: RsbHeader::default(),
            resource_count: 1,
            file_index: Arc::new(vec![FileListInfo {
                name_path: "old.pam".into(),
                pool_index: 42,
            }]),
            packets: Arc::new(vec![record.clone()]),
            ptx_infos: Arc::new(Vec::new()),
            warnings: Arc::new(Vec::new()),
            channel_order_mode: ArchiveChannelOrderMode::Auto,
            source: ArchiveSource::Memory(Arc::new(Vec::new())),
            metadata_override: None,
        };
        assert_eq!(
            physical_files(&archive, &PacketEdits::new(), &RemovedPackets::new())[0].packet_index,
            0
        );
        let document = PacketDocument::new(
            record,
            Vec::new(),
            vec![UnpackedFile {
                path: "new.pam".into(),
                data: vec![1],
                is_part1: false,
                part1_info: None,
            }],
        );
        let edits = BTreeMap::from([(
            0,
            EditedPacket {
                original_paths: Arc::new(vec!["old.pam".into()]),
                document: Arc::new(document),
            },
        )]);
        let files = physical_files(&archive, &edits, &RemovedPackets::new());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new.pam");
        assert!(physical_files(&archive, &edits, &BTreeSet::from([0])).is_empty());
    }

    #[test]
    fn reads_v3_embedded_descriptions_with_zero_based_image_type() {
        use rsb_archive::{RSB_MAGIC, RsbHeader, RsbWriter};
        use std::io::{Seek, SeekFrom};
        let description: ResourcesDescription = serde_json::from_value(json!({"groups":{
            "Plant":{"composite":true,"subgroups":{"Plant_768":{"res":"768","language":"enUS","resources":{
                "ATLAS":{"type":0,"path":"atlases/plant","properties":{},"ptx_info":{"imagetype":"0","aflags":"0","x":"0","y":"0","ax":"0","ay":"0","aw":"16","ah":"16","rows":"1","cols":"1","parent":""}},
                "ANIM":{"type":3,"path":"anim/plant","properties":{}},
                "LEAF":{"type":0,"path":"images/leaf","properties":{},"ptx_info":{"imagetype":"0","aflags":"0","x":"0","y":"0","ax":"1","ay":"2","aw":"3","ah":"4","rows":"1","cols":"1","parent":"ATLAS"}}
            }}}}
        }})).unwrap();
        let mut header = RsbHeader {
            magic: RSB_MAGIC,
            version: 3,
            ptx_info_each_length: 16,
            ..Default::default()
        };
        let mut bytes = Cursor::new(vec![0; 0x100]);
        bytes.seek(SeekFrom::Start(0x100)).unwrap();
        RsbWriter::new(&mut bytes)
            .write_resources_description(&description, &mut header)
            .unwrap();
        header.information_section_size = bytes.position() as u32;
        bytes.seek(SeekFrom::Start(0)).unwrap();
        RsbWriter::new(&mut bytes).write_header(&header).unwrap();
        let archive = crate::loader::open_memory("v3.rsb".into(), bytes.into_inner()).unwrap();
        let data = embedded_manifest(&archive).unwrap();
        assert_eq!(data.resources.len(), 3);
        let catalog = build_catalog(
            &data,
            &[
                file("Plant_768", "ATLASES/PLANT.PTX"),
                file("Plant_768", "ANIM/PLANT.PAM"),
            ],
        );
        assert_eq!(catalog.mapped, 3);
        let leaf = catalog
            .rows
            .iter()
            .find(|row| row.entry.id == "LEAF")
            .unwrap();
        assert_eq!(leaf.state, MappingState::AtlasChild);
        assert_eq!(leaf.entry.region.as_ref().unwrap().width, 3);
        assert_eq!(
            catalog
                .rows
                .iter()
                .find(|row| row.entry.id == "ATLAS")
                .unwrap()
                .entry
                .kind,
            "Image"
        );
        assert!(catalog.rows.iter().all(|row| row.entry.language == "enUS"));
    }

    #[test]
    fn reads_newton_binary_manifests() {
        use newton_manifest::{
            Resource, ResourceGroup, ResourceManifest, ResourceType, SimpleGroup,
        };
        let manifest = ResourceManifest {
            slot_count: 1,
            groups: vec![ResourceGroup::Simple(SimpleGroup {
                id: "Plant_768".into(),
                resolution: Some(768),
                parent: None,
                resources: vec![Resource {
                    resource_type: ResourceType::PopAnim,
                    slot: 0,
                    id: "ANIM".into(),
                    path: "anim/plant".into(),
                    width: None,
                    height: None,
                    x: None,
                    y: None,
                    ax: None,
                    ay: None,
                    aw: None,
                    ah: None,
                    cols: None,
                    rows: None,
                    atlas: false,
                    parent: None,
                }],
            })],
        };
        let bytes = newton_manifest::to_bytes(&manifest).unwrap();
        let data = parse_manifest("RESOURCES.NEWTON", &bytes).unwrap();
        let catalog = build_catalog(&data, &[file("Plant_768", "ANIM/PLANT.PAM")]);
        assert_eq!(catalog.mapped, 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn maps_resources_from_a_real_rsb_when_available() -> Result<(), String> {
        let path = std::env::var_os("RSB_RESOURCE_REAL_SAMPLE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../../pvz2/com.popcap.ios.PvZ2.app/main.rsb")
            });
        if !path.exists() {
            eprintln!("skipping resource explorer real sample test");
            return Ok(());
        }
        let archive = crate::loader::open_native(path)?;
        let files = physical_files(&archive, &PacketEdits::new(), &RemovedPackets::new());
        let mut data = embedded_manifest(&archive)?;
        let packets = files
            .iter()
            .filter(|file| is_manifest_path(&file.path))
            .map(|file| file.packet_index)
            .collect::<BTreeSet<_>>();
        for index in packets {
            let packet = archive.load_packet(index)?;
            for file in packet
                .files
                .iter()
                .filter(|file| is_manifest_path(&file.path))
            {
                let incoming = parse_manifest(&file.path, &file.data)?;
                if std::env::var_os("RSB_TRACE_MANIFESTS").is_some() {
                    let previous = data
                        .resources
                        .iter()
                        .map(|entry| (entry_key(entry), entry))
                        .collect::<HashMap<_, _>>();
                    let overlap = incoming
                        .resources
                        .iter()
                        .filter(|entry| previous.contains_key(&entry_key(entry)))
                        .count();
                    let differences = incoming
                        .resources
                        .iter()
                        .filter_map(|entry| {
                            let old = previous.get(&entry_key(entry))?;
                            (normalize_path(&old.path) != normalize_path(&entry.path)
                                || old.region != entry.region
                                || old.parent != entry.parent
                                || old.kind != entry.kind)
                                .then_some((old, entry))
                        })
                        .collect::<Vec<_>>();
                    eprintln!(
                        "manifest {}: {} entries, {} overlapping IDs, {} changed definitions",
                        file.path,
                        incoming.resources.len(),
                        overlap,
                        differences.len()
                    );
                    for (old, next) in differences.iter().take(8) {
                        eprintln!("manifest conflict: {old:?} -> {next:?}");
                    }
                }
                data.merge(incoming);
            }
        }
        let catalog = build_catalog(&data, &files);
        eprintln!(
            "real RSB v{}: files={}, resources={}, mapped={}, missing={}, ambiguous={}, unlisted={}, program={}",
            archive.header.version,
            files.len(),
            data.resources.len(),
            catalog.mapped,
            catalog.missing,
            catalog.ambiguous,
            catalog.unlisted,
            catalog.program
        );
        for row in catalog
            .rows
            .iter()
            .filter(|row| row.state == MappingState::Missing)
            .take(8)
        {
            eprintln!(
                "unmapped: {} {} {} ({})",
                row.entry.id, row.entry.kind, row.entry.path, row.entry.subgroup
            );
        }
        assert!(data.resources.len() > 100);
        assert!(catalog.mapped > 100);
        assert!(
            catalog
                .rows
                .iter()
                .any(|row| row.state == MappingState::AtlasChild)
        );
        let tree = crate::resource_browser::DirectoryIndex::build(&catalog);
        assert_eq!(tree.directories[""].resources, catalog.rows.len());
        let root_items = tree.items(&catalog, "", Default::default());
        assert_eq!(
            root_items.iter().map(|item| item.count).sum::<usize>(),
            catalog.rows.len()
        );
        // Sample real folders, including duplicate-path resolution variants.
        for path in tree.directories.keys().take(16) {
            let leaves = tree
                .items(&catalog, path, Default::default())
                .iter()
                .filter(|item| {
                    matches!(
                        item.target,
                        crate::resource_browser::BrowserTarget::Resource(_)
                    )
                })
                .count();
            assert_eq!(
                leaves,
                tree.paths
                    .iter()
                    .filter(|entry| entry.folder == *path)
                    .count()
            );
        }
        Ok(())
    }
}
