use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily, Id};

#[derive(Clone, Copy)]
pub struct Theme {}

fn theme_key() -> Id {
    Id::new("kairos-ui-widgets.theme")
}

/// The editor UI font. Unity/Tuanjie ship Inter (SIL OFL 1.1) for their editor
/// chrome; using the same face is what makes the Kairos editor read like Unity
/// instead of like a stock egui app. The licence accompanies the files in
/// `assets/fonts/Inter-LICENSE.txt`.
const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.otf");
const INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.otf");

impl Theme {
    pub fn dark() -> Self {
        Self {}
    }

    pub fn light() -> Self {
        Self {}
    }

    pub fn install(self, ctx: &Context) {
        ctx.data_mut(|d| d.insert_temp(theme_key(), self));
        self.apply_egui_style(ctx);
    }

    pub fn apply_egui_style(&self, ctx: &Context) {
        install_unity_fonts(ctx);
    }
}

/// Installs Inter as the proportional UI face, keeping egui's default font as a
/// fallback for glyphs Inter does not cover (CJK, extra symbols).
fn install_unity_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "Inter".to_owned(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts.font_data.insert(
        "Inter Medium".to_owned(),
        Arc::new(FontData::from_static(INTER_MEDIUM)),
    );

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "Inter".to_owned());
    proportional.insert(1, "Inter Medium".to_owned());

    ctx.set_fonts(fonts);
}
