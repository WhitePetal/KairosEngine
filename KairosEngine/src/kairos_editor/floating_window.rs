use std::rc::Rc;

use crate::kairos_editor::{UI};


pub enum FloatingWindow {
    AboutWindow(Rc<UI>),
}

impl UI {
    pub fn to_floating_window(ui: Rc<UI>) -> Option<FloatingWindow> {
        match ui.as_ref() {
            UI::AboutWindow(_) => Some(FloatingWindow::AboutWindow(ui)),
            _ => None
        }
    }    
}