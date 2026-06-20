use super::Color32;

impl From<Color32> for egui::Color32 {
    fn from(c: Color32) -> Self {
        egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
    }
}

impl From<egui::Color32> for Color32 {
    fn from(c: egui::Color32) -> Self {
        Color32::new(c.r(), c.g(), c.b(), c.a())
    }
}
