use crate::kairos_editor::ui::Drawer;



pub mod tree;

pub enum Surface {
    Empty,
    Main(Box<dyn Drawer>),
    Window(Box<dyn Drawer>)
}

pub struct DockState {

}