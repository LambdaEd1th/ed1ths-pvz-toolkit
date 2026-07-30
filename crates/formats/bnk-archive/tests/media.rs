use bnk_archive::{BankHeader, BankVersion, OwnedEmbeddedMedia, SoundBank, from_bytes, to_bytes};

#[test]
fn embedded_media_can_be_created_resolved_and_replaced() {
    let mut bank = SoundBank {
        header: BankHeader {
            version: BankVersion::V140,
            id: 1,
            language: 0,
            header_expand: Vec::new(),
        },
        chunks: Vec::new(),
    };
    bank.set_embedded_media(
        [
            OwnedEmbeddedMedia {
                id: 10,
                data: vec![1, 2, 3],
            },
            OwnedEmbeddedMedia {
                id: 20,
                data: vec![4, 5],
            },
        ],
        16,
    )
    .unwrap();

    let bytes = to_bytes(&bank).unwrap();
    let decoded = from_bytes(&bytes).unwrap();
    let media = decoded.embedded_media().unwrap();
    assert_eq!(media[0].id, 10);
    assert_eq!(media[0].data, [1, 2, 3]);
    assert_eq!(media[1].id, 20);
    assert_eq!(media[1].data, [4, 5]);
}
