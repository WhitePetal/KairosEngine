use crate::kairos_editor::ui_message::MessageID;


pub struct SetToolBarHeightMessage {
    height: f32
}

impl SetToolBarHeightMessage {
    pub fn new(height: f32) -> Self {
        Self { height }    
    }

    pub fn height(&self) -> f32 {
        self.height
    }
}

pub struct ShowAboutWindow {

}

impl ShowAboutWindow {
    pub fn new() -> Self {
        Self { }
    }
}