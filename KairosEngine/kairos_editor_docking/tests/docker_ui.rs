use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2, WidgetText};
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
