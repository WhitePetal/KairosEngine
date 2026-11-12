use egui::{Color32, Context, Pos2, RawInput, Rect, Shape, Ui, Vec2, WidgetText};
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
