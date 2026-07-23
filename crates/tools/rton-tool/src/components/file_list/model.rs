use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::domain::leaf_display_name;

#[cfg(test)]
use super::FileTreeNode;
use super::{FileListItem, FileListRow, FileSelection};

#[cfg(test)]
pub(crate) fn build_file_tree(items: Vec<FileListItem>) -> Vec<FileTreeNode> {
    let mut builder = TreeBuilderFolder::root();
    for item in items {
        let mut parts = split_display_path(&item.path);
        if parts.is_empty() {
            parts.push(item.name.clone());
        }
        builder.insert(&parts, item, false);
    }
    builder.into_nodes()
}

pub(crate) fn build_file_list_rows(
    items: &[FileListItem],
    selection: &FileSelection,
    collapsed_paths: &HashSet<String>,
) -> Vec<FileListRow> {
    let mut builder = TreeBuilderFolder::root();
    for item in items {
        let mut parts = split_display_path(&item.path);
        if parts.is_empty() {
            parts.push(item.name.clone());
        }
        builder.insert(&parts, item.clone(), selection.is_selected(item));
    }
    builder.into_rows(collapsed_paths)
}

#[derive(Debug, Default)]
struct TreeBuilderFolder {
    name: String,
    path: String,
    count: usize,
    selected_count: usize,
    keys: Vec<String>,
    folders: BTreeMap<String, TreeBuilderFolder>,
    files: Vec<(String, FileListItem)>,
    selected_files: Vec<String>,
}

impl TreeBuilderFolder {
    fn root() -> Self {
        Self::default()
    }

    fn insert(&mut self, parts: &[String], item: FileListItem, selected: bool) {
        if parts.len() <= 1 {
            let name = parts
                .first()
                .cloned()
                .unwrap_or_else(|| leaf_display_name(&item.path));
            if selected {
                self.selected_files.push(item.key.clone());
            }
            self.files.push((name, item));
            return;
        }

        let folder_name = parts[0].clone();
        let folder_path = if self.path.is_empty() {
            folder_name.clone()
        } else {
            format!("{}/{}", self.path, folder_name)
        };
        let descendant_key = item.key.clone();
        let folder = self
            .folders
            .entry(folder_path.clone())
            .or_insert_with(|| TreeBuilderFolder {
                name: folder_name,
                path: folder_path,
                ..TreeBuilderFolder::default()
            });
        folder.count += 1;
        if selected {
            folder.selected_count += 1;
        }
        folder.keys.push(descendant_key);
        folder.insert(&parts[1..], item, selected);
    }

    fn into_rows(self, collapsed_paths: &HashSet<String>) -> Vec<FileListRow> {
        let mut rows = Vec::with_capacity(self.count + self.files.len() + self.folders.len());
        append_rows_from_parts(
            self.folders,
            self.files,
            self.selected_files,
            0,
            collapsed_paths,
            &mut rows,
        );
        rows
    }

    #[cfg(test)]
    fn into_nodes(self) -> Vec<FileTreeNode> {
        tree_nodes_from_parts(self.folders, self.files)
    }

    #[cfg(test)]
    fn into_tree_node(self) -> FileTreeNode {
        let TreeBuilderFolder {
            name,
            path,
            count,
            selected_count: _,
            keys,
            folders,
            files,
            selected_files: _,
        } = self;

        FileTreeNode::Folder {
            name,
            path,
            count,
            keys,
            children: tree_nodes_from_parts(folders, files),
        }
    }
}

