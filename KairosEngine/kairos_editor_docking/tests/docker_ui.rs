use egui::load::SizedTexture;
use egui::{
    Color32, ColorImage, Context, ImageSource, Pos2, RawInput, Rect, Shape, TextureOptions, Ui,
    Vec2, WidgetText,
};
use kairos_editor_docking::{
    docker::{DockState, Docker},
    viewer::{CloseResponse, TabViewer},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Id(u32);

struct Viewer {
    drawn: Vec<Id>,
}

impl TabViewer for Viewer {
    type Id = Id;

    fn title(&mut self, id: Self::Id) -> WidgetText {
        format!("tab {}", id.0).into()
    }

    fn ui(&mut self, _ui: &mut Ui, id: Self::Id) {
        self.drawn.push(id);
    }

    fn closeable(&mut self, _id: Self::Id) -> bool {
        true
    }

    fn on_close(&mut self, _id: Self::Id) -> CloseResponse {
        CloseResponse::Close
    }
}

#[test]
fn docker_draws_the_active_tab_body() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(7));

    let mut viewer = Viewer { drawn: Vec::new() };
    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));

    let _ = ctx.run_ui(input, |ui| {
        Docker::new("dock", &mut dock).show(ui, &mut viewer);
    });

    assert!(
        viewer.drawn.contains(&Id(7)),
        "active tab body was not drawn: {:?}",
        viewer.drawn
    );
}

#[test]
fn docker_draws_a_floating_window_surface() {
    use kairos_editor_docking::{
        split::Fraction,
        surface::SurfaceIndex,
        tree::{NodeIndex, TabIndex},
    };

    let mut dock: DockState<Id> = DockState::new();
    let [left, _right] =
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), Fraction::new(0.7), vec![Id(1)]);
    dock.main_surface_mut()[left].append_tab(Id(2));
    let window = dock.detach_tab(
        (SurfaceIndex::main(), left, TabIndex(0)),
        Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 240.0)),
    );
    assert!(window.is_some());

    let mut viewer = Viewer { drawn: Vec::new() };
    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));

    let _ = ctx.run_ui(input, |ui| {
        Docker::new("dock", &mut dock).show(ui, &mut viewer);
    });

    // Second frame: the window state now has position/size and goes through the
    // normal (non-initial) path with custom title bar and no tab bar (single tab).
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
    let _ = ctx.run_ui(input, |ui| {
        Docker::new("dock", &mut dock).show(ui, &mut viewer);
    });

    assert!(
        viewer.drawn.contains(&Id(2)),
        "detached tab body was not drawn: {:?}",
        viewer.drawn
    );
}

struct IconViewer {
    icon: ImageSource<'static>,
}

impl TabViewer for IconViewer {
    type Id = Id;

    fn icon(&mut self, _id: Self::Id) -> Option<ImageSource<'static>> {
        Some(self.icon.clone())
    }

    fn title(&mut self, id: Self::Id) -> WidgetText {
        format!("tab {}", id.0).into()
    }

    fn ui(&mut self, _ui: &mut Ui, _id: Self::Id) {}
}

/// Find the rect a texture was painted into.
///
/// egui 0.35 paints textures as brushed rects, not meshes.
fn find_icon(shape: &Shape, texture_id: egui::TextureId) -> Option<Rect> {
    match shape {
        Shape::Rect(rect)
            if rect
                .brush
                .as_ref()
                .is_some_and(|brush| brush.fill_texture_id == texture_id) =>
        {
            Some(rect.rect)
        }
        Shape::Vec(shapes) => shapes.iter().find_map(|shape| find_icon(shape, texture_id)),
        _ => None,
    }
}

fn find_text_pos(shape: &Shape) -> Option<Pos2> {
    match shape {
        Shape::Text(text) => Some(text.pos),
        Shape::Vec(shapes) => shapes.iter().find_map(find_text_pos),
        _ => None,
    }
}

#[test]
fn tab_icon_is_painted_inside_the_tab_header() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(3));

    let ctx = Context::default();
    let texture = ctx.load_texture(
        "tab-icon",
        ColorImage::filled([16, 16], Color32::WHITE),
        TextureOptions::default(),
    );
    let mut viewer = IconViewer {
        icon: ImageSource::Texture(SizedTexture::new(texture.id(), Vec2::splat(16.0))),
    };

    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
    let output = ctx.run_ui(input, |ui| {
        Docker::new("dock", &mut dock).show(ui, &mut viewer);
    });

    let icon_rect = output
        .shapes
        .iter()
        .find_map(|clipped| find_icon(&clipped.shape, texture.id()))
        .expect("the tab icon should be painted");

    // The icon is part of the tab header: it sits in the tab bar row and keeps
    // its configured size.
    assert!((icon_rect.width() - 16.0).abs() < 0.5, "{icon_rect:?}");
    assert!((icon_rect.height() - 16.0).abs() < 0.5, "{icon_rect:?}");
    assert!(icon_rect.center().y < 24.0, "{icon_rect:?}");

    // The title starts to the right of the icon instead of overlapping it.
    let text_pos = output
        .shapes
        .iter()
        .find_map(|clipped| find_text_pos(&clipped.shape))
        .expect("tab title should be painted");
    assert!(
        text_pos.x >= icon_rect.right(),
        "icon {icon_rect:?}, title {text_pos:?}"
    );
}

