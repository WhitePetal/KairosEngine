use eframe::egui::{self, Color32, RichText};

use crate::kairos_editor::{UI, UIFactor, UIID, ui_message::Messager};



pub struct AboutWindow {

}

impl AboutWindow {
    fn new() -> Self {
        Self {  }
    }

    pub fn update(&self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame, messager: &mut Messager) {
        egui::SidePanel::left("AboutWindow")
            .frame(egui::Frame::NONE.fill(Color32::BLUE))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // ui.add_space(100.0);
                    ui.label(RichText::new("About Kairos Engine").size(24.0).color(Color32::LIGHT_GRAY));
                    ui.label(RichText::new("About Window demo").size(14.0).color(Color32::GRAY));
                }
            );
        });
    }
}

impl UIFactor for  AboutWindow {
    fn new() -> UI
    {
        UI::AboutWindow(Self::new())
    }
    
    fn id() -> UIID {
        UIID::AboutWindow
    }    
}