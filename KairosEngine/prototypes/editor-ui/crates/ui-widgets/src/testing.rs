//! Minimal headless test harness for egui 0.35 (egui_kittest is not cached).
//!
//! ## Why this exists
//! egui_kittest is not available in the offline cargo cache, so the prototype
//! ships a ~90-line harness built directly on egui::Context::run_ui with
//! synthetic RawInput pointer/keyboard events.
//!
//! ## The click recipe (discovered empirically; encoded in Harness::click)
//! egui resolves pointer interaction at the START of a pass using the widget
//! rects registered by the *previous* pass (see Context::read_response docs:
//! "widget interaction happens at the start of the pass, using the widget rects
//! from the previous pass"). A synthetic click therefore needs three passes:
//!
//! 1. draw pass            -- records widget rects (this is where the test
//!                            reads a widget's rect from its Output),
//! 2. PointerMoved(pos)    -- hover is established against pass-1 rects,
//! 3. press + release      -- click is reported by the widget in this pass.
//!
//! Press and release may share one pass. Press+release in a single pass WITHOUT
//! a preceding hover pass does NOT register a click. This was verified with a
//! throwaway probe test before the harness was written.
//!
//! ## Multi-pass caveat
//! Context::run_ui may invoke the draw closure twice (egui default
//! max_passes = 2, hit when a layout calls request_discard). Test-side Output
//! capture must therefore OVERWRITE a slot -- *out.borrow_mut() = Some(...) --
//! never push into a Vec, otherwise every action is collected twice. This is
//! the test-side counterpart of the design rule "UI pass must be side-effect
//! free" (docs/plan/editor-ui-architecture/README.md 3.3.2).

use egui::{
    Context, Event, FullOutput, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, vec2,
};

/// Headless egui host: owns one Context and feeds it synthetic frames.
pub struct Harness {
    pub ctx: Context,
    pub screen: Rect,
    time: f64,
    /// Number of passes actually executed (including multi-pass re-runs).
    pub passes: usize,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new(800.0, 560.0)
    }
}

impl Harness {
    pub fn new(width: f32, height: f32) -> Self {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.0);
        Self {
            ctx,
            screen: Rect::from_min_size(Pos2::ZERO, vec2(width, height)),
            time: 0.0,
            passes: 0,
        }
    }

    /// Run one pass with the given events. The closure is the "UI pass"; it may
    /// be called more than once (multi-pass), so it must be side-effect free.
    pub fn frame(&mut self, events: Vec<Event>, mut draw: impl FnMut(&mut Ui)) -> FullOutput {
        self.time += 1.0 / 60.0;
        self.passes += 1;
        let raw = RawInput {
            screen_rect: Some(self.screen),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        self.ctx.run_ui(raw, |ui| draw(ui))
    }

    /// Synthetic primary-button click at pos. Runs the 3-pass recipe above.
    pub fn click(&mut self, pos: Pos2, draw: impl FnMut(&mut Ui)) {
        self.click_button(pos, PointerButton::Primary, draw);
    }

    /// Synthetic secondary (right) click at pos.
    pub fn secondary_click(&mut self, pos: Pos2, draw: impl FnMut(&mut Ui)) {
        self.click_button(pos, PointerButton::Secondary, draw);
    }

    pub fn click_button(&mut self, pos: Pos2, button: PointerButton, mut draw: impl FnMut(&mut Ui)) {
        self.frame(Vec::new(), &mut draw);
        self.frame(vec![Event::PointerMoved(pos)], &mut draw);
        self.frame(
            vec![
                Event::PointerButton { pos, button, pressed: true, modifiers: Modifiers::NONE },
                Event::PointerButton { pos, button, pressed: false, modifiers: Modifiers::NONE },
            ],
            &mut draw,
        );
    }

    /// Synthetic primary-button double click: two full click sequences at the
    /// same position. egui's double-click window (~0.3s) is wider than the two
    /// sequences take at the harness time step, so `response.double_clicked()`
    /// is reported by the second release pass.
    pub fn double_click(&mut self, pos: Pos2, draw: impl FnMut(&mut Ui)) {
        let mut draw = draw;
        self.click(pos, &mut draw);
        self.click(pos, &mut draw);
    }

    /// Synthetic drag: press at `from`, move to `to` while held, release.
    /// Sliders/scrollbars use `Sense::drag`, which does not react to a plain
    /// click: egui needs press-then-move before the value changes.
    pub fn drag(&mut self, from: Pos2, to: Pos2, mut draw: impl FnMut(&mut Ui)) {
        self.frame(Vec::new(), &mut draw);
        self.frame(vec![Event::PointerMoved(from)], &mut draw);
        let press = Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![press], &mut draw);
        let steps = 4;
        for step in 1..=steps {
            let pos = from + (to - from) * (step as f32 / steps as f32);
            self.frame(vec![Event::PointerMoved(pos)], &mut draw);
        }
        let release = Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![release], &mut draw);
    }

    /// Send a text-input event (the target widget must already have focus).
    pub fn type_text(&mut self, text: &str, draw: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::Text(text.to_owned())], draw);
    }

    /// Press+release a key.
    pub fn press_key(&mut self, key: Key, draw: impl FnMut(&mut Ui)) {
        self.frame(
            vec![
                Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE },
                Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE },
            ],
            draw,
        );
    }
}
