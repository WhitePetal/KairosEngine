use crate::kairos_editor::ui::Drawer;



pub struct ConsoleWindowStyle {

}

pub struct ConsoleWindowModel {
    style: ConsoleWindowStyle,
}

pub struct ConsoleWindow {
    modle: ConsoleWindowModel,
}

impl ConsoleWindow {
    
}

impl Drawer for ConsoleWindow {
    fn update(&self, _ctx: &eframe::egui::Context, _frame: &mut eframe::Frame, _messager: &mut super::Messager) {
        todo!()
    }

    fn get_name(&self) -> &'static str {
        todo!()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        todo!()
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        todo!()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        todo!()
    }

    fn update_style(&mut self, style_fields: &Vec<super::ui_style_fields::StyleField>) {
        todo!()
    }
}