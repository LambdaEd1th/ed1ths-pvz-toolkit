use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;

use dzip::{
    Archive, ArchiveBuilder, ArchivePathKey, ArchivePreparation, DzOptions, EntryId, EntryOptions,
    MemoryVolumeSink, MemoryVolumeSource, PackOptions,
};

use crate::model::{
    ArchiveSummary, BuildRequest, BuiltArchive, DraftEntry, EntrySummary, MaterializeRequest,
    NamedBytes, SegmentRecipe,
};

type MemoryArchive = Archive<Cursor<Vec<u8>>, MemoryVolumeSource>;

struct ArchiveSession {
    archive: MemoryArchive,
    name: String,
    source_size: u64,
}

#[derive(Default)]
pub(crate) struct ArchiveService {
    next_session_id: u64,
    sessions: HashMap<u64, ArchiveSession>,
}

impl ArchiveService {
    pub(crate) fn open(
        &mut self,
        main_name: String,
        main_bytes: Vec<u8>,
        auxiliary_files: Vec<NamedBytes>,
    ) -> Result<ArchiveSummary, String> {
        let session = ArchiveSession::open(main_name, main_bytes, auxiliary_files)?;
        Ok(self.insert(session))
    }

    pub(crate) fn close(&mut self, session_id: u64) {
        self.sessions.remove(&session_id);
    }

    pub(crate) fn materialize(
        &mut self,
        request: MaterializeRequest,
    ) -> Result<Vec<NamedBytes>, String> {
        request
            .entries
            .into_iter()
            .map(|entry| {
                let path = entry.path.clone();
                let bytes = self.entry_bytes(request.source_session, &entry)?;
                Ok(NamedBytes { name: path, bytes })
            })
            .collect()
    }

