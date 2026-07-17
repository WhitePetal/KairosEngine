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

impl From<[u8; 3]> for Color32 {
    fn from(ar: [u8; 3]) -> Self {
        Color32::new(ar[0], ar[1], ar[2], 255)
    }
}
impl From<[u8; 4]> for Color32 {
    fn from(ar: [u8; 4]) -> Self {
        Color32::new(ar[0], ar[1], ar[2], ar[3])
    }
}

impl From<Color32> for syntect::highlighting::Color {
    fn from(value: Color32) -> Self {
        Self {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}
