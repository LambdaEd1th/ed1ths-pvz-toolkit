mod model;
mod selection;
mod view;

pub(crate) use selection::{FileSelection, file_item_matches_search, file_path_matches_scope};
pub(crate) use view::FileList;

#[cfg(test)]
pub(crate) use model::{build_file_tree, collect_file_tree_keys};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileListItem {
    pub(crate) key: String,
    pub(crate) file_id: Option<usize>,
    pub(crate) tab_id: Option<usize>,
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) detail: String,
    pub(crate) active: bool,
    pub(crate) dirty: bool,
    pub(crate) closeable: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileTreeNode {
    Folder {
        name: String,
        path: String,
        count: usize,
        keys: Vec<String>,
        children: Vec<FileTreeNode>,
    },
    File {
        name: String,
        item: FileListItem,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileListRow {
    Folder {
        key: String,
        name: String,
        path: String,
        depth: usize,
        count: usize,
        selected_count: usize,
        collapsed: bool,
    },
    File {
        key: String,
        name: String,
        item: FileListItem,
        depth: usize,
        selected: bool,
    },
}
