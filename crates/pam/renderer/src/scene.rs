use std::sync::Arc;

use pam_viewer_core::{PamDocument, Rect, SpriteKey};
use parking_lot::RwLock;

#[derive(Clone, Debug)]
pub struct StageScene {
    pub document: Option<Arc<PamDocument>>,
    pub document_revision: u64,
    pub sprite: SpriteKey,
    pub frame: usize,
    pub image_filter: Vec<bool>,
    pub sprite_filter: Vec<bool>,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub boundary: bool,
    pub dark_background: bool,
}

impl Default for StageScene {
    fn default() -> Self {
        Self {
            document: None,
            document_revision: 0,
            sprite: SpriteKey::Main,
            frame: 0,
            image_filter: Vec::new(),
            sprite_filter: Vec::new(),
            zoom: 1.0,
            pan: [0.0, 0.0],
            boundary: true,
            dark_background: true,
        }
    }
}

impl StageScene {
    pub fn replace(&mut self, mut next: Self) {
        let same_document = match (&self.document, &next.document) {
            (Some(current), Some(next)) => Arc::ptr_eq(current, next),
            (None, None) => true,
            _ => false,
        };
        next.document_revision = if same_document {
            self.document_revision
        } else {
            self.document_revision.wrapping_add(1)
        };
        *self = next;
    }

    pub fn set_document(&mut self, document: Option<Arc<PamDocument>>) {
        self.document_revision = self.document_revision.wrapping_add(1);
        self.image_filter = document
            .as_ref()
            .map(|document| vec![true; document.pam.image.len()])
            .unwrap_or_default();
        self.sprite_filter = document
            .as_ref()
            .map(|document| vec![true; document.pam.sprite.len()])
            .unwrap_or_default();
        self.sprite = SpriteKey::Main;
        self.frame = 0;
        self.zoom = 1.0;
        self.pan = document
            .as_ref()
            .map(|document| {
                let bounds = document.stage_bounds();
                [
                    -bounds.x - bounds.width / 2.0,
                    -bounds.y - bounds.height / 2.0,
                ]
            })
            .unwrap_or([0.0, 0.0]);
        self.document = document;
    }

    pub fn active_frame_count(&self) -> usize {
        let Some(document) = self.document.as_ref() else {
            return 0;
        };
        match self.sprite {
            SpriteKey::Main => document
                .pam
                .main_sprite
                .as_ref()
                .map(|sprite| sprite.frame.len())
                .unwrap_or(0),
            SpriteKey::Sprite(index) => document
                .pam
                .sprite
                .get(index)
                .map(|sprite| sprite.frame.len())
                .unwrap_or(0),
        }
    }

    pub fn stage_bounds(&self) -> Option<Rect> {
        self.document
            .as_ref()
            .map(|document| document.stage_bounds())
    }
}

#[derive(Clone, Default)]
pub struct SharedStage(pub Arc<RwLock<StageScene>>);

impl SharedStage {
    pub fn snapshot(&self) -> StageScene {
        self.0.read().clone()
    }

    pub fn update(&self, update: impl FnOnce(&mut StageScene)) {
        update(&mut self.0.write());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pam_viewer_core::PamInfo;

    fn document(name: &str) -> Arc<PamDocument> {
        Arc::new(
            PamDocument::new(
                name,
                PamInfo {
                    version: 6,
                    frame_rate: 30,
                    position: [0.0, 0.0],
                    size: [64.0, 64.0],
                    image: Vec::new(),
                    sprite: Vec::new(),
                    main_sprite: None,
                },
                Vec::new(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn document_revision_advances_when_a_document_is_reopened() {
        let first = document("first.pam");
        let reopened = document("first.pam");
        let mut scene = StageScene::default();

        scene.replace(StageScene {
            document: Some(first.clone()),
            ..StageScene::default()
        });
        let first_revision = scene.document_revision;

        scene.replace(StageScene {
            document: Some(first),
            frame: 1,
            ..StageScene::default()
        });
        assert_eq!(scene.document_revision, first_revision);

        scene.set_document(None);
        let empty_revision = scene.document_revision;

        scene.replace(StageScene {
            document: Some(reopened),
            ..StageScene::default()
        });
        assert_eq!(scene.document_revision, empty_revision.wrapping_add(1));
    }
}
