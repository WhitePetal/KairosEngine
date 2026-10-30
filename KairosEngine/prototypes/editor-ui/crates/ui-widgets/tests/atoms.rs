//! Headless atom tests: every atom renders a frame; the interactive ones are
//! also driven with synthetic pointer/keyboard events (see testing.rs for the
//! 3-pass click recipe this harness encodes).

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Color32, Rect, Ui};
use ui_widgets_prototype::testing::Harness;
use ui_widgets_prototype::{
    Badge, BadgeTone, Button, ButtonVariant, Checkbox, ColorSwatch, ComboSelect, GlyphIcons,
    Heading, IconButton, IconId, IconProvider, Label, LabelTone, NumberRow, Separator, SliderRow,
    Spinner, TextInput, Theme,
};

fn themed_harness() -> Harness {
    let harness = Harness::default();
    Theme::dark().install(&harness.ctx);
    harness
}

#[test]
fn all_atoms_render_without_panic() {
    let mut harness = themed_harness();
    let mut selected = 0usize;
    let mut changed = false;
    harness.frame(Vec::new(), |ui| {
        ui.vertical(|ui| {
            ui.add(Button::primary("Apply"));
            ui.add(Button::new("Secondary").icon(IconId::Save));
            ui.add(Button::new("Ghost").variant(ButtonVariant::Ghost).small());
            ui.add(Button::danger("Delete"));
            ui.add(Button::new("Disabled").enabled(false));
            ui.add(IconButton::new(IconId::Settings).tooltip("Settings").active(true));
            ui.add(IconButton::new(IconId::Trash).enabled(false));
            changed = ui.add(Checkbox::new(true, "Metallic")).clicked();
            let mut text = "crate.material".to_owned();
            ui.add(TextInput::new(&mut text).hint("name").mono(true));
            let mut roughness = 0.4f32;
            ui.add(SliderRow::new("Roughness", &mut roughness, 0.0..=1.0));
            let mut count = 3.0f64;
            ui.add(NumberRow::new("Count", &mut count).range(0.0..=10.0).suffix("px"));
            let options = ["Low", "Medium", "High"];
            ui.add(ComboSelect::new(&options, &mut selected));
            ui.add(ColorSwatch::new(Color32::from_rgb(200, 120, 60)));
            ui.add(Badge::new("unsaved").tone(BadgeTone::Error));
            ui.add(Badge::new("ok").tone(BadgeTone::Success));
            ui.add(Spinner::new().size(14.0));
            ui.add(Separator);
            ui.add(Heading::new("Properties"));
            ui.add(Label::new("asset path").muted().mono(true));
            ui.add(Label::new("failure").tone(LabelTone::Error));
            ui.add(Label::new("success").tone(LabelTone::Success));
        });
    });
    assert!(harness.passes >= 1);
    assert!(!changed);
}

#[test]
fn primary_button_click_reports_clicked() {
    let mut harness = themed_harness();
    let rect = Rc::new(RefCell::new(Rect::NOTHING));
    {
        let rect = rect.clone();
        harness.frame(Vec::new(), move |ui| {
            *rect.borrow_mut() = ui.add(Button::primary("Save")).rect;
        });
    }
    let center = rect.borrow().center();
    let clicked = Rc::new(RefCell::new(false));
    {
        let clicked = clicked.clone();
        harness.click(center, move |ui| {
            *clicked.borrow_mut() = ui.add(Button::primary("Save")).clicked();
        });
    }
    assert!(*clicked.borrow(), "click should be reported in the release pass (passes={})", harness.passes);
}

#[test]
fn disabled_button_ignores_click() {
    let mut harness = themed_harness();
    let rect = Rc::new(RefCell::new(Rect::NOTHING));
    {
        let rect = rect.clone();
        harness.frame(Vec::new(), move |ui| {
            *rect.borrow_mut() = ui.add(Button::primary("Save").enabled(false)).rect;
        });
    }
    let center = rect.borrow().center();
    let clicked = Rc::new(RefCell::new(false));
    {
        let clicked = clicked.clone();
        harness.click(center, move |ui| {
            *clicked.borrow_mut() = ui.add(Button::primary("Save").enabled(false)).clicked();
        });
    }
    assert!(!*clicked.borrow(), "disabled button must not report clicks");
}

#[test]
fn checkbox_click_is_reported() {
    let mut harness = themed_harness();
    let rect = Rc::new(RefCell::new(Rect::NOTHING));
    {
        let rect = rect.clone();
        harness.frame(Vec::new(), move |ui| {
            *rect.borrow_mut() = ui.add(Checkbox::new(true, "Metallic")).rect;
        });
    }
    let center = rect.borrow().center();
    let clicked = Rc::new(RefCell::new(false));
    {
        let clicked = clicked.clone();
        harness.click(center, move |ui| {
            *clicked.borrow_mut() = ui.add(Checkbox::new(true, "Metallic")).clicked();
        });
    }
    assert!(*clicked.borrow(), "checkbox should report a click so the caller can toggle the domain value");
}

#[test]
fn text_input_accepts_synthetic_typing() {
    let mut harness = themed_harness();
    let buffer = Rc::new(RefCell::new(String::new()));
    let rect = Rc::new(RefCell::new(Rect::NOTHING));
    let changed = Rc::new(RefCell::new(false));

    let mut draw = {
        let buffer = buffer.clone();
        let rect = rect.clone();
        let changed = changed.clone();
        move |ui: &mut Ui| {
            let mut text = buffer.borrow().clone();
            let response = ui.add(TextInput::new(&mut text).hint("search"));
            *buffer.borrow_mut() = text;
            *rect.borrow_mut() = response.rect;
            *changed.borrow_mut() = response.changed();
        }
    };

    harness.frame(Vec::new(), &mut draw);
    let center = rect.borrow().center();
    // Click to focus, then type into the focused TextEdit.
    harness.click(center, &mut draw);
    harness.type_text("hero", &mut draw);

    assert_eq!(&*buffer.borrow(), "hero");
    assert!(*changed.borrow(), "TextInput response should report changed after typing");
}

struct TestIcons;

impl IconProvider for TestIcons {
    fn glyph(&self, _id: IconId) -> &'static str {
        "X"
    }
}

static TEST_ICONS: TestIcons = TestIcons;

#[test]
fn icon_provider_is_swappable() {
    let custom = Theme { icons: &TEST_ICONS, ..Theme::dark() };
    assert_eq!(custom.icon(IconId::Folder), "X");
    let default = Theme::dark();
    assert_eq!(default.icon(IconId::Folder), GlyphIcons.glyph(IconId::Folder));
}

#[test]
fn theme_is_installed_into_context_data() {
    let mut harness = Harness::default();
    let custom = Theme { accent: Color32::from_rgb(255, 0, 0), ..Theme::dark() };
    custom.install(&harness.ctx);

    let seen = Rc::new(RefCell::new(None));
    {
        let seen = seen.clone();
        harness.frame(Vec::new(), move |ui| {
            *seen.borrow_mut() = Some(Theme::get(ui));
        });
    }
    let seen = seen.borrow().expect("frame should read the installed theme");
    assert_eq!(seen.accent, Color32::from_rgb(255, 0, 0));
}

#[test]
fn theme_falls_back_to_dark_when_not_installed() {
    let mut harness = Harness::default();
    let seen = Rc::new(RefCell::new(None));
    {
        let seen = seen.clone();
        harness.frame(Vec::new(), move |ui| {
            *seen.borrow_mut() = Some(Theme::get(ui));
        });
    }
    let seen = seen.borrow().expect("frame ran");
    assert_eq!(seen.accent, Theme::dark().accent);
}
