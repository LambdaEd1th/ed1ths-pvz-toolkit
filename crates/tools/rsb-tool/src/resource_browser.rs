//! A virtual directory tree over logical manifest paths, independent of RSG storage.

use crate::resources::{MappingState, ResourceCatalog, normalize_path};
use crate::view_model::{BrowserSort, BrowserSortKey};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

const PROGRAM_FOLDER: &str = "\0program";
const UNPATHED_FOLDER: &str = "\0unpathed";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Directory {
    pub name: String,
    pub children: BTreeSet<String>,
    pub resources: usize,
    /// Total unpacked size of unique physical files below this directory.
    pub size: Option<u64>,
    name_from_manifest: bool,
}

impl Directory {
    fn consider_name(&mut self, name: &str, from_manifest: bool) {
        // Keep an actual source spelling, separate from the case-insensitive key.
        // Manifest paths outrank physical-index fallbacks. If manifests disagree
        // only on case, prefer their lowercase spelling, then mixed case.
        let priority = |name: &str, manifest: bool| {
            (
                manifest,
                name == name.to_ascii_lowercase(),
                name != name.to_ascii_uppercase(),
            )
        };
        let next = priority(name, from_manifest);
        let current = priority(&self.name, self.name_from_manifest);
        if self.name.is_empty() || next > current || (next == current && name < self.name.as_str())
        {
            self.name = name.into();
            self.name_from_manifest = from_manifest;
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourcePath {
    pub folder: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirectoryIndex {
    pub directories: BTreeMap<String, Directory>,
    pub paths: Vec<ResourcePath>,
    sorted_resources: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrowserTarget {
    Folder(String),
    Resource(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserItem {
    pub target: BrowserTarget,
    pub count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrowserSelection {
    pub targets: BTreeSet<BrowserTarget>,
    pub focused: Option<BrowserTarget>,
    anchor: Option<usize>,
}

impl BrowserSelection {
    pub fn select(&mut self, items: &[BrowserItem], index: usize, toggle: bool, range: bool) {
        let Some(item) = items.get(index) else {
            return;
        };
        self.focused = Some(item.target.clone());
        if range {
            let start = self
                .anchor
                .unwrap_or(index)
                .min(items.len().saturating_sub(1));
            if !toggle {
                self.targets.clear();
            }
            self.targets.extend(
                items[start.min(index)..=start.max(index)]
                    .iter()
                    .map(|item| item.target.clone()),
            );
        } else {
            if toggle {
                if !self.targets.remove(&item.target) {
                    self.targets.insert(item.target.clone());
                }
            } else {
                self.targets = BTreeSet::from([item.target.clone()]);
            }
            self.anchor = Some(index);
        }
    }

    pub fn all(items: &[BrowserItem]) -> Self {
        Self {
            targets: items.iter().map(|item| item.target.clone()).collect(),
            focused: items.first().map(|item| item.target.clone()),
            anchor: Some(0),
        }
    }

    pub fn resource_indices(&self, tree: &DirectoryIndex) -> Vec<usize> {
        let mut indices = BTreeSet::new();
        let folders = self
            .targets
            .iter()
            .filter_map(|target| match target {
                BrowserTarget::Resource(index) => {
                    indices.insert(*index);
                    None
                }
                BrowserTarget::Folder(path) => Some(path),
            })
            .collect::<Vec<_>>();
        if !folders.is_empty() {
            for (index, path) in tree.paths.iter().enumerate() {
                if folders
                    .iter()
                    .any(|folder| is_descendant(&path.folder, folder))
                {
                    indices.insert(index);
                }
            }
        }
        indices.into_iter().collect()
    }
}

#[derive(Default)]
pub struct BrowserFilter<'a> {
    pub query: &'a str,
    pub group: &'a str,
    pub kind: &'a str,
    pub state: &'a str,
    pub sort: BrowserSort,
}

pub fn parent_path(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

pub fn is_descendant(folder: &str, ancestor: &str) -> bool {
    ancestor.is_empty()
        || folder == ancestor
        || folder
            .strip_prefix(ancestor)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

impl DirectoryIndex {
    pub fn build(catalog: &ResourceCatalog) -> Self {
        let mut output = Self::default();
        output.directories.insert(
            String::new(),
            Directory {
                name: "逻辑资源".into(),
                ..Default::default()
            },
        );
        for row in &catalog.rows {
            let original_parts = row
                .entry
                .path
                .split(['/', '\\'])
                .filter(|part| !part.is_empty() && *part != ".")
                .collect::<Vec<_>>();
            let normalized = normalize_path(&row.entry.path);
            let (folder, name) = if row.state == MappingState::Program {
                (PROGRAM_FOLDER.to_string(), row.entry.id.clone())
            } else if normalized.is_empty() {
                (UNPATHED_FOLDER.to_string(), row.entry.id.clone())
            } else {
                (
                    parent_path(&normalized).to_string(),
                    original_parts
                        .last()
                        .copied()
                        .unwrap_or(&row.entry.id)
                        .to_string(),
                )
            };
            let folder_size = if matches!(row.state, MappingState::File | MappingState::Unlisted) {
                row.size
            } else {
                None
            };
            let mut path = String::new();
            for (depth, part) in folder
                .split('/')
                .filter(|part| !part.is_empty())
                .enumerate()
            {
                let parent = path.clone();
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(part);
                output
                    .directories
                    .get_mut(&parent)
                    .unwrap()
                    .children
                    .insert(path.clone());
                let name = match part {
                    PROGRAM_FOLDER => "程序资源",
                    UNPATHED_FOLDER => "无路径资源",
                    _ => original_parts[depth],
                };
                let directory = output.directories.entry(path.clone()).or_default();
                directory.consider_name(name, row.state != MappingState::Unlisted);
                directory.resources += 1;
                if let Some(size) = folder_size {
                    directory.size = Some(directory.size.unwrap_or(0).saturating_add(size));
                }
            }
            let root = output.directories.get_mut("").unwrap();
            root.resources += 1;
            if let Some(size) = folder_size {
                root.size = Some(root.size.unwrap_or(0).saturating_add(size));
            }
            output.paths.push(ResourcePath { folder, name });
        }
        output.sorted_resources = (0..output.paths.len()).collect();
        let names = output
            .paths
            .iter()
            .map(|path| path.name.to_lowercase())
            .collect::<Vec<_>>();
        output
            .sorted_resources
            .sort_by(|&a, &b| names[a].cmp(&names[b]).then(a.cmp(&b)));
        output
    }

    pub fn breadcrumbs(&self, path: &str) -> Vec<String> {
        let mut output = vec![String::new()];
        let mut next = String::new();
        for part in path.split('/').filter(|part| !part.is_empty()) {
            if !next.is_empty() {
                next.push('/');
            }
            next.push_str(part);
            if self.directories.contains_key(&next) {
                output.push(next.clone());
            }
        }
        output
    }

    pub fn visible_tree(&self, expanded: &BTreeSet<String>) -> Vec<(String, usize)> {
        let mut output = Vec::new();
        let mut stack = vec![(String::new(), 0)];
        while let Some((path, depth)) = stack.pop() {
            let Some(directory) = self.directories.get(&path) else {
                continue;
            };
            if expanded.contains(&path) {
                stack.extend(
                    directory
                        .children
                        .iter()
                        .rev()
                        .map(|child| (child.clone(), depth + 1)),
                );
            }
            output.push((path, depth));
        }
        output
    }

    /// Filters keep folders navigable. Text searches flatten the current subtree.
    pub fn items(
        &self,
        catalog: &ResourceCatalog,
        current: &str,
        filter: BrowserFilter<'_>,
    ) -> Vec<BrowserItem> {
        let query = filter.query.trim().to_lowercase();
        let mut folders = BTreeMap::<String, usize>::new();
        let mut files = Vec::new();
        for &index in &self.sorted_resources {
            let row = &catalog.rows[index];
            let path = &self.paths[index];
            if !is_descendant(&path.folder, current)
                || (!filter.group.is_empty() && filter.group != row.entry.group)
                || (!filter.kind.is_empty() && filter.kind != row.entry.kind)
                || (!query.is_empty() && !row.search.contains(&query))
                || !match filter.state {
                    "mapped" => matches!(row.state, MappingState::File | MappingState::AtlasChild),
                    "missing" => row.state == MappingState::Missing,
                    "ambiguous" => row.state == MappingState::Ambiguous,
                    "unlisted" => row.state == MappingState::Unlisted,
                    "program" => row.state == MappingState::Program,
                    _ => true,
                }
            {
                continue;
            }
            if !query.is_empty() || path.folder == current {
                files.push(BrowserItem {
                    target: BrowserTarget::Resource(index),
                    count: 1,
                });
            } else {
                let relative = if current.is_empty() {
                    &path.folder
                } else {
                    &path.folder[current.len() + 1..]
                };
                let child = relative.split('/').next().unwrap_or_default();
                let child = if current.is_empty() {
                    child.into()
                } else {
                    format!("{current}/{child}")
                };
                *folders.entry(child).or_default() += 1;
            }
        }
        let mut output = folders
            .into_iter()
            .map(|(path, count)| BrowserItem {
                target: BrowserTarget::Folder(path),
                count,
            })
            .collect::<Vec<_>>();
        output.sort_by(|a, b| {
            let BrowserTarget::Folder(a) = &a.target else {
                unreachable!()
            };
            let BrowserTarget::Folder(b) = &b.target else {
                unreachable!()
            };
            let ordering = match filter.sort.key {
                BrowserSortKey::Name => filter.sort.apply(compare_text(
                    &self.directories[a].name,
                    &self.directories[b].name,
                )),
                BrowserSortKey::Size => compare_size(
                    self.directories[a].size,
                    self.directories[b].size,
                    filter.sort,
                )
                .then_with(|| compare_text(&self.directories[a].name, &self.directories[b].name)),
                BrowserSortKey::Type => {
                    compare_text(&self.directories[a].name, &self.directories[b].name)
                }
            };
            ordering.then_with(|| a.cmp(b))
        });
        files.sort_by(|left, right| {
            let (BrowserTarget::Resource(left), BrowserTarget::Resource(right)) =
                (&left.target, &right.target)
            else {
                return Ordering::Equal;
            };
            let left_name = &self.paths[*left].name;
            let right_name = &self.paths[*right].name;
            let ordering = match filter.sort.key {
                BrowserSortKey::Name => filter.sort.apply(compare_text(left_name, right_name)),
                BrowserSortKey::Type => filter
                    .sort
                    .apply(
                        compare_text(
                            &catalog.rows[*left].entry.kind,
                            &catalog.rows[*right].entry.kind,
                        )
                        .then_with(|| {
                            compare_text(
                                &catalog.rows[*left].entry.subgroup,
                                &catalog.rows[*right].entry.subgroup,
                            )
                        }),
                    )
                    .then_with(|| compare_text(left_name, right_name)),
                BrowserSortKey::Size => compare_size(
                    catalog.rows[*left].size,
                    catalog.rows[*right].size,
                    filter.sort,
                )
                .then_with(|| compare_text(left_name, right_name)),
            };
            ordering
                .then_with(|| left_name.cmp(right_name))
                .then_with(|| left.cmp(right))
        });
        output.extend(files);
        output
    }
}

fn compare_text(left: &str, right: &str) -> Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

fn compare_size(left: Option<u64>, right: Option<u64>, sort: BrowserSort) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => sort.apply(left.cmp(&right)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavigationHistory {
    entries: Vec<String>,
    position: usize,
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self {
            entries: vec![String::new()],
            position: 0,
        }
    }
}

impl NavigationHistory {
    pub fn current(&self) -> &str {
        &self.entries[self.position]
    }
    pub fn can_back(&self) -> bool {
        self.position > 0
    }
    pub fn can_forward(&self) -> bool {
        self.position + 1 < self.entries.len()
    }
    pub fn navigate(&mut self, path: String) {
        if path == self.current() {
            return;
        }
        self.entries.truncate(self.position + 1);
        self.entries.push(path);
        self.position += 1;
    }
    pub fn back(&mut self) {
        self.position = self.position.saturating_sub(1);
    }
    pub fn forward(&mut self) {
        if self.can_forward() {
            self.position += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemWindow {
    pub columns: usize,
    pub height: usize,
    pub content_height: usize,
    pub start: usize,
    pub end: usize,
}

pub fn item_window(
    count: usize,
    scroll: f64,
    viewport: usize,
    width: usize,
    grid: bool,
) -> ItemWindow {
    let columns = if grid { (width / 150).clamp(1, 24) } else { 1 };
    let height = if grid { 116 } else { 50 };
    let rows = count.div_ceil(columns);
    let first = ((scroll.max(0.0) as usize) / height).min(rows);
    let start = first.saturating_sub(4) * columns;
    let end = ((first + viewport.div_ceil(height).min(192) + 5) * columns).min(count);
    ItemWindow {
        columns,
        height,
        content_height: rows * height,
        start,
        end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        FileSizeMap, ManifestData, ResourceEntry, ResourceLocation, build_catalog,
        build_catalog_with_sizes, normalize_path,
    };
    use crate::view_model::BrowserSortDirection;

    fn catalog() -> ResourceCatalog {
        build_catalog(
            &ManifestData {
                resources: vec![
                    ResourceEntry {
                        id: "A".into(),
                        path: "images\\plants\\Pea.png".into(),
                        kind: "Image".into(),
                        group: "Plant".into(),
                        ..Default::default()
                    },
                    ResourceEntry {
                        id: "B".into(),
                        path: "Images/Plants/Pea.png".into(),
                        kind: "Image".into(),
                        subgroup: "Variant".into(),
                        ..Default::default()
                    },
                    ResourceEntry {
                        id: "C".into(),
                        path: "images/plants2/Leaf".into(),
                        kind: "Image".into(),
                        ..Default::default()
                    },
                    ResourceEntry {
                        id: "D".into(),
                        path: "effects/fire".into(),
                        kind: "RenderEffect".into(),
                        ..Default::default()
                    },
                    ResourceEntry {
                        id: "E".into(),
                        path: "!program".into(),
                        ..Default::default()
                    },
                    ResourceEntry {
                        id: "F".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            &[],
        )
    }

    #[test]
    fn selection_supports_toggle_ranges_and_deduplicated_recursive_folders() {
        let catalog = catalog();
        let tree = DirectoryIndex::build(&catalog);
        let items = tree.items(&catalog, "IMAGES/PLANTS", BrowserFilter::default());
        let mut selection = BrowserSelection::default();
        selection.select(&items, 0, false, false);
        assert_eq!(selection.targets.len(), 1);
        selection.select(&items, 1, true, false);
        assert_eq!(selection.targets.len(), 2);
        selection.select(&items, 0, true, false);
        assert_eq!(selection.targets.len(), 1);
        selection.select(&items, 1, false, true);
        assert_eq!(selection.targets.len(), 2);
        selection
            .targets
            .insert(BrowserTarget::Folder("IMAGES".into()));
        selection
            .targets
            .insert(BrowserTarget::Folder("IMAGES/PLANTS".into()));
        let indices = selection.resource_indices(&tree);
        assert_eq!(indices.len(), 3);
        assert!(indices.iter().all(|&index| {
            catalog.rows[index]
                .entry
                .path
                .to_ascii_lowercase()
                .starts_with("image")
        }));
        selection.targets = BTreeSet::from([BrowserTarget::Folder("IMAGES/PLANTS".into())]);
        assert_eq!(
            selection.resource_indices(&tree).len(),
            2,
            "plants2 is not a child of plants"
        );
    }

    #[test]
    fn folders_merge_case_and_separators_but_preserve_resource_variants() {
        let catalog = catalog();
        let tree = DirectoryIndex::build(&catalog);
        assert_eq!(tree.directories["IMAGES/PLANTS"].resources, 2);
        assert_eq!(tree.directories[""].resources, 6);
        let rows = tree.items(&catalog, "IMAGES/PLANTS", BrowserFilter::default());
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|row| matches!(row.target, BrowserTarget::Resource(_)))
        );
        let index = catalog
            .rows
            .iter()
            .position(|row| row.entry.id == "A")
            .unwrap();
        assert_eq!(tree.paths[index].name, "Pea.png");
    }

    #[test]
    fn root_shows_folders_and_virtual_resources_stay_separate() {
        let catalog = catalog();
        let tree = DirectoryIndex::build(&catalog);
        let rows = tree.items(&catalog, "", BrowserFilter::default());
        assert_eq!(rows.len(), 4);
        assert!(
            rows.iter()
                .all(|row| matches!(row.target, BrowserTarget::Folder(_)))
        );
        assert_eq!(tree.directories[PROGRAM_FOLDER].resources, 1);
        assert_eq!(tree.directories[UNPATHED_FOLDER].resources, 1);
    }

    #[test]
    fn directory_labels_follow_manifest_case_without_changing_navigation_keys() {
        let catalog = catalog();
        let tree = DirectoryIndex::build(&catalog);
        assert_eq!(tree.directories["IMAGES"].name, "images");
        assert_eq!(tree.directories["IMAGES/PLANTS"].name, "plants");
        assert_eq!(tree.directories["EFFECTS"].name, "effects");
        let labels = tree
            .breadcrumbs("IMAGES/PLANTS")
            .iter()
            .map(|path| tree.directories[path].name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["逻辑资源", "images", "plants"]);
        assert_eq!(
            tree.items(&catalog, "IMAGES/PLANTS", BrowserFilter::default())
                .len(),
            2
        );
        assert_eq!(
            catalog
                .rows
                .iter()
                .find(|row| row.entry.id == "A")
                .unwrap()
                .entry
                .path,
            "images\\plants\\Pea.png"
        );
    }

    #[test]
    fn resource_items_sort_by_name_and_size_in_both_directions() {
        let files = vec![
            ResourceLocation {
                packet_index: 0,
                packet_name: "Packet".into(),
                path: "a-large.bin".into(),
            },
            ResourceLocation {
                packet_index: 0,
                packet_name: "Packet".into(),
                path: "b-small.bin".into(),
            },
        ];
        let sizes = FileSizeMap::from([
            ((0, normalize_path("a-large.bin")), 100_u64),
            ((0, normalize_path("b-small.bin")), 10_u64),
        ]);
        let catalog = build_catalog_with_sizes(&ManifestData::default(), &files, &sizes);
        assert_eq!(
            catalog
                .rows
                .iter()
                .find(|row| row.entry.path == "a-large.bin")
                .and_then(|row| row.size),
            Some(100)
        );
        let tree = DirectoryIndex::build(&catalog);
        let names = |sort: BrowserSort| {
            tree.items(
                &catalog,
                "",
                BrowserFilter {
                    sort,
                    ..Default::default()
                },
            )
            .into_iter()
            .map(|item| match item.target {
                BrowserTarget::Resource(index) => catalog.rows[index].entry.path.clone(),
                BrowserTarget::Folder(path) => panic!("unexpected folder {path}"),
            })
            .collect::<Vec<_>>()
        };
        let sort = |key, direction| BrowserSort { key, direction };
        assert_eq!(
            names(sort(BrowserSortKey::Name, BrowserSortDirection::Ascending)),
            ["a-large.bin", "b-small.bin"]
        );
        assert_eq!(
            names(sort(BrowserSortKey::Name, BrowserSortDirection::Descending)),
            ["b-small.bin", "a-large.bin"]
        );
        assert_eq!(
            names(sort(BrowserSortKey::Size, BrowserSortDirection::Ascending)),
            ["b-small.bin", "a-large.bin"]
        );
        assert_eq!(
            names(sort(BrowserSortKey::Size, BrowserSortDirection::Descending)),
            ["a-large.bin", "b-small.bin"]
        );
    }

    #[test]
    fn manifest_spelling_outranks_physical_paths_and_preserves_mixed_case() {
        let mut catalog = build_catalog(
            &ManifestData {
                resources: vec![ResourceEntry {
                    id: "PEA".into(),
                    path: "images/MySprites/Pea.png".into(),
                    kind: "Image".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            &[
                crate::resources::ResourceLocation {
                    packet_index: 0,
                    packet_name: "Atlas".into(),
                    path: "IMAGES/MYSPRITES/PEA.PNG".into(),
                },
                crate::resources::ResourceLocation {
                    packet_index: 0,
                    packet_name: "Atlas".into(),
                    path: "IMAGES/MYSPRITES/UNLISTED.DAT".into(),
                },
            ],
        );
        // Exercise physical fallbacks encountered before the manifest resource.
        catalog
            .rows
            .sort_by_key(|row| row.state != MappingState::Unlisted);
        let tree = DirectoryIndex::build(&catalog);
        assert_eq!(tree.directories["IMAGES"].name, "images");
        assert_eq!(tree.directories["IMAGES/MYSPRITES"].name, "MySprites");
        assert_eq!(tree.directories["IMAGES/MYSPRITES"].resources, 2);
        let mapped = catalog
            .rows
            .iter()
            .find(|row| row.entry.id == "PEA")
            .unwrap();
        assert_eq!(mapped.state, MappingState::File);
        assert_eq!(mapped.locations[0].path, "IMAGES/MYSPRITES/PEA.PNG");
        let unlisted_index = catalog
            .rows
            .iter()
            .position(|row| row.state == MappingState::Unlisted)
            .unwrap();
        assert_eq!(tree.paths[unlisted_index].name, "UNLISTED.DAT");
    }

    #[test]
    fn search_is_recursive_and_respects_directory_boundaries() {
        let catalog = catalog();
        let tree = DirectoryIndex::build(&catalog);
        assert_eq!(
            tree.items(
                &catalog,
                "IMAGES",
                BrowserFilter {
                    query: "pea",
                    ..Default::default()
                }
            )
            .len(),
            2
        );
        assert!(
            tree.items(
                &catalog,
                "IMAGES/PLANTS",
                BrowserFilter {
                    query: "leaf",
                    ..Default::default()
                }
            )
            .is_empty()
        );
        assert!(!is_descendant("IMAGES/PLANTS2", "IMAGES/PLANTS"));
    }

    #[test]
    fn filters_keep_only_matching_directories_and_counts() {
        let catalog = catalog();
        let tree = DirectoryIndex::build(&catalog);
        let rows = tree.items(
            &catalog,
            "",
            BrowserFilter {
                group: "Plant",
                kind: "Image",
                state: "missing",
                ..Default::default()
            },
        );
        assert_eq!(
            rows,
            vec![BrowserItem {
                target: BrowserTarget::Folder("IMAGES".into()),
                count: 1
            }]
        );
    }

    #[test]
    fn expansion_and_breadcrumbs_are_path_based() {
        let tree = DirectoryIndex::build(&catalog());
        assert_eq!(tree.visible_tree(&BTreeSet::new()).len(), 1);
        let expanded = BTreeSet::from([String::new(), "IMAGES".into()]);
        assert!(
            tree.visible_tree(&expanded)
                .contains(&("IMAGES/PLANTS".into(), 2))
        );
        assert_eq!(
            tree.breadcrumbs("IMAGES/PLANTS"),
            vec!["", "IMAGES", "IMAGES/PLANTS"]
        );
    }

    #[test]
    fn navigating_after_back_discards_forward_history() {
        let mut history = NavigationHistory::default();
        history.navigate("IMAGES".into());
        history.navigate("IMAGES/PLANTS".into());
        history.back();
        assert!(history.can_forward());
        history.forward();
        assert_eq!(history.current(), "IMAGES/PLANTS");
        history.back();
        history.navigate("EFFECTS".into());
        assert!(!history.can_forward());
        history.navigate("EFFECTS".into());
        history.back();
        assert_eq!(history.current(), "IMAGES");
    }

    #[test]
    fn large_icon_and_detail_views_are_virtualized() {
        for grid in [false, true] {
            let window = item_window(250_000, 123_456.0, 700, 1000, grid);
            assert!(window.start > 0);
            assert!(window.end - window.start < 120);
            assert_eq!(window.start % window.columns, 0);
            let tail = item_window(17, 1_000_000.0, 700, 1000, grid);
            assert_eq!(tail.end, 17);
        }
    }
}
