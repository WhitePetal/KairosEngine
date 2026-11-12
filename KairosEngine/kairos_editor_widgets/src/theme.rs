use egui::{Context, Id};



#[derive(Clone, Copy)]
pub struct Theme {

}

fn theme_key() -> Id {
    Id::new("kairos-ui-widgets.theme")
}

impl Theme {
    pub fn dark() -> Self {
        Self {  }
    }

    pub fn light() -> Self {
        Self {  }
    }

    pub fn install(self, ctx: &Context) {
        ctx.data_mut(|d| d.insert_temp(theme_key(), self));
        self.apply_egui_style(ctx);
    }

    pub fn apply_egui_style(&self, ctx: &Context) {

    }
}
