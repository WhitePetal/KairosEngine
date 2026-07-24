use egui::TextureId;

pub struct EguiTextureHandle {
    id: TextureId,
    sender: std::sync::mpsc::Sender<TextureId>,
}

impl EguiTextureHandle {
    pub fn new(id: TextureId, sender: std::sync::mpsc::Sender<TextureId>) -> Self {
        Self { id, sender }
    }

    pub fn id(&self) -> TextureId {
        self.id
    }
}

impl Drop for EguiTextureHandle {
    fn drop(&mut self) {
        let _ = self.sender.send(self.id);
    }
}
