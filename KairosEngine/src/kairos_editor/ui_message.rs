pub mod tool_bar;

use std::{any::{Any, TypeId}, cell::RefCell, collections::HashMap, rc::Rc};

pub enum Message {
    SetToolBarHeight(f32),
    OpenAboutWindow,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum MessageID {
    SetToolBarHeight,
    OpenAboutWindow
}

pub trait MessageHandler {
    fn handle(&self, msg: &Message);
}

pub struct Messager {
    messages: RefCell<HashMap<MessageID, Vec<Rc<dyn MessageHandler>>>>
}

impl Messager {
    pub fn new() -> Self {
        Self { messages: RefCell::new(HashMap::new()) }
    }

    pub fn registe(&self, id: MessageID, handler: Rc<dyn MessageHandler>) {
        let mut messages = self.messages.borrow_mut();
        messages.entry(id).or_default().push(handler);
    }

    pub fn send(&self, id: &MessageID, msg: &Message) {
        let messages = self.messages.borrow();
        println!("FK?: {:?}", id);
        if let Some(handlers)  = messages.get(id) {
            for handler in handlers.iter() {
                let handler = Rc::clone(handler);
                println!("FK");
                handler.handle(msg);
            }
        }
    }
}