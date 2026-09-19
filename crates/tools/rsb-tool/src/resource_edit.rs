use crate::{
    domain::{ArchiveDocument, PacketDocument},
    editing::{self, AddedFile, PacketEdits, RemovedPackets},
    manifest_edit::ManifestDocument,
    resource_actions::ResourceReader,
    resources::{self, MappedResource, MappingState, ResourceCatalog, ResourceEntry},
};
use rsb_archive::{Rsb, RsbPtxInfo, RsbWriter, UnpackedFile};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn catalog(archive: Arc<ArchiveDocument>, edits: &PacketEdits) -> Arc<ResourceCatalog> {
        let manifest = pollster::block_on(resources::load_manifests(
            archive.clone(),
            edits.clone(),
            RemovedPackets::new(),
        ));
        Arc::new(resources::build_catalog(
            &manifest,
            &resources::physical_files(&archive, edits, &RemovedPackets::new()),
        ))
    }
    fn entry(catalog: &ResourceCatalog, id: &str) -> ResourceEntry {
        catalog
            .rows
            .iter()
            .find(|r| r.entry.id == id)
            .unwrap()
            .entry
            .clone()
    }
    fn run(
        archive: Arc<ArchiveDocument>,
        edits: PacketEdits,
        change: ResourceChange,
    ) -> Result<ResourceCommit, String> {
        let catalog = catalog(archive.clone(), &edits);
        let ptx = archive.ptx_infos.as_ref().clone();
        pollster::block_on(apply(
            archive,
            edits,
            RemovedPackets::new(),
            ptx,
            catalog,
            change,
        ))
    }
    fn save(archive: &ArchiveDocument, commit: &ResourceCommit) -> Arc<ArchiveDocument> {
        let mut archive = archive.clone();
        archive.metadata_override = commit.metadata.clone();
        let edit = editing::archive_edit(
            &commit.edits,
            &RemovedPackets::new(),
            archive.packets.len(),
            &commit.ptx,
            &archive.ptx_infos,
        );
        let bytes = crate::archive_save::rebuild_native(&archive, &edit).unwrap();
        Arc::new(crate::loader::open_memory("edited.rsb".into(), bytes).unwrap())
    }
    fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba(color));
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn rename_atlas_updates_both_manifests_and_preserves_unknown_fields() {
        let archive = crate::edit_fixture::archive();
        let catalog = catalog(archive.clone(), &PacketEdits::new());
        assert_eq!(catalog.mapped, 3);
        let before = entry(&catalog, "ATLAS");
        let mut after = before.clone();
        after.id = "RENAMED_ATLAS".into();
        let commit = run(
            archive.clone(),
            PacketEdits::new(),
            ResourceChange {
                before: Some(before),
                after: Some(after),
                replacement: None,
            },
        )
        .unwrap();
        assert_eq!(commit.edits.len(), 1);
        for file in commit.edits[&0].document.files.iter() {
            let decoded = ManifestDocument::decode(&file.path, &file.data).unwrap();
            assert_eq!(
                decoded.value["groups"][1]["resources"][0]["id"],
                "RENAMED_ATLAS"
            );
            assert_eq!(
                decoded.value["groups"][1]["resources"][1]["parent"],
                "RENAMED_ATLAS"
            );
            assert_eq!(decoded.value["groups"][1]["resources"][1]["slot"], 1);
            assert_eq!(decoded.value["groups"][1]["resources"][1]["x"], -1);
            if file.path.ends_with("rton") {
                assert_eq!(decoded.value["unknown"]["keep"], true);
                assert_eq!(
                    decoded.value["groups"][1]["resources"][1]["custom"]["not_exposed"],
                    "preserve"
                );
            }
        }
        let reopened = save(&archive, &commit);
        assert_eq!(
            super::tests::catalog(reopened, &PacketEdits::new()).mapped,
            3
        );
    }

    #[test]
    fn addition_and_delete_sync_slots_but_keep_physical_files() {
        let archive = crate::edit_fixture::archive();
        let mut after = entry(&catalog(archive.clone(), &PacketEdits::new()), "CONFIG");
        after.id = "ADDED".into();
        after.path = "data/added.txt".into();
        let commit = run(
            archive.clone(),
            PacketEdits::new(),
            ResourceChange {
                before: None,
                after: Some(after),
                replacement: Some(AddedFile {
                    name: "added.txt".into(),
                    data: b"added".to_vec(),
                }),
            },
        )
        .unwrap();
        assert_eq!(commit.edits.len(), 2);
        for file in commit.edits[&0].document.files.iter() {
            let decoded = ManifestDocument::decode(&file.path, &file.data).unwrap();
            assert_eq!(decoded.value["slot_count"], 4);
            assert_eq!(decoded.value["groups"][2]["resources"][1]["slot"], 3);
        }
        let archive = save(&archive, &commit);
        let before = entry(&catalog(archive.clone(), &PacketEdits::new()), "ADDED");
        let commit = run(
            archive.clone(),
            PacketEdits::new(),
            ResourceChange {
                before: Some(before),
                after: None,
                replacement: None,
            },
        )
        .unwrap();
        let archive = save(&archive, &commit);
        let catalog = catalog(archive.clone(), &PacketEdits::new());
        assert!(!catalog.rows.iter().any(|r| r.entry.id == "ADDED"));
        assert!(catalog.rows.iter().any(|r| {
            r.state == MappingState::Unlisted
                && r.locations
                    .iter()
                    .any(|l| l.path.eq_ignore_ascii_case("data/added.txt"))
        }));
        for file in archive.load_packet(0).unwrap().files.iter() {
            assert_eq!(
                ManifestDocument::decode(&file.path, &file.data)
                    .unwrap()
                    .value["slot_count"],
                4
            );
        }
    }

    #[test]
    fn transparent_child_replacement_overwrites_pixels_and_preserves_neighbors() {
        let archive = crate::edit_fixture::archive();
        let before = entry(&catalog(archive.clone(), &PacketEdits::new()), "LEAF");
        let commit = run(
            archive.clone(),
            PacketEdits::new(),
            ResourceChange {
                before: Some(before.clone()),
                after: Some(before),
                replacement: Some(AddedFile {
                    name: "leaf.png".into(),
                    data: png(2, 2, [0, 0, 0, 0]),
                }),
            },
        )
        .unwrap();
        assert_eq!(commit.edits.len(), 1);
        assert!(commit.edits.contains_key(&1));
        let archive = save(&archive, &commit);
        let atlas = entry(&catalog(archive.clone(), &PacketEdits::new()), "ATLAS");
        let catalog = catalog(archive.clone(), &PacketEdits::new());
        let row = catalog.rows.iter().find(|r| r.entry == atlas).unwrap();
        let mut reader = ResourceReader::new(archive, PacketEdits::new(), RemovedPackets::new());
        let png = pollster::block_on(reader.png(row)).unwrap();
        let image = image::load_from_memory(&png).unwrap().into_rgba8();
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(
                    image.get_pixel(x, y).0,
                    if (1..3).contains(&x) && (1..3).contains(&y) {
                        [0, 0, 0, 0]
                    } else {
                        [20, 40, 60, 255]
                    }
                );
            }
        }
    }

    #[test]
    fn invalid_edits_are_rejected_without_touching_the_archive() {
        let archive = crate::edit_fixture::archive();
        let original = archive.source_bytes().unwrap();
        let catalog = catalog(archive.clone(), &PacketEdits::new());
        let before = entry(&catalog, "LEAF");
        let mut after = before.clone();
        after.region.as_mut().unwrap().x = 4;
        assert!(
            run(
                archive.clone(),
                PacketEdits::new(),
                ResourceChange {
                    before: Some(before.clone()),
                    after: Some(after),
                    replacement: None
                }
            )
            .err()
            .unwrap()
            .contains("超出父图集")
        );
        assert!(
            run(
                archive.clone(),
                PacketEdits::new(),
                ResourceChange {
                    before: Some(before.clone()),
                    after: Some(before),
                    replacement: Some(AddedFile {
                        name: "wrong.png".into(),
                        data: png(1, 1, [255; 4])
                    })
                }
            )
            .err()
            .unwrap()
            .contains("必须保持")
        );
        assert!(
            run(
                archive.clone(),
                PacketEdits::new(),
                ResourceChange {
                    before: Some(entry(&catalog, "ATLAS")),
                    after: None,
                    replacement: None
                }
            )
            .err()
            .unwrap()
            .contains("仍有子图引用")
        );
        assert_eq!(archive.source_bytes().unwrap(), original);
    }

    #[test]
    fn ordinary_content_replacement_does_not_modify_manifests() {
        let archive = crate::edit_fixture::archive();
        let before = entry(&catalog(archive.clone(), &PacketEdits::new()), "CONFIG");
        let commit = run(
            archive.clone(),
            PacketEdits::new(),
            ResourceChange {
                before: Some(before.clone()),
                after: Some(before),
                replacement: Some(AddedFile {
                    name: "replacement.txt".into(),
                    data: b"replacement".to_vec(),
                }),
            },
        )
        .unwrap();
        assert_eq!(commit.edits.len(), 1);
        assert!(commit.edits.contains_key(&2));
        let archive = save(&archive, &commit);
        assert_eq!(
            archive.load_packet(2).unwrap().files[0].data,
            b"replacement"
        );
    }

    #[test]
    fn embedded_description_and_both_binary_manifests_save_together() {
        let archive = crate::edit_fixture::with_description();
        let before = entry(&catalog(archive.clone(), &PacketEdits::new()), "ATLAS");
        assert_eq!(before.source.lines().count(), 3);
        let mut after = before.clone();
        after.id = "RENAMED".into();
        let commit = run(
            archive.clone(),
            PacketEdits::new(),
            ResourceChange {
                before: Some(before),
                after: Some(after),
                replacement: None,
            },
        )
        .unwrap();
        assert!(commit.message.contains("3 份清单"));
        let saved = save(&archive, &commit);
        let resources = catalog(saved.clone(), &PacketEdits::new());
        assert_eq!(entry(&resources, "LEAF").parent, "RENAMED");
        assert_eq!(entry(&resources, "RENAMED").source.lines().count(), 3);
        let mut reader = Rsb::open(Cursor::new(saved.metadata_bytes().unwrap())).unwrap();
        let description = reader.read_resources_description("").unwrap();
        let atlas = &description.groups["Textures"].subgroups["Textures_768"].resources["RENAMED"];
        assert_eq!(atlas.properties["keep"], "yes");
        assert_eq!(atlas.ptx_info.as_ref().unwrap().aw, "4");
    }

    #[test]
    fn edits_and_reopens_real_international_manifest_when_requested() {
        let Ok(path) = std::env::var("RSB_RESOURCE_REAL_SAMPLE") else {
            return;
        };
        let archive = Arc::new(crate::loader::open_native(path.into()).unwrap());
        let resources = catalog(archive.clone(), &PacketEdits::new());
        let before = entry(&resources, "IMAGE_MAINMENU_BACKGROUND");
        assert_eq!(before.source.lines().count(), 2);
        let mut after = before.clone();
        after.id = "IMAGE_MAINMENU_BACKGROUND_EDIT_TEST".into();
        let commit = run(
            archive.clone(),
            PacketEdits::new(),
            ResourceChange {
                before: Some(before),
                after: Some(after),
                replacement: None,
            },
        )
        .unwrap();
        assert!(commit.message.contains("2 份清单"));
        let saved = save(&archive, &commit);
        let resources = catalog(saved, &PacketEdits::new());
        assert!(
            resources
                .rows
                .iter()
                .any(|r| r.entry.id == "IMAGE_MAINMENU_BACKGROUND_EDIT_TEST"
                    && r.state == MappingState::AtlasChild
                    && r.entry.source.lines().count() == 2)
        );
        assert!(
            !resources
                .rows
                .iter()
                .any(|r| r.entry.id == "IMAGE_MAINMENU_BACKGROUND")
        );
    }

    #[test]
    fn unlisted_file_can_be_replaced_without_a_logical_id() {
        let archive = crate::edit_fixture::archive();
        let catalog = catalog(archive.clone(), &PacketEdits::new());
        let before = catalog
            .rows
            .iter()
            .find(|r| r.state == MappingState::Unlisted && r.entry.path.ends_with("newton"))
            .unwrap()
            .entry
            .clone();
        assert!(before.id.is_empty());
        let commit = run(
            archive,
            PacketEdits::new(),
            ResourceChange {
                before: Some(before.clone()),
                after: Some(before),
                replacement: Some(AddedFile {
                    name: "replacement.newton".into(),
                    data: b"raw user content".to_vec(),
                }),
            },
        )
        .unwrap();
        assert_eq!(commit.edits.len(), 1);
        assert!(
            commit.edits[&0]
                .document
                .files
                .iter()
                .any(|f| f.path.ends_with("newton") && f.data == b"raw user content")
        );
    }

    #[test]
    fn a_corrupt_mirror_blocks_definition_edits_without_partial_changes() {
        let archive = crate::edit_fixture::archive();
        let packet = archive.load_packet(0).unwrap();
        let mut files = packet.files.as_ref().clone();
        files
            .iter_mut()
            .find(|f| f.path.ends_with("newton"))
            .unwrap()
            .data = b"invalid".to_vec();
        let rebuilt =
            editing::repack_packet(&packet, packet.record.info.name.clone(), 3, files).unwrap();
        let mut edits = PacketEdits::new();
        editing::record_edit(&mut edits, &packet, rebuilt);
        let before = entry(&catalog(archive.clone(), &edits), "CONFIG");
        let mut after = before.clone();
        after.id = "CHANGED".into();
        let original_rton = edits[&0]
            .document
            .files
            .iter()
            .find(|f| f.path.ends_with("rton"))
            .unwrap()
            .data
            .clone();
        assert!(
            run(
                archive,
                edits.clone(),
                ResourceChange {
                    before: Some(before),
                    after: Some(after),
                    replacement: None
                }
            )
            .err()
            .unwrap()
            .contains("无法安全同步")
        );
        assert_eq!(
            edits[&0]
                .document
                .files
                .iter()
                .find(|f| f.path.ends_with("rton"))
                .unwrap()
                .data,
            original_rton
        );
    }
}
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Seek, SeekFrom},
    sync::Arc,
};