fn append_rows_from_parts(
    folders: BTreeMap<String, TreeBuilderFolder>,
    files: Vec<(String, FileListItem)>,
    selected_files: Vec<String>,
    depth: usize,
    collapsed_paths: &HashSet<String>,
    rows: &mut Vec<FileListRow>,
) {
    let mut nodes = Vec::with_capacity(folders.len() + files.len());
    nodes.extend(folders.into_values().map(TreeRowPart::Folder));
    nodes.extend(
        files
            .into_iter()
            .map(|(name, item)| TreeRowPart::File { name, item }),
    );
    sort_tree_row_parts(&mut nodes);
    let selected_lookup = selected_files.into_iter().collect::<HashSet<_>>();

    for node in nodes {
        match node {
            TreeRowPart::Folder(folder) => {
                let collapsed = collapsed_paths.contains(&folder.path);
                let key = file_tree_node_key_for_folder(&folder.path);
                let TreeBuilderFolder {
                    name,
                    path,
                    count,
                    selected_count,
                    keys: _,
                    folders,
                    files,
                    selected_files,
                } = folder;
                rows.push(FileListRow::Folder {
                    key,
                    name,
                    path: path.clone(),
                    depth,
                    count,
                    selected_count,
                    collapsed,
                });
                if !collapsed {
                    append_rows_from_parts(
                        folders,
                        files,
                        selected_files,
                        depth + 1,
                        collapsed_paths,
                        rows,
                    );
                }
            }
            TreeRowPart::File { name, item } => {
                let selected = selected_lookup.contains(&item.key);
                rows.push(FileListRow::File {
                    key: item.key.clone(),
                    name,
                    item,
                    depth,
                    selected,
                });
            }
        }
    }
}

#[derive(Debug)]
enum TreeRowPart {
    Folder(TreeBuilderFolder),
    File { name: String, item: FileListItem },
}

fn sort_tree_row_parts(nodes: &mut [TreeRowPart]) {
    nodes.sort_by(|left, right| match (left, right) {
        (TreeRowPart::Folder(left), TreeRowPart::Folder(right)) => left
            .name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase()),
        (TreeRowPart::File { name: left, .. }, TreeRowPart::File { name: right, .. }) => {
            left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
        }
        (TreeRowPart::Folder(_), TreeRowPart::File { .. }) => std::cmp::Ordering::Less,
        (TreeRowPart::File { .. }, TreeRowPart::Folder(_)) => std::cmp::Ordering::Greater,
    });
}

#[cfg(test)]
fn tree_nodes_from_parts(
    folders: BTreeMap<String, TreeBuilderFolder>,
    files: Vec<(String, FileListItem)>,
) -> Vec<FileTreeNode> {
    let mut nodes = Vec::with_capacity(folders.len() + files.len());
    nodes.extend(folders.into_values().map(TreeBuilderFolder::into_tree_node));
    nodes.extend(
        files
            .into_iter()
            .map(|(name, item)| FileTreeNode::File { name, item }),
    );
    sort_file_tree_node_level(&mut nodes);
    nodes
}

#[cfg(test)]
fn sort_file_tree_node_level(nodes: &mut [FileTreeNode]) {
    nodes.sort_by(|left, right| match (left, right) {
        (FileTreeNode::Folder { name: left, .. }, FileTreeNode::Folder { name: right, .. })
        | (FileTreeNode::File { name: left, .. }, FileTreeNode::File { name: right, .. }) => {
            left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
        }
        (FileTreeNode::Folder { .. }, FileTreeNode::File { .. }) => std::cmp::Ordering::Less,
        (FileTreeNode::File { .. }, FileTreeNode::Folder { .. }) => std::cmp::Ordering::Greater,
    });
}

#[cfg(test)]
pub(crate) fn collect_file_tree_keys(nodes: &[FileTreeNode]) -> Vec<String> {
    let mut keys = Vec::new();
    for node in nodes {
        match node {
            FileTreeNode::Folder {
                keys: folder_keys, ..
            } => keys.extend(folder_keys.iter().cloned()),
            FileTreeNode::File { item, .. } => keys.push(item.key.clone()),
        }
    }
    keys
}

pub(super) fn file_list_row_key(row: &FileListRow) -> &str {
    match row {
        FileListRow::Folder { key, .. } | FileListRow::File { key, .. } => key,
    }
}

fn file_tree_node_key_for_folder(path: &str) -> String {
    format!("folder:{path}")
}

fn split_display_path(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}
