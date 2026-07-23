use super::super::*;

#[test]
fn locates_value_paths_in_text_editor_content() {
    let doc = parse_text(SAMPLE_JSON, TextFormat::Json).expect("sample parses");

    let aliases_item =
        locate_value_path_in_text(&doc.value, "$.aliases#1[1]", SAMPLE_JSON, TextFormat::Json)
            .expect("array item is located");
    assert_eq!(aliases_item.line, 5);
    assert_eq!(aliases_item.column, 4);

    let objclass =
        locate_value_path_in_text(&doc.value, "$.objclass#0", SAMPLE_JSON, TextFormat::Json)
            .expect("object key is located");
    assert_eq!(objclass.line, 2);
    assert_eq!(objclass.column, 2);

    let yaml = value_to_text(&doc.value, TextFormat::Yaml).expect("yaml serializes");
    let yaml_alias =
        locate_value_path_in_text(&doc.value, "$.aliases#1[1]", &yaml, TextFormat::Yaml)
            .expect("yaml array item is located");
    assert_eq!(
        yaml_alias,
        offset_to_text_position(
            &yaml,
            yaml.find("TutorialGate").expect("yaml contains value")
        )
    );
    let yaml_objclass =
        locate_value_path_in_text(&doc.value, "$.objclass#0", &yaml, TextFormat::Yaml)
            .expect("yaml key is located");
    assert_eq!(
        yaml_objclass,
        offset_to_text_position(&yaml, yaml.find("objclass:").expect("yaml contains key"))
    );

    let toml = value_to_text(&doc.value, TextFormat::Toml).expect("toml serializes");
    let toml_alias =
        locate_value_path_in_text(&doc.value, "$.aliases#1[1]", &toml, TextFormat::Toml)
            .expect("toml array item is located");
    assert_eq!(
        toml_alias,
        offset_to_text_position(
            &toml,
            toml.find("\"TutorialGate\"")
                .expect("toml contains array item")
        )
    );
    let toml_objclass =
        locate_value_path_in_text(&doc.value, "$.objclass#0", &toml, TextFormat::Toml)
            .expect("toml key is located");
    assert_eq!(
        toml_objclass,
        offset_to_text_position(&toml, toml.find("objclass").expect("toml contains key"))
    );

    assert_eq!(
        locate_value_path_in_text(
            &doc.value,
            "$.objclass#0",
            "not_objclass = \"objclass\"\n",
            TextFormat::Toml
        ),
        None
    );
    assert_eq!(
        locate_value_path_in_text(
            &doc.value,
            "$.objclass#0",
            "[other]\nobjclass = \"wrong scope\"\n",
            TextFormat::Toml
        ),
        None
    );
}

#[test]
fn locates_value_paths_in_rton_bytes() {
    let doc = parse_text(SAMPLE_JSON, TextFormat::Json).expect("sample parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");
    let byte_doc = ByteDocument::from_vec(bytes);

    assert_eq!(locate_rton_value_offset(&byte_doc, "$"), Some(8));

    let aliases =
        locate_rton_value_offset(&byte_doc, "$.aliases#1").expect("array value offset is located");
    assert_eq!(byte_doc.byte_at(aliases), Some(0x86));

    let second_alias = locate_rton_value_offset(&byte_doc, "$.aliases#1[1]")
        .expect("nested array item offset is located");
    assert!(matches!(
        byte_doc.byte_at(second_alias),
        Some(0x81 | 0x82 | 0x90 | 0x92)
    ));
}
