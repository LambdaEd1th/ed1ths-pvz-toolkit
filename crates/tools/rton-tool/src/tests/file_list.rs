use super::*;

#[test]
fn builds_file_tree_from_display_paths() {
    let nodes = build_file_tree(vec![
        file_list_item_for_tree(1, "folder/b.rton"),
        file_list_item_for_tree(2, "folder/sub/a.json"),
        file_list_item_for_tree(3, "root.toml"),
    ]);

    assert_eq!(nodes.len(), 2);
    let FileTreeNode::Folder {
        name,
        count,
        keys,
        children,
        ..
    } = &nodes[0]
    else {
        panic!("first node should be a folder");
    };

    assert_eq!(name, "folder");
    assert_eq!(*count, 2);
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"file:1".to_string()));
    assert!(keys.contains(&"file:2".to_string()));
    assert_eq!(
        collect_file_tree_keys(children),
        vec!["file:2".to_string(), "file:1".to_string()]
    );
    assert!(matches!(
        &children[0],
        FileTreeNode::Folder { name, count: 1, .. } if name == "sub"
    ));
    assert!(matches!(
        &nodes[1],
        FileTreeNode::File { name, item } if name == "root.toml" && item.file_id == Some(3)
    ));
}

#[test]
fn loadable_display_names_match_supported_extensions() {
    assert!(is_loadable_display_name("folder/level.RTON"));
    assert!(is_loadable_display_name("folder/profile.dat"));
    assert!(is_loadable_display_name("config.yaml"));
    assert!(is_loadable_display_name("data.yml"));
    assert!(is_loadable_display_name("package.toml"));
    assert!(is_loadable_display_name("notes.txt"));
    assert!(is_loadable_display_name("README"));
    assert!(!is_loadable_display_name(""));
}

#[test]
fn file_selection_selects_directory_by_path_rule() {
    let items = vec![
        file_list_item_for_tree(1, "folder/a.rton"),
        file_list_item_for_tree(2, "folder/sub/b.json"),
        file_list_item_for_tree(3, "other/c.toml"),
    ];
    let mut selection = FileSelection::default();

    selection.toggle_path("folder".to_string(), true);

    assert_eq!(selection.selected_count(&items), 2);
    assert_eq!(
        selection
            .selected_items(&items)
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["file:1", "file:2"]
    );
}

#[test]
fn file_selection_rules_apply_last_matching_rule() {
    let items = vec![
        file_list_item_for_tree(1, "folder/alpha.rton"),
        file_list_item_for_tree(2, "folder/beta.json"),
        file_list_item_for_tree(3, "other/alpha.toml"),
    ];
    let mut selection = FileSelection::default();

    selection.select_visible("alpha");
    selection.toggle_key("file:1".to_string(), false);
    selection.toggle_key("file:2".to_string(), true);

    assert_eq!(
        selection
            .selected_items(&items)
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["file:2", "file:3"]
    );
}
