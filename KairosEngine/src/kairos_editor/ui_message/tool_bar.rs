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
    
    fn get_message_data(&self) -> &dyn std::any::Any {
        self
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