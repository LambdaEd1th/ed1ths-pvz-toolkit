use std::sync::Arc;

use pam_viewer_core::{
    AddsInfo, FrameInfo, ImageAsset, ImageInfo, MovesInfo, PamInfo, SpriteInfo, SpriteKey,
    compile_animation, decode_json, encode_json, parse_frame_labels,
};

fn image_asset() -> ImageAsset {
    ImageAsset::new(
        "image",
        8,
        8,
        Arc::<[u8]>::from(vec![255; 8 * 8 * 4]),
        Arc::<[u8]>::from([]),
    )
}

#[test]
fn json_uses_the_pam_codec_rectangle_shape() {
    let json = r#"{
      "version": 6,
      "frame_rate": 30,
      "position": [0.0, 0.0],
      "size": [64.0, 64.0],
      "image": [],
      "sprite": [],
      "main_sprite": null
    }"#;
    let pam = decode_json(json).unwrap();
    let encoded = encode_json(&pam).unwrap();
    assert!(encoded.contains("\"main_sprite\": null"));

    let legacy = r#"{
      "version": 6,
      "frame_rate": 30,
      "position": [0.0, 0.0],
      "size": [64.0, 64.0],
      "image": [],
      "sprite": [],
      "main_sprite": {
        "name": "main",
        "frame_rate": 30.0,
        "work_area": [0, 1],
        "frame": [{"change": [{
          "index": 0,
          "transform": [0.0, 0.0],
          "source_rectangle": [0.0, 0.0, 1.0, 1.0]
        }]}]
      }
    }"#;
    assert!(decode_json(legacy).is_err());
}

#[test]
fn labels_close_on_stop_and_orphans_are_ignored() {
    let sprite = SpriteInfo {
        frame: vec![
            FrameInfo {
                label: Some("idle".into()),
                ..Default::default()
            },
            FrameInfo {
                stop: true,
                ..Default::default()
            },
            FrameInfo {
                label: Some("orphan".into()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let labels = parse_frame_labels(&sprite);
    assert_eq!(labels.len(), 1);
    assert_eq!((labels[0].begin, labels[0].end), (0, 1));
}

#[test]
fn an_initial_child_sprite_is_flattened_on_frame_zero() {
    let child = SpriteInfo {
        name: Some("child".into()),
        frame_rate: Some(30.0),
        work_area: Some([0, 1]),
        frame: vec![FrameInfo {
            append: vec![AddsInfo {
                index: 0,
                name: None,
                resource: 0,
                sprite: false,
                additive: false,
                preload_frame: 0,
                time_scale: 1.0,
            }],
            change: vec![MovesInfo {
                index: 0,
                transform: vec![5.0, 7.0],
                color: None,
                source_rectangle: None,
                sprite_frame_number: None,
            }],
            ..Default::default()
        }],
    };
    let main = SpriteInfo {
        name: Some("main".into()),
        frame_rate: Some(30.0),
        work_area: Some([0, 1]),
        frame: vec![FrameInfo {
            append: vec![AddsInfo {
                index: 0,
                name: None,
                resource: 0,
                sprite: true,
                additive: false,
                preload_frame: 0,
                time_scale: 1.0,
            }],
            change: vec![MovesInfo {
                index: 0,
                transform: vec![2.0, 3.0],
                color: None,
                source_rectangle: None,
                sprite_frame_number: None,
            }],
            ..Default::default()
        }],
    };
    let pam = PamInfo {
        version: 6,
        frame_rate: 30,
        position: [0.0, 0.0],
        size: [64.0, 64.0],
        image: vec![ImageInfo {
            name: "image".into(),
            size: Some([8, 8]),
            transform: vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }],
        sprite: vec![child],
        main_sprite: Some(main),
    };
    let compiled = compile_animation(&pam).unwrap();
    let commands = compiled
        .flatten_frame(
            &pam,
            &[Some(image_asset())],
            SpriteKey::Main,
            0,
            &[true],
            &[true],
        )
        .unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!((commands[0].matrix[4], commands[0].matrix[5]), (7.0, 10.0));
}