#[test]
fn focused_floating_window_is_raised_above_other_windows() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(1));
    let first = dock.open_in_window(
        Id(2),
        Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(240.0, 160.0)),
    );
    let second = dock.open_in_window(
        Id(3),
        Rect::from_min_size(Pos2::new(80.0, 80.0), Vec2::new(240.0, 160.0)),
    );
    assert_ne!(first, second);

    fn run_frame(ctx: &Context, dock: &mut DockState<Id>, viewer: &mut Viewer) {
        let mut input = RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let _ = ctx.run_ui(input, |ui| {
            Docker::new("dock", dock).show(ui, viewer);
        });
    }

    let mut viewer = Viewer { drawn: Vec::new() };
    let ctx = Context::default();

    // First pass creates both window layers; the last-created window is on top.
    run_frame(&ctx, &mut dock, &mut viewer);
    assert!(dock.focus_tab(Id(2)));
    run_frame(&ctx, &mut dock, &mut viewer);

    let expected = egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("dock").with(("window", first.0)),
    );
    assert_eq!(
        ctx.memory(|memory| memory.areas().top_layer_id(egui::Order::Middle)),
        Some(expected),
        "the focused floating window should be raised"
    );
}

#[test]
fn focusing_a_tab_paints_the_blue_headerline() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(1));
    dock.append_tab(Id(2));
    assert!(dock.focus_tab(Id(1)), "the dock has a tab for Id(1)");

    let mut viewer = Viewer { drawn: Vec::new() };
    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
    let output = ctx.run_ui(input, |ui| {
        Docker::new("dock", &mut dock).show(ui, &mut viewer);
    });

    fn has_fill(shape: &Shape, fill: Color32) -> bool {
        match shape {
            Shape::Rect(rect) => rect.fill == fill,
            Shape::Vec(shapes) => shapes.iter().any(|shape| has_fill(shape, fill)),
            _ => false,
        }
    }
    let headerline = Color32::from_rgb(51, 105, 146);
    assert!(
        output
            .shapes
            .iter()
            .any(|clipped| has_fill(&clipped.shape, headerline)),
        "focusing a tab must paint the focused headerline"
    );
}

#[test]
fn focused_tab_headerline_is_not_covered_by_the_split_separator() {
    use kairos_editor_docking::{split::Fraction, surface::SurfaceIndex, tree::NodeIndex};

    let mut dock: DockState<Id> = DockState::new();
    let [left, right] =
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), Fraction::new(0.5), vec![Id(2)]);
    dock.main_surface_mut()[left].append_tab(Id(1));
    dock.set_focused_node_and_surface((SurfaceIndex::main(), right));

    let mut viewer = Viewer { drawn: Vec::new() };
    let ctx = Context::default();
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
    let output = ctx.run_ui(input, |ui| {
        Docker::new("dock", &mut dock).show(ui, &mut viewer);
    });

    #[derive(Clone, Copy)]
    struct PaintedRect {
        rect: Rect,
        fill: Color32,
    }
    fn flatten(shape: &Shape, out: &mut Vec<PaintedRect>) {
        match shape {
            Shape::Rect(rect) => out.push(PaintedRect {
                rect: rect.rect,
                fill: rect.fill,
            }),
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| flatten(shape, out)),
            _ => {}
        }
    }
    let mut painted = Vec::new();
    for clipped in &output.shapes {
        flatten(&clipped.shape, &mut painted);
    }

    let headerline_color = Color32::from_rgb(51, 105, 146);
    let index = painted
        .iter()
        .position(|painted| painted.fill == headerline_color)
        .expect("the focused tab draws the blue headerline");
    let headerline = painted[index].rect;

    // The split separator is painted after the leaves. It must lie beside the
    // tab bar, not on top of the headerline (this regressed to a fully covered
    // line when the right/bottom child rect overlapped the separator).
    for x in [0.25, 0.5, 0.9] {
        let point = Pos2::new(
            headerline.min.x + headerline.width() * x,
            headerline.center().y,
        );
        for later in &painted[index + 1..] {
            assert!(
                later.fill.a() == 0 || !later.rect.contains(point),
                "paint over the focused headerline at {point:?}: {:?} fill {:?}",
                later.rect,
                later.fill
            );
        }
    }
}
