use std::{cell::RefCell, rc::Rc};

use egui::{Ui, WidgetText};

#[derive(Clone)]
pub struct TabState {
    pub tab: Rc<RefCell<dyn Tab>>,
}

impl TabState {
    pub fn new(tab: Rc<RefCell<dyn Tab>>) -> Self {
        Self {
            tab
        }
    }

    pub fn title(&self) -> WidgetText {
        self.tab.borrow().title()
    }

    pub fn context_menu(&mut self, ui: &mut Ui) {
        self.tab.borrow_mut().context_menu(ui);
    }

    pub fn on_rect_changed(&mut self) {
        self.tab.borrow_mut().on_rect_changed();
    }

    pub fn scroll_bars(&self) -> [bool; 2] {
        self.tab.borrow().scroll_bars()
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.tab.borrow_mut().ui(ui);
    }
}

pub trait Tab {
    fn title(&self) -> WidgetText;

    fn context_menu(&mut self, _ui: &mut Ui) {}

    fn on_rect_changed(&mut self) {}

    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
    }

    fn ui(&mut self, ui: &mut Ui);
}