#[derive(Clone)]
pub struct ResourceChange {
    pub before: Option<ResourceEntry>,
    pub after: Option<ResourceEntry>,
    pub replacement: Option<AddedFile>,
}

#[derive(Clone)]
pub struct ResourceCommit {
    pub identity: usize,
    pub before_metadata: Option<Arc<Vec<u8>>>,
    pub before_edits: PacketEdits,
    pub before_removed: RemovedPackets,
    pub before_ptx: Vec<RsbPtxInfo>,
    pub edits: PacketEdits,
    pub ptx: Vec<RsbPtxInfo>,
    pub metadata: Option<Arc<Vec<u8>>>,
    pub message: String,
}

fn same_key(a: &ResourceEntry, b: &ResourceEntry) -> bool {
    a.id.eq_ignore_ascii_case(&b.id)
        && a.subgroup.eq_ignore_ascii_case(&b.subgroup)
        && a.resolution == b.resolution
        && a.language.eq_ignore_ascii_case(&b.language)
}

pub fn validate(change: &ResourceChange, catalog: &ResourceCatalog) -> Result<(), String> {
    if change
        .replacement
        .as_ref()
        .is_some_and(|file| file.data.len() > 128 * 1024 * 1024)
    {
        return Err("替换文件不能超过 128 MiB".into());
    }
    if change.before.is_none() && change.after.is_none() {
        return Err("没有资源编辑内容".into());
    }
    // Physical/unlisted files have no logical ID. Content replacement must not
    // require creating a definition or normalizing the existing manifest first.
    if change.before == change.after {
        if change.replacement.is_none() {
            return Err("没有需要应用的修改".into());
        }
        return if catalog.rows.iter().any(|row| {
            change.before.as_ref() == Some(&row.entry) && crate::resource_actions::can_extract(row)
        }) {
            Ok(())
        } else {
            Err("资源内容必须能够唯一定位到包内文件".into())
        };
    }
    if let Some(before) = &change.before
        && catalog.rows.iter().any(|row| {
            row.state != MappingState::Unlisted
                && same_key(&row.entry, before)
                && row.entry != *before
        })
    {
        return Err("源清单存在同 ID 的冲突定义，不能安全同步；请先在清单编辑器中消除冲突".into());
    }
    if let Some(after) = &change.after {
        for (name, value) in [
            ("ID", &after.id),
            ("资源组", &after.group),
            ("子组", &after.subgroup),
        ] {
            if value.trim().is_empty()
                || value != value.trim()
                || value.chars().any(char::is_control)
            {
                return Err(format!("{name} 不能为空、包含控制字符或首尾空格"));
            }
        }
        if after.path.starts_with(['/', '\\'])
            || after.path.split(['/', '\\']).any(|p| p == "..")
            || after.path.contains(':')
            || after.path.chars().any(char::is_control)
        {
            return Err("逻辑路径必须是安全的归档相对路径".into());
        }
        if after.atlas && !after.parent.is_empty() {
            return Err("资源不能同时是图集和图集子图".into());
        }
        if !after.parent.is_empty() && after.parent.eq_ignore_ascii_case(&after.id) {
            return Err("资源不能把自己设置为父图集".into());
        }
        if after.atlas && after.kind != "Image" {
            return Err("父图集必须使用 Image 类型".into());
        }
        if !after.parent.is_empty()
            && (after.kind != "Image"
                || after.region.as_ref().is_none_or(|r| {
                    r.width == 0
                        || r.height == 0
                        || r.x.checked_add(r.width).is_none()
                        || r.y.checked_add(r.height).is_none()
                }))
        {
            return Err("图集子图需要 Image 类型和有效的裁剪区域".into());
        }
        if !after.resolution.is_empty() && after.resolution.parse::<u32>().is_err() {
            return Err("分辨率必须为非负整数或留空，0 表示默认值".into());
        }
        if after.language.len() > 4 || !after.language.is_ascii() {
            return Err("语言代码最多为 4 个 ASCII 字符".into());
        }
        if catalog
            .rows
            .iter()
            .filter(|r| r.state != MappingState::Unlisted)
            .any(|r| same_key(&r.entry, after) && change.before.as_ref() != Some(&r.entry))
        {
            return Err("目标子组/分辨率/语言中已有同 ID 的资源，请先解决冲突".into());
        }
    }
    if let Some(before) = &change.before {
        let children = catalog.rows.iter().any(|r| {
            r.entry.parent.eq_ignore_ascii_case(&before.id)
                && r.entry.subgroup.eq_ignore_ascii_case(&before.subgroup)
                && r.entry.resolution == before.resolution
        });
        if children
            && change.after.as_ref().is_none_or(|after| {
                after.subgroup != before.subgroup
                    || after.group != before.group
                    || after.resolution != before.resolution
                    || after.language != before.language
                    || !after.atlas
                    || after.kind != "Image"
            })
        {
            return Err(
                "这个图集仍有子图引用，不能删除、移组或改成非图集；请先处理子图定义".into(),
            );
        }
    }
    Ok(())
}

