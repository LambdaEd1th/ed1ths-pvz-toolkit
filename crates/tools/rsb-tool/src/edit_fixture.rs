//! Small, deterministic archive shared by edit/save regression tests.
use rsb_archive::{
    RsbArchiveEdit, RsbHeader, RsbPtxInfo, RsbWriter, RsgPacketAddition, RsgPacketGroup,
    UnpackedFile,
};
use serde_json::{Value, json};
use std::io::Cursor;

pub fn manifest() -> Value {
    json!({"slot_count":3,"groups":[
        {"type":"composite","id":"Textures","subgroups":[{"id":"Textures_768","res":768}]},
        {"type":"simple","id":"Textures_768","parent":"Textures","res":768,"resources":[
            {"type":"Image","slot":0,"id":"ATLAS","path":"atlases/test","atlas":true,"width":4,"height":4},
            {"type":"Image","slot":1,"id":"LEAF","path":"images/leaf","parent":"ATLAS","ax":1,"ay":1,"aw":2,"ah":2,"x":-1,"y":3}
        ]},
        {"type":"simple","id":"Data_Common","resources":[{"type":"File","slot":2,"id":"CONFIG","path":"data/config.txt"}]}
    ]})
}

pub fn file(path: &str, data: Vec<u8>) -> UnpackedFile {
    UnpackedFile {
        path: path.into(),
        data,
        is_part1: false,
        part1_info: None,
    }
}

pub fn bytes() -> Vec<u8> {
    let header = RsbHeader {
        magic: rsb_archive::RSB_MAGIC,
        version: 4,
        information_section_size: 4096,
        resource_path_section_size: 0,
        resource_path_section_offset: 0,
        rsg_list_length: 0,
        rsg_list_begin_offset: 0,
        rsg_number: 0,
        rsg_info_begin_offset: 0,
        rsg_info_each_length: 204,
        composite_number: 0,
        composite_info_begin_offset: 0,
        composite_info_each_length: 1156,
        composite_list_length: 0,
        composite_list_begin_offset: 0,
        autopool_number: 0,
        autopool_info_begin_offset: 0,
        autopool_info_each_length: 152,
        ptx_number: 0,
        ptx_info_begin_offset: 0,
        ptx_info_each_length: 24,
        part1_begin_offset: 0,
        part2_begin_offset: 0,
        part3_begin_offset: 0,
        information_without_manifest_section_size: 4096,
    };
    let mut source = Cursor::new(vec![0; 4096]);
    RsbWriter::new(&mut source).write_header(&header).unwrap();
    let newton = serde_json::from_value(manifest()).unwrap();
    let mut rton = manifest();
    rton["unknown"] = json!({"keep":true});
    rton["groups"][1]["resources"][1]["custom"] = json!({"not_exposed":"preserve"});
    let packet = |name: &str, files, group: &str, res: &str| RsgPacketAddition {
        name: name.into(),
        version: 4,
        compression_flags: 3,
        files,
        group: Some(RsgPacketGroup {
            name: group.into(),
            is_composite: name != group,
            category: [res.into(), String::new()],
        }),
    };
    rsb_archive::rebuild_rsb(
        source.get_ref(),
        &RsbArchiveEdit {
            added_packets: vec![
                packet(
                    "__MANIFESTGROUP__",
                    vec![
                        file(
                            "properties/resources.rton",
                            serde_rton::to_bytes(&rton).unwrap(),
                        ),
                        file(
                            "properties/resources.newton",
                            newton_manifest::to_bytes(&newton).unwrap(),
                        ),
                    ],
                    "__MANIFESTGROUP__",
                    "0",
                ),
                packet(
                    "Textures_768",
                    vec![UnpackedFile {
                        path: "atlases/test.ptx".into(),
                        data: [20, 40, 60, 255].repeat(16),
                        is_part1: true,
                        part1_info: Some(rsb_archive::Part1Extra {
                            id: 0,
                            width: 4,
                            height: 4,
                        }),
                    }],
                    "Textures",
                    "768",
                ),
                packet(
                    "Data_Common",
                    vec![file("data/config.txt", b"original config".to_vec())],
                    "Data_Common",
                    "0",
                ),
            ],
            ptx_infos: Some(vec![RsbPtxInfo {
                ptx_index: 0,
                width: 4,
                height: 4,
                pitch: 16,
                format: 0,
                alpha_size: Some(0),
                alpha_format: Some(0),
            }]),
            ..Default::default()
        },
    )
    .unwrap()
}

pub fn archive() -> std::sync::Arc<crate::domain::ArchiveDocument> {
    std::sync::Arc::new(crate::loader::open_memory("resource-edit.rsb".into(), bytes()).unwrap())
}

#[test]
fn write_browser_fixture_when_requested() {
    if let Ok(path) = std::env::var("RSB_EDIT_FIXTURE_PATH") {
        std::fs::write(path, bytes()).unwrap();
    }
}

pub fn with_description() -> std::sync::Arc<crate::domain::ArchiveDocument> {
    use std::io::{Seek, SeekFrom};
    let mut archive = archive().as_ref().clone();
    let mut output = Cursor::new(archive.metadata_bytes().unwrap());
    let mut header = archive.header.clone();
    let ptx = |parent: &str, x: u32, y: u32, size: u32| json!({"imagetype":"0","aflags":"0","x":"0","y":"0","ax":x.to_string(),"ay":y.to_string(),"aw":size.to_string(),"ah":size.to_string(),"rows":"1","cols":"1","parent":parent});
    let description = json!({"groups":{"Textures":{"composite":true,"subgroups":{"Textures_768":{"res":"768","language":"","resources":{
        "ATLAS":{"type":0,"path":"atlases/test","properties":{"keep":"yes"},"ptx_info":ptx("",0,0,4)},
        "LEAF":{"type":0,"path":"images/leaf","properties":{},"ptx_info":ptx("ATLAS",1,1,2)}
    }}}}}});
    let description = serde_json::from_value(description).unwrap();
    output.seek(SeekFrom::End(0)).unwrap();
    RsbWriter::new(&mut output)
        .write_resources_description(&description, &mut header)
        .unwrap();
    header.information_section_size = output.get_ref().len() as u32;
    output.set_position(0);
    RsbWriter::new(&mut output).write_header(&header).unwrap();
    archive.metadata_override = Some(std::sync::Arc::new(output.into_inner()));
    std::sync::Arc::new(archive)
}
