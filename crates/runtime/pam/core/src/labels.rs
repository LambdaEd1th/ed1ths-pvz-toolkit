use pam_codec::SpriteInfo;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameLabel {
    pub name: String,
    pub begin: usize,
    pub end: usize,
}

/// PAM labels remain pending until a stop frame closes the segment. Orphaned
/// labels are intentionally omitted, matching the PopAnim/Twinning behavior.
pub fn parse_frame_labels(sprite: &SpriteInfo) -> Vec<FrameLabel> {
    let mut labels = Vec::new();
    let mut pending = Vec::<(String, usize)>::new();
    for (index, frame) in sprite.frame.iter().enumerate() {
        if let Some(label) = frame.label.as_ref() {
            pending.push((label.clone(), index));
        }
        if frame.stop {
            labels.extend(pending.drain(..).map(|(name, begin)| FrameLabel {
                name,
                begin,
                end: index,
            }));
        }
    }
    labels
}