struct PacketDraft {
    original: Arc<PacketDocument>,
    files: Vec<UnpackedFile>,
}

async fn packet(
    archive: &Arc<ArchiveDocument>,
    edits: &PacketEdits,
    index: usize,
) -> Result<Arc<PacketDocument>, String> {
    if let Some(edit) = edits.get(&index) {
        return Ok(edit.document.clone());
    }
    crate::processing::load_manifest_packet(archive.clone(), index)
        .await
        .map(Arc::new)
}

pub async fn apply(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed: RemovedPackets,
    ptx: Vec<RsbPtxInfo>,
    catalog: Arc<ResourceCatalog>,
    change: ResourceChange,
) -> Result<ResourceCommit, String> {
    validate(&change, &catalog)?;
    let mut commit = ResourceCommit {
        identity: archive.identity(),
        before_metadata: archive.metadata_override.clone(),
        before_edits: edits.clone(),
        before_removed: removed.clone(),
        before_ptx: ptx.clone(),
        edits: edits.clone(),
        ptx,
        metadata: archive.metadata_override.clone(),
        message: String::new(),
    };
    let mut drafts = BTreeMap::<usize, PacketDraft>::new();
    let definition_changed = change.before != change.after;
    if definition_changed
        && let Some(after) = &change.after
        && change.before.as_ref().is_none_or(|before| {
            before.parent != after.parent
                || before.region != after.region
                || before.subgroup != after.subgroup
                || before.resolution != after.resolution
                || before.language != after.language
        })
    {
        validate_region(&archive, &edits, &removed, &commit.ptx, &catalog, after).await?;
    }
    let mut manifest_count = 0;
    if definition_changed {
        let mut indices = resources::physical_files(&archive, &edits, &removed)
            .into_iter()
            .filter(|f| resources::is_manifest_path(&f.path))
            .map(|f| f.packet_index)
            .collect::<BTreeSet<_>>();
        indices.extend(
            archive
                .packets
                .iter()
                .filter(|p| {
                    p.info.name.to_ascii_uppercase().contains("MANIFEST")
                        && !removed.contains(&p.index)
                })
                .map(|p| p.index),
        );
        let mut documents = Vec::new();
        let mut slot = 0;
        for index in indices {
            let packet = packet(&archive, &edits, index).await?;
            for (file_index, file) in packet
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| resources::is_manifest_path(&f.path))
            {
                let document = ManifestDocument::decode(&file.path, &file.data)
                    .map_err(|e| format!("{} 无法安全同步：{e}", file.path))?;
                slot = slot.max(crate::manifest_edit::next_slot(&document.value));
                documents.push((index, file_index, file.path.clone(), document));
            }
            drafts.insert(
                index,
                PacketDraft {
                    files: packet.files.as_ref().clone(),
                    original: packet,
                },
            );
        }
        for (index, file_index, path, mut document) in documents {
            if change.before.is_none()
                && change
                    .after
                    .as_ref()
                    .is_some_and(|e| !e.source.lines().any(|source| source == path))
            {
                continue;
            }
            if document.apply(change.before.as_ref(), change.after.as_ref(), slot)? != 0 {
                drafts.get_mut(&index).unwrap().files[file_index].data = document.encode()?;
                manifest_count += 1;
            }
        }
        let metadata = archive.metadata_bytes()?;
        let mut reader = Rsb::open(Cursor::new(&metadata)).map_err(|e| e.to_string())?;
        let mut header = reader.header.clone();
        if header.part1_begin_offset != 0 {
            let mut description = serde_json::to_value(
                reader
                    .read_resources_description("")
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let source = format!("RSB v{} · 内嵌资源描述", header.version);
            if change.before.is_some()
                || change
                    .after
                    .as_ref()
                    .is_some_and(|e| e.source.lines().any(|s| s == source))
            {
                let bytes = serde_json::to_vec(&description).map_err(|e| e.to_string())?;
                let mut document = ManifestDocument::decode("description.json", &bytes)?;
                if document.apply(change.before.as_ref(), change.after.as_ref(), slot)? != 0 {
                    description = document.value;
                    let description =
                        serde_json::from_value(description).map_err(|e| e.to_string())?;
                    let mut output = Cursor::new(metadata);
                    output.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
                    RsbWriter::new(&mut output)
                        .write_resources_description(&description, &mut header)
                        .map_err(|e| e.to_string())?;
                    header.information_section_size =
                        u32::try_from(output.get_ref().len()).map_err(|_| "资源描述过大")?;
                    output.set_position(0);
                    RsbWriter::new(&mut output)
                        .write_header(&header)
                        .map_err(|e| e.to_string())?;
                    commit.metadata = Some(Arc::new(output.into_inner()));
                    manifest_count += 1;
                }
            }
        }
        if manifest_count == 0 {
            return Err(
                "没有找到可写入的包内清单。外部导入清单仅用于映射；请从包内资源创建或编辑定义。"
                    .into(),
            );
        }
    }

    if let Some(replacement) = &change.replacement {
        let entry = change.after.as_ref().ok_or("删除定义时不能替换内容")?;
        let row = change
            .before
            .as_ref()
            .and_then(|e| catalog.rows.iter().find(|r| &r.entry == e))
            .cloned();
        if let Some(row) = row.filter(|r| {
            matches!(
                r.state,
                MappingState::File | MappingState::AtlasChild | MappingState::Unlisted
            )
        }) {
            replace_content(
                &archive,
                &edits,
                &removed,
                &mut drafts,
                &mut commit.ptx,
                &row,
                replacement,
            )
            .await?;
        } else if !entry.parent.is_empty() {
            return Err("新增子图请先保存定义，再替换它的图片内容".into());
        } else {
            let candidates = editing::visible_packet_records(&archive, &edits, &removed)
                .into_iter()
                .filter(|r| r.info.name.eq_ignore_ascii_case(&entry.subgroup))
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                return Err("添加文件需选择唯一且已存在的 RSG 子组".into());
            }
            let index = candidates[0].index;
            if let std::collections::btree_map::Entry::Vacant(entry) = drafts.entry(index) {
                let packet = packet(&archive, &edits, index).await?;
                entry.insert(PacketDraft {
                    files: packet.files.as_ref().clone(),
                    original: packet,
                });
            }
            let draft = drafts.get_mut(&index).unwrap();
            let path = entry.path.clone();
            if path.is_empty() || !path.rsplit('/').next().unwrap_or("").contains('.') {
                return Err("添加实际文件时，逻辑路径必须包含文件扩展名".into());
            }
            if draft
                .files
                .iter()
                .any(|f| resources::normalize_path(&f.path) == resources::normalize_path(&path))
            {
                return Err("子组中已有相同路径，不能覆盖".into());
            }
            if path.to_ascii_lowercase().ends_with(".ptx") {
                return Err("新增 PTX 请使用包内文件视图的“添加纹理”；此处可添加普通文件或已有图集的子图定义".into());
            }
            draft.files.push(UnpackedFile {
                path,
                data: replacement.data.clone(),
                is_part1: false,
                part1_info: None,
            });
        }
    }
    for (_, draft) in drafts {
        if draft
            .files
            .iter()
            .zip(draft.original.files.iter())
            .all(|(a, b)| a.path == b.path && a.data == b.data)
            && draft.files.len() == draft.original.files.len()
        {
            continue;
        }
        let flags = draft
            .original
            .record
            .header
            .as_ref()
            .ok_or("RSG 包头不可用")?
            .flags;
        let rebuilt = editing::repack_packet(
            &draft.original,
            draft.original.record.info.name.clone(),
            flags,
            draft.files,
        )?;
        editing::record_edit(&mut commit.edits, &draft.original, rebuilt);
        #[cfg(target_arch = "wasm32")]
        gloo_timers::future::TimeoutFuture::new(0).await;
    }
    commit.message = if definition_changed {
        format!("资源定义已更新 · 同步 {manifest_count} 份清单 · 尚未保存 RSB")
    } else {
        "资源内容已替换 · 尚未保存 RSB".into()
    };
    Ok(commit)
}

