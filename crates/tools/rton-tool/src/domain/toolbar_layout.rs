use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DropPlacement {
    Before,
    After,
}

impl DropPlacement {
    pub(crate) fn class(self) -> &'static str {
        match self {
            DropPlacement::Before => "drop-before",
            DropPlacement::After => "drop-after",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DropMarker<T> {
    pub(crate) id: T,
    pub(crate) placement: DropPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ToolbarGroupId {
    File,
    Edit,
    TextExport,
    RtonExport,
    Preferences,
}

impl ToolbarGroupId {
    pub(crate) fn code(self) -> &'static str {
        match self {
            ToolbarGroupId::File => "file",
            ToolbarGroupId::Edit => "edit",
            ToolbarGroupId::TextExport => "textExport",
            ToolbarGroupId::RtonExport => "rtonExport",
            ToolbarGroupId::Preferences => "prefs",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        match code {
            "file" => Some(ToolbarGroupId::File),
            "edit" => Some(ToolbarGroupId::Edit),
            "textExport" => Some(ToolbarGroupId::TextExport),
            "rtonExport" => Some(ToolbarGroupId::RtonExport),
            "prefs" => Some(ToolbarGroupId::Preferences),
            _ => None,
        }
    }

    pub(crate) fn label_key(self) -> &'static str {
        match self {
            ToolbarGroupId::File => "toolbar-group-file",
            ToolbarGroupId::Edit => "toolbar-group-edit",
            ToolbarGroupId::TextExport => "toolbar-group-text-export",
            ToolbarGroupId::RtonExport => "toolbar-group-rton-export",
            ToolbarGroupId::Preferences => "toolbar-group-preferences",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarDropTarget {
    Group {
        id: ToolbarGroupId,
        placement: DropPlacement,
    },
    RowEnd {
        row_index: usize,
    },
}

pub(crate) fn default_toolbar_rows() -> Vec<Vec<ToolbarGroupId>> {
    vec![
        vec![ToolbarGroupId::File, ToolbarGroupId::Edit],
        vec![
            ToolbarGroupId::TextExport,
            ToolbarGroupId::RtonExport,
            ToolbarGroupId::Preferences,
        ],
    ]
}

fn all_toolbar_group_ids() -> [ToolbarGroupId; 5] {
    [
        ToolbarGroupId::File,
        ToolbarGroupId::Edit,
        ToolbarGroupId::TextExport,
        ToolbarGroupId::RtonExport,
        ToolbarGroupId::Preferences,
    ]
}

pub(crate) fn normalize_toolbar_rows(source: Option<&str>) -> Vec<Vec<ToolbarGroupId>> {
    let mut rows = vec![Vec::new(), Vec::new()];
    let mut seen = HashSet::<ToolbarGroupId>::new();

    if let Some(source_rows) = source
        .and_then(|source| serde_json::from_str::<serde_json::Value>(source).ok())
        .and_then(|value| value.as_array().cloned())
    {
        for (row_index, row) in source_rows.iter().enumerate() {
            let Some(items) = row.as_array() else {
                continue;
            };
            let target_row = row_index.min(rows.len() - 1);
            for item in items {
                let Some(id) = item.as_str().and_then(ToolbarGroupId::from_code) else {
                    continue;
                };
                if seen.insert(id) {
                    rows[target_row].push(id);
                }
            }
        }
    } else {
        return default_toolbar_rows();
    }

    for id in all_toolbar_group_ids() {
        if seen.insert(id) {
            let last_row_index = rows.len() - 1;
            rows[last_row_index].push(id);
        }
    }

    rows
}

pub(crate) fn toolbar_rows_to_json(rows: &[Vec<ToolbarGroupId>]) -> String {
    let rows = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|id| serde_json::Value::String(id.code().to_string()))
                .collect::<Vec<_>>()
        })
        .map(serde_json::Value::Array)
        .collect::<Vec<_>>();
    serde_json::Value::Array(rows).to_string()
}

pub(crate) fn apply_toolbar_drop_target(
    rows: &mut [Vec<ToolbarGroupId>],
    dragged_id: ToolbarGroupId,
    target: ToolbarDropTarget,
) -> Option<DropMarker<ToolbarGroupId>> {
    match target {
        ToolbarDropTarget::Group { id, placement } => {
            if dragged_id == id {
                return None;
            }
            move_toolbar_group(rows, dragged_id, id, placement);
            Some(DropMarker { id, placement })
        }
        ToolbarDropTarget::RowEnd { row_index } => {
            move_toolbar_group_to_row_end(rows, dragged_id, row_index);
            None
        }
    }
}

pub(crate) fn move_toolbar_group(
    rows: &mut [Vec<ToolbarGroupId>],
    group_id: ToolbarGroupId,
    target_id: ToolbarGroupId,
    placement: DropPlacement,
) {
    if group_id == target_id {
        return;
    }
    let Some((source_row_index, source_index)) = find_toolbar_group(rows, group_id) else {
        return;
    };
    let Some((target_row_index, target_index)) = find_toolbar_group(rows, target_id) else {
        return;
    };
    if source_row_index == target_row_index
        && ((placement == DropPlacement::Before && source_index + 1 == target_index)
            || (placement == DropPlacement::After && source_index == target_index + 1))
    {
        return;
    }

    let group = rows[source_row_index].remove(source_index);
    let Some((target_row_index, target_index)) = find_toolbar_group(rows, target_id) else {
        let fallback_row_index = source_row_index.min(rows.len() - 1);
        rows[fallback_row_index].push(group);
        return;
    };
    let insert_index = match placement {
        DropPlacement::Before => target_index,
        DropPlacement::After => target_index + 1,
    };
    let target_len = rows[target_row_index].len();
    rows[target_row_index].insert(insert_index.min(target_len), group);
}

pub(crate) fn move_toolbar_group_to_row_end(
    rows: &mut [Vec<ToolbarGroupId>],
    group_id: ToolbarGroupId,
    row_index: usize,
) {
    if row_index >= rows.len() {
        return;
    }
    let Some((source_row_index, source_index)) = find_toolbar_group(rows, group_id) else {
        return;
    };
    if source_row_index == row_index && rows[row_index].last() == Some(&group_id) {
        return;
    }
    let group = rows[source_row_index].remove(source_index);
    rows[row_index].push(group);
}

fn find_toolbar_group(
    rows: &[Vec<ToolbarGroupId>],
    group_id: ToolbarGroupId,
) -> Option<(usize, usize)> {
    rows.iter().enumerate().find_map(|(row_index, row)| {
        row.iter()
            .position(|id| *id == group_id)
            .map(|index| (row_index, index))
    })
}
