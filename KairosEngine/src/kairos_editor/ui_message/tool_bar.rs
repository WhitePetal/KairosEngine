use std::any::TypeId;
use super::Message;


pub struct SetToolBarHeightMessage {
    id: TypeId,
    height: f32
}
impl Message for SetToolBarHeightMessage {
    fn get_id() -> TypeId {
        TypeId::of::<Self>()
    }
    
    fn id(&self) -> TypeId {
        self.id
    }
}
impl SetToolBarHeightMessage {
    pub fn new(height: f32) -> Self {
        Self { id: Self::get_id(), height }    
    }

    pub fn height(&self) -> f32 {
        self.height
    }
}

pub struct ShowAboutWindow {
    id: TypeId
}
impl Message for ShowAboutWindow {
    fn get_id() -> TypeId {
        TypeId::of::<Self>()
    }
    
    fn id(&self) -> TypeId {
        self.id
    }
}
impl ShowAboutWindow {
    pub fn new() -> Self {
        Self { id: Self::get_id() }
    }
}