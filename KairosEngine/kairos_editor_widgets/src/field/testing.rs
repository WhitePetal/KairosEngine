//! Helpers the field widget tests share.

use egui::{Color32, Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, vec2};

use super::style::FieldStyle;
use crate::text::TextStyle;

pub(super) fn input() -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(240.0, 80.0))),
        ..Default::default()
    }
}

pub(super) fn style() -> FieldStyle {
    FieldStyle::new(TextStyle::sized(Color32::from_rgb(220, 220, 220), 12.0))
        .fills(
            Color32::from_rgb(0x28, 0x28, 0x28),
            Color32::from_rgb(0x3E, 0x3E, 0x3E),
        )
        .edges(
            Color32::from_rgb(0x20, 0x20, 0x20),
            Color32::from_rgb(0x5A, 0x5A, 0x5A),
            Color32::from_rgb(0x4A, 0x90, 0xD9),
        )
}

/// The value box: the right-hand column of a row.
pub(super) fn rect() -> Rect {
    Rect::from_min_size(egui::pos2(60.0, 0.0), vec2(120.0, 18.0))
}

/// The name column of the same row, left of the value box.
pub(super) fn label_rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, vec2(60.0, 18.0))
}

pub(super) fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    }
}

pub(super) fn enter() -> Event {
    Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::default(),
    }
}

pub(super) fn click(ctx: &Context, at: Pos2, mut draw: impl FnMut(&mut Ui)) {
    let mut press = input();
    press.events = vec![button(at, true)];
    let _ = ctx.run_ui(press, &mut draw);
    let mut release = input();
    release.events = vec![button(at, false)];
    let _ = ctx.run_ui(release, &mut draw);
}
