use super::*;

#[test]
fn reorders_tab_after_target() {
    let mut tabs = vec![tab(1), tab(2), tab(3)];

    reorder_tabs_by_id(&mut tabs, 1, 2, DropPlacement::After);

    assert_eq!(tab_ids(&tabs), vec![2, 1, 3]);
}

#[test]
fn reorders_tab_before_target() {
    let mut tabs = vec![tab(1), tab(2), tab(3)];

    reorder_tabs_by_id(&mut tabs, 3, 1, DropPlacement::Before);

    assert_eq!(tab_ids(&tabs), vec![3, 1, 2]);
}

#[test]
fn dirty_tabs_require_close_confirmation() {
    let mut dirty_tab = tab(2);
    dirty_tab.dirty = true;
    let tabs = vec![tab(1), dirty_tab];

    assert!(!tab_requires_close_confirmation(&tabs, 1));
    assert!(tab_requires_close_confirmation(&tabs, 2));
    assert!(!tab_requires_close_confirmation(&tabs, 3));
}

#[test]
fn clamps_panel_width_to_original_bounds() {
    assert_eq!(clamp_panel_width(120.0), PANEL_MIN_WIDTH);
    assert_eq!(clamp_panel_width(333.4), 333);
    assert_eq!(clamp_panel_width(620.0), PANEL_MAX_WIDTH);
}

#[test]
fn maps_theme_preferences_to_shell_and_editor_themes() {
    assert_eq!(
        ThemePreference::System.shell_class(),
        "rton-app-shell font-sans system-theme"
    );
}

#[test]
fn round_trips_editor_mode_preference_codes() {
    for mode in [
        EditorMode::RtonHex,
        EditorMode::Json,
        EditorMode::Yaml,
        EditorMode::Toml,
    ] {
        assert_eq!(EditorMode::from_code(mode.code()), Some(mode));
    }
    assert_eq!(EditorMode::from_code(" YAML\n"), Some(EditorMode::Yaml));
    assert_eq!(EditorMode::from_code("unknown"), None);
}

#[test]
fn creates_valid_blank_tabs_for_every_editor_mode() {
    let options = EncodeOptions {
        encoding: BinaryEncoding::Compact,
        encrypted: true,
    };

    for mode in [
        EditorMode::RtonHex,
        EditorMode::Json,
        EditorMode::Yaml,
        EditorMode::Toml,
    ] {
        let tab = create_blank_tab_state(7, mode, options).expect("blank tab is created");

        assert_eq!(tab.file_name, format!("untitled-7.{}", mode.code()));
        assert_eq!(tab.mode, mode);
        assert!(!tab.dirty);
        let document = document_for_tab(&tab).expect("blank tab parses");

        match mode {
            EditorMode::RtonHex => {
                assert_eq!(document.value, RtonValue::Object(Vec::new()));
                assert_eq!(tab.source_encode_options.encoding, BinaryEncoding::Compact);
                assert!(!tab.source_encode_options.encrypted);
            }
            EditorMode::Json => {
                assert_eq!(tab.text_buffer.unwrap().materialize(), "{}");
                assert_eq!(document.value, RtonValue::Object(Vec::new()));
            }
            EditorMode::Yaml => {
                assert_eq!(tab.text_buffer.unwrap().materialize(), "---\n");
            }
            EditorMode::Toml => {
                assert_eq!(tab.text_buffer.unwrap().materialize(), "\n");
                assert_eq!(document.value, RtonValue::Object(Vec::new()));
            }
        }
    }
}

#[test]
fn normalizes_toolbar_rows_and_appends_missing_groups() {
    let rows = normalize_toolbar_rows(Some(
        r#"[["prefs","file","file","unknown"],["about","format"],["edit"]]"#,
    ));

    assert_eq!(
        rows,
        vec![
            vec![ToolbarGroupId::Preferences, ToolbarGroupId::File],
            vec![
                ToolbarGroupId::Edit,
                ToolbarGroupId::TextExport,
                ToolbarGroupId::RtonExport,
            ],
        ]
    );
    assert_eq!(
        normalize_toolbar_rows(Some("not json")),
        default_toolbar_rows()
    );
}

#[test]
fn moves_toolbar_groups_across_rows() {
    let mut rows = default_toolbar_rows();

    move_toolbar_group(
        &mut rows,
        ToolbarGroupId::Preferences,
        ToolbarGroupId::File,
        DropPlacement::Before,
    );
    assert_eq!(
        rows,
        vec![
            vec![
                ToolbarGroupId::Preferences,
                ToolbarGroupId::File,
                ToolbarGroupId::Edit,
            ],
            vec![ToolbarGroupId::TextExport, ToolbarGroupId::RtonExport,],
        ]
    );

    move_toolbar_group_to_row_end(&mut rows, ToolbarGroupId::File, 1);
    assert_eq!(
        rows,
        vec![
            vec![ToolbarGroupId::Preferences, ToolbarGroupId::Edit],
            vec![
                ToolbarGroupId::TextExport,
                ToolbarGroupId::RtonExport,
                ToolbarGroupId::File,
            ],
        ]
    );

    let encoded = toolbar_rows_to_json(&rows);
    assert_eq!(normalize_toolbar_rows(Some(&encoded)), rows);
}