async fn validate_region(
    archive: &Arc<ArchiveDocument>,
    edits: &PacketEdits,
    removed: &RemovedPackets,
    ptx: &[RsbPtxInfo],
    catalog: &ResourceCatalog,
    after: &ResourceEntry,
) -> Result<(), String> {
    if after.parent.is_empty() {
        return Ok(());
    }
    let candidates = catalog
        .rows
        .iter()
        .filter(|r| {
            r.entry.id.eq_ignore_ascii_case(&after.parent)
                && r.entry.subgroup.eq_ignore_ascii_case(&after.subgroup)
                && r.entry.resolution == after.resolution
                && r.entry.language.eq_ignore_ascii_case(&after.language)
                && r.entry.parent.is_empty()
                && r.entry.kind == "Image"
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err("父图集必须在同一子组、分辨率和语言中存在且唯一".into());
    }
    let mut document = archive.as_ref().clone();
    document.ptx_infos = Arc::new(ptx.to_vec());
    let mut reader =
        ResourceReader::new(Arc::new(document.clone()), edits.clone(), removed.clone());
    let (packet, index) = reader.resolve(candidates[0]).await?;
    let file = &packet.files[index];
    let (width, height) = if file.is_part1 {
        let metadata = document
            .texture_metadata(&packet.record, file)
            .ok_or("父图集缺少 PTX 元数据")?;
        (metadata.width, metadata.height)
    } else {
        image::ImageReader::new(Cursor::new(&file.data))
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .into_dimensions()
            .map_err(|e| e.to_string())?
    };
    let region = after.region.as_ref().ok_or("子图缺少裁剪范围")?;
    if u64::from(region.x) + u64::from(region.width) > u64::from(width)
        || u64::from(region.y) + u64::from(region.height) > u64::from(height)
    {
        return Err(format!("裁剪范围超出父图集 {width} × {height} 像素"));
    }
    Ok(())
}

async fn replace_content(
    archive: &Arc<ArchiveDocument>,
    edits: &PacketEdits,
    removed: &RemovedPackets,
    drafts: &mut BTreeMap<usize, PacketDraft>,
    ptx: &mut [RsbPtxInfo],
    row: &MappedResource,
    replacement: &AddedFile,
) -> Result<(), String> {
    let mut current = archive.as_ref().clone();
    current.ptx_infos = Arc::new(ptx.to_vec());
    let mut reader = ResourceReader::new(Arc::new(current.clone()), edits.clone(), removed.clone());
    let (packet, index) = reader.resolve(row).await?;
    let file = &packet.files[index];
    let mut data = replacement.data.clone();
    if file.is_part1 || row.state == MappingState::AtlasChild || row.entry.atlas {
        let mut atlas_row = row.clone();
        atlas_row.state = MappingState::File;
        let png = reader.png(&atlas_row).await?;
        let mut atlas = image::load_from_memory(&png)
            .map_err(|e| e.to_string())?
            .into_rgba8();
        let pixels = image::load_from_memory(&data)
            .map_err(|e| format!("替换图片必须是有效 PNG/WebP/JPEG：{e}"))?
            .into_rgba8();
        if row.state == MappingState::AtlasChild {
            let region = row.entry.region.as_ref().ok_or("子图缺少裁剪区域")?;
            if pixels.dimensions() != (region.width, region.height) {
                return Err(format!(
                    "子图替换必须保持 {} × {} 像素",
                    region.width, region.height
                ));
            }
            crate::resource_actions::crop(&atlas, region)?;
            image::imageops::replace(
                &mut atlas,
                &pixels,
                i64::from(region.x),
                i64::from(region.y),
            );
        } else {
            if pixels.dimensions() != atlas.dimensions() {
                return Err(format!(
                    "图集替换必须保持 {} × {} 像素",
                    atlas.width(),
                    atlas.height()
                ));
            }
            atlas = pixels;
        }
        let mut png = Cursor::new(Vec::new());
        atlas
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        data = png.into_inner();
        if file.is_part1 {
            let metadata = current
                .texture_metadata(&packet.record, file)
                .ok_or("PTX 元数据缺失")?;
            let info = &metadata.info;
            let encoded =
                crate::processing::encode_texture(rsb_preview_worker::TextureEncodeRequest {
                    source: data,
                    width: metadata.width,
                    height: metadata.height,
                    format: info.format,
                    alpha_size: info.alpha_size,
                    alpha_format: info.alpha_format,
                    pitch: info.pitch,
                    apple_channel_order: crate::preview::archive_uses_apple_channel_order(&current),
                    encoding: rsb_preview_worker::TextureEncoding::Metadata,
                })
                .await?;
            data = encoded.data;
            let info = ptx
                .get_mut(metadata.global_index)
                .ok_or("PTX 元数据索引已失效")?;
            info.format = encoded.format;
            info.alpha_size = encoded.alpha_size;
            info.alpha_format = encoded.alpha_format;
        }
    }
    let draft = drafts
        .entry(packet.record.index)
        .or_insert_with(|| PacketDraft {
            files: packet.files.as_ref().clone(),
            original: packet.clone(),
        });
    draft.files[index].data = data;
    Ok(())
}
