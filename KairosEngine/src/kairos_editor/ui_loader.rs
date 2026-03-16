use std::{collections::HashMap, rc::Rc};

use crate::kairos_editor::{UI, UIFactor, UIID};



pub struct UILoader {
    caches: HashMap<UIID, Rc<UI>>
}

impl UILoader {
    pub fn new() -> Self {
        Self { 
            caches: HashMap::new()
        }
    }

    pub fn load_ui<T: UIFactor>(&mut self, id: &UIID) -> Rc<UI> {
        if !self.caches.contains_key(id) {
            self.caches.insert(*id, Rc::new(T::new()));
        }
        Rc::clone(&self.caches[id])
    }
}