use eframe::egui::Visuals;


pub mod consts;
pub mod ui;

pub struct KairosEngine {
    ui_context: ui::Context,
}

impl KairosEngine {
    pub fn new(_cc: &eframe::CreationContext) -> Result<Self, Box<dyn std::error::Error>> {
        let mut ui_context = ui::Context::new();
        ui_context.messager.send(ui::Message::CreateToolbar);
        ui_context.messager.send(ui::Message::CreateMainContent);

        Ok(Self{
            ui_context
        })
    }
}

impl eframe::App for KairosEngine {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ctx.set_visuals(visuals);

        self.ui_context.handle(ctx);

        self.ui_context.darw(ctx, frame);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        
    }
}

// struct Context {
//     param: String,
//     id: u32,
// }

// trait FromContect {
//     fn from_context(context: &Context) -> Self;
// }

// struct Params(String);

// struct Id(u32);

// impl FromContect for Params {
//     fn from_context(context: &Context) -> Self {
//         Self(context.param.clone())
//     }
// }

// trait Handler<T> {
//     fn call(self, context: Context);
// }

// impl<F, T> Handler<T> for F
//     where 
//         F: Fn(T),
//         T: FromContect
// {
//     fn call(self, context: Context) {
//         self(T::from_context(&context))
//     }
// }

// fn trigger<T, H>(context: Context, handler: H)
//     where H: Handler<T>
// {
//     handler.call(context);
// }

// fn print_param(param: Params)
// {
//     println!("Param is {}", param.0);
// }

// fn print_all(Params(param): Params, Id(id) : Id) {
//     println!("param is {param}, id is {id}");
// }

// fn test() {
//     let context = Context {
//         param: "WTF".to_string(),
//         id: 32
//     };

//     trigger(context, print_param);
// }