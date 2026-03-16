use crate::kairos_editor::floating_window::about_window::AboutWindow;



pub mod paths;
pub mod ui_message;
pub mod ui_loader;
pub mod main_window;
pub mod floating_window;

struct OtherWindow {

}

pub trait UIFactor {
    fn new() -> UI;
    fn id() -> UIID;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum UIID {
    AboutWindow,
    OtherWindow
}

pub enum UI {
    AboutWindow(AboutWindow),
    OtherWindow(OtherWindow),
}

pub enum FloatingWindow {
    AboutWindow(AboutWindow)
}