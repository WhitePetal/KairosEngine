pub mod tool_bar;

use std::{any::{Any, TypeId}, collections::HashMap};

pub trait Message{
    fn get_id() -> TypeId where Self: Sized;

    fn id(&self) -> TypeId;

    fn get_message_data(&self) -> &dyn Any;
}

pub struct Messager {
    messages: HashMap<TypeId, Vec<Box<dyn FnMut(&dyn Message)>>>
}

impl Messager {
    pub fn new() -> Self {
        Self { messages: HashMap::new() }
    }

    pub fn registe(&mut self, id: TypeId, handler: Box<dyn FnMut(&dyn Message)>) {
        self.messages.entry(id).or_default().push(handler);
    }

    pub fn send(&mut self, msg: &dyn Message) {
        let id = msg.id();
        if let Some(handlers)  = self.messages.get_mut(&id) {
            for handler in handlers.iter_mut() {
                handler(msg);
            }
        }
    }
}