    pub(crate) fn build(&mut self, request: BuildRequest) -> Result<BuiltArchive, String> {
        if request.entries.is_empty() {
            return Err("请先向归档中添加至少一个文件".to_string());
        }

        let mut unique_paths = HashSet::with_capacity(request.entries.len());
        for entry in &request.entries {
            let path = entry.path.trim();
            if path.is_empty() {
                return Err("归档路径不能为空".to_string());
            }
            if !unique_paths.insert(ArchivePathKey::from_archive_str(path)) {
                return Err(format!("归档中存在重复路径：{}", entry.path));
            }
        }

        let minimum_volumes = request
            .entries
            .iter()
            .map(|entry| usize::from(entry.volume) + 1)
            .max()
            .unwrap_or(1);
        let volume_count = request.volume_count.max(minimum_volumes).max(1);
        if volume_count > u16::MAX as usize {
            return Err("归档分卷数量超过 65535".to_string());
        }
        let archive_name = normalize_archive_name(&request.archive_name);
        let volume_names = archive_volume_names(&archive_name, volume_count);
        let mut builder = ArchiveBuilder::with_options(PackOptions {
            volume_names,
            alignment: request.alignment,
            dz: DzOptions {
                settings: request.range_settings,
                use_combuf: request.use_common_buffer,
                ..DzOptions::default()
            },
        });

        for entry in request.entries {
            let bytes = self.entry_bytes(request.source_session, &entry)?;
            add_draft_entry(&mut builder, &entry, &bytes)?;
        }

        let mut sink = MemoryVolumeSink::default();
        let report = builder
            .write_to_sink(&mut sink)
            .map_err(|error| error.to_string())?;
        let volumes = (0..report.volumes)
            .map(|index| {
                let id = u16::try_from(index).map_err(|_| "分卷编号溢出".to_string())?;
                Ok(NamedBytes {
                    name: sink
                        .name(id)
                        .ok_or_else(|| format!("分卷 {id} 缺少文件名"))?
                        .to_string(),
                    bytes: sink
                        .volume(id)
                        .ok_or_else(|| format!("分卷 {id} 没有生成数据"))?
                        .to_vec(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let main = volumes
            .first()
            .ok_or_else(|| "归档构建器没有生成主分卷".to_string())?;
        let session = ArchiveSession::open(
            main.name.clone(),
            main.bytes.clone(),
            volumes.iter().skip(1).cloned().collect(),
        )?;
        let summary = self.insert(session);
        Ok(BuiltArchive { volumes, summary })
    }

    fn entry_bytes(
        &mut self,
        source_session: Option<u64>,
        entry: &DraftEntry,
    ) -> Result<Vec<u8>, String> {
        if let Some(bytes) = &entry.bytes {
            return Ok(bytes.to_vec());
        }
        let session_id =
            source_session.ok_or_else(|| format!("{} 没有可用的文件数据或源归档", entry.path))?;
        let source_id = entry
            .source_id
            .ok_or_else(|| format!("{} 缺少源条目编号", entry.path))?;
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("DZip 会话 {session_id} 已失效"))?;
        session
            .archive
            .read_entry(EntryId(source_id))
            .map_err(|error| format!("无法读取 {}：{error}", entry.path))
    }

    fn insert(&mut self, session: ArchiveSession) -> ArchiveSummary {
        self.next_session_id = self.next_session_id.wrapping_add(1).max(1);
        let id = self.next_session_id;
        let summary = session.summary(id);
        self.sessions.insert(id, session);
        summary
    }
}

impl ArchiveSession {
    fn open(
        main_name: String,
        main_bytes: Vec<u8>,
        auxiliary_files: Vec<NamedBytes>,
    ) -> Result<Self, String> {
        let main_size = main_bytes.len() as u64;
        let preparation =
            ArchivePreparation::read(Cursor::new(main_bytes), dzip::ReadOptions::default())
                .map_err(|error| error.to_string())?;
        let volumes =
            match_auxiliary_volumes(&preparation.metadata().volume_files, auxiliary_files)?;
        let source_size = volumes.iter().try_fold(main_size, |total, (_, bytes)| {
            total
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| "归档大小溢出".to_string())
        })?;
        let mut archive = preparation
            .open(MemoryVolumeSource::new(volumes))
            .map_err(|error| error.to_string())?;
        let available = archive.volume_source().available_ids().collect::<Vec<_>>();
        for id in available {
            archive
                .resolve_volume(id)
                .map_err(|error| error.to_string())?;
        }
        Ok(Self {
            archive,
            name: main_name,
            source_size,
        })
    }

    fn summary(&self, session_id: u64) -> ArchiveSummary {
        let index = self.archive.index();
        let chunks = index.resolved_chunks();
        let mut unpacked_size = 0_u64;
        let entries = self
            .archive
            .entries()
            .iter()
            .map(|entry| {
                unpacked_size = unpacked_size.saturating_add(entry.decompressed_size());
                let packed_size = entry
                    .segments()
                    .iter()
                    .all(|segment| self.archive.is_volume_resolved(segment.volume()))
                    .then(|| {
                        entry
                            .chunk_ids()
                            .iter()
                            .filter_map(|id| chunks.get(*id as usize))
                            .map(|chunk| u64::from(chunk.physical_length))
                            .sum()
                    });
                EntrySummary {
                    id: entry.id().0,
                    path: portable_path(entry.path()),
                    size: entry.decompressed_size(),
                    packed_size,
                    compression: entry.compression(),
                    volume: entry.volume(),
                    chunks: entry.segments().len(),
                    segments: entry
                        .segments()
                        .iter()
                        .map(|segment| SegmentRecipe {
                            length: usize::try_from(
                                segment.decoded_range().end - segment.decoded_range().start,
                            )
                            .unwrap_or(usize::MAX),
                            raw_flags: index.stored_chunks()[segment.chunk_id() as usize].flags,
                            volume: segment.volume(),
                        })
                        .collect(),
                }
            })
            .collect();
        let loaded_auxiliary = self.archive.volume_source().available_ids().count();
        ArchiveSummary {
            session_id,
            name: self.name.clone(),
            entries,
            source_size: self.source_size,
            source_complete: loaded_auxiliary == index.volume_files().len(),
            unpacked_size,
            chunk_count: chunks.len(),
            volume_count: index.volume_files().len() + 1,
            loaded_volume_count: loaded_auxiliary + 1,
            range_settings: index.range_settings().unwrap_or_default(),
            use_common_buffer: index.has_dz_common_buffer(),
        }
    }
}

fn add_draft_entry(
    builder: &mut ArchiveBuilder,
    entry: &DraftEntry,
    bytes: &[u8],
) -> Result<(), String> {
    if let Some(segments) = &entry.segments {
        let expected = segments
            .iter()
            .try_fold(0_usize, |total, segment| total.checked_add(segment.length));
        if expected == Some(bytes.len()) {
            let mut offset = 0_usize;
            for segment in segments {
                let end = offset + segment.length;
                builder
                    .add_bytes(
                        &entry.path,
                        bytes[offset..end].to_vec(),
                        EntryOptions::new()
                            .volume(segment.volume)
                            .raw_flags(segment.raw_flags),
                    )
                    .map_err(|error| error.to_string())?;
                offset = end;
            }
            return Ok(());
        }
    }
    builder
        .add_bytes(
            &entry.path,
            bytes.to_vec(),
            EntryOptions::new()
                .compression(entry.compression)
                .volume(entry.volume),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn match_auxiliary_volumes(
    expected_names: &[dzip::ArchiveString],
    supplied: Vec<NamedBytes>,
) -> Result<Vec<(u16, Vec<u8>)>, String> {
    let mut by_name = HashMap::<String, Vec<u8>>::new();
    for file in supplied {
        let key = normalized_name(&file.name);
        if by_name.insert(key, file.bytes).is_some() {
            return Err(format!("存在重复分卷：{}", file.name));
        }
    }
    let mut result = Vec::new();
    for (index, expected) in expected_names.iter().enumerate() {
        let id = u16::try_from(index + 1).map_err(|_| "分卷编号溢出".to_string())?;
        let expected = expected.to_string_lossy();
        let key = normalized_name(&expected);
        let bytes = if let Some(bytes) = by_name.remove(&key) {
            Some(bytes)
        } else {
            let expected_base = basename(&key);
            let matches = by_name
                .keys()
                .filter(|candidate| basename(candidate) == expected_base)
                .cloned()
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => None,
                [matching] => by_name.remove(matching),
                _ => return Err(format!("分卷名称不明确：{expected}")),
            }
        };
        if let Some(bytes) = bytes {
            result.push((id, bytes));
        }
    }
    if !by_name.is_empty() {
        let mut unexpected = by_name.into_keys().collect::<Vec<_>>();
        unexpected.sort();
        return Err(format!("包含不属于该归档的分卷：{}", unexpected.join(", ")));
    }
    Ok(result)
}

fn normalized_name(name: &str) -> String {
    name.trim().replace('\\', "/").to_ascii_lowercase()
}

fn basename(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

fn portable_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn normalize_archive_name(value: &str) -> String {
    let value = value.trim();
    let name = if value.is_empty() { "archive" } else { value };
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".dz") || lower.ends_with(".dzip") {
        name.to_string()
    } else {
        format!("{name}.dz")
    }
}

fn archive_volume_names(archive_name: &str, count: usize) -> Vec<String> {
    let main = normalize_archive_name(archive_name);
    let lower = main.to_ascii_lowercase();
    let extension_length = if lower.ends_with(".dzip") { 5 } else { 3 };
    let stem = main[..main.len() - extension_length].to_string();
    let mut names = Vec::with_capacity(count);
    names.push(main);
    names.extend((1..count).map(|index| format!("{stem}.{index:03}")));
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_names_and_volume_names_are_stable() {
        assert_eq!(normalize_archive_name("game"), "game.dz");
        assert_eq!(normalize_archive_name("game.DZIP"), "game.DZIP");
        assert_eq!(
            archive_volume_names("game.dz", 3),
            ["game.dz", "game.001", "game.002"]
        );
    }

    #[test]
    fn service_opens_materializes_and_rebuilds_a_real_archive() {
        let bytes =
            include_bytes!("../../../formats/dzip-archive/test_data/native/tiny.dz").to_vec();
        let mut service = ArchiveService::default();
        let summary = service
            .open("tiny.dz".to_string(), bytes, Vec::new())
            .unwrap();
        assert!(!summary.entries.is_empty());
        let entries = summary
            .entries
            .clone()
            .into_iter()
            .map(DraftEntry::from_summary)
            .collect::<Vec<_>>();
        let files = service
            .materialize(MaterializeRequest {
                source_session: Some(summary.session_id),
                entries: entries.clone(),
            })
            .unwrap();
        assert_eq!(files.len(), entries.len());
        let built = service
            .build(BuildRequest {
                source_session: Some(summary.session_id),
                archive_name: "rebuilt.dz".to_string(),
                volume_count: 1,
                alignment: 0,
                range_settings: summary.range_settings,
                use_common_buffer: summary.use_common_buffer,
                entries,
            })
            .unwrap();
        assert_eq!(built.volumes.len(), 1);
        assert_eq!(built.summary.entries.len(), files.len());
    }
}
