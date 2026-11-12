//! Headless compound tests: one-frame render coverage for every compound plus
//! synthetic click tests proving that Output data (clicked ids) is produced by
//! real pointer interaction -- which is what feature components map to Actions.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Ui, pos2};
use ui_widgets_prototype::testing::Harness;
use ui_widgets_prototype::{
    Breadcrumb, ConfirmDialog, ContextMenu, ContextMenuItem, EmptyState, Expander, FormSection,
    IconId, ListItem, PanelChrome, PropertyRow, SearchBox, SliderRow, StatusBar, Toast, ToastTone,
    Toolbar, ToolbarItem, ToolbarOutput, TreeOutput, TreeRow, TreeView,
};

fn themed_harness() -> Harness {
    let harness = Harness::default();
    ui_widgets_prototype::Theme::dark().install(&harness.ctx);
    harness
}

#[test]
fn all_compounds_render_without_panic() {
    let mut harness = themed_harness();
    harness.frame(Vec::new(), |ui| {
        let mut chrome = PanelChrome::new("Project")
            .closable(true)
            .toolbar(Toolbar::new(vec![
                ToolbarItem::button("new").icon(IconId::Plus).tooltip("New"),
                ToolbarItem::toggle("select", true)
                    .icon(IconId::Dot)
                    .tooltip("Select tool"),
            ]));
        let mut search_text = String::new();
        chrome.show(ui, |ui| {
            let mut search = SearchBox::new("Search assets");
            search.show(ui, &mut search_text);

            let mut crumbs = Breadcrumb::new(vec!["assets", "models", "hero.scene"]);
            crumbs.show(ui);

            let mut tree = TreeView::new("smoke").rows(vec![
                TreeRow::new(1, "assets")
                    .expander(Expander::Expanded)
                    .icon(IconId::FolderOpen),
                TreeRow::new(2, "models")
                    .indent(1)
                    .expander(Expander::Collapsed)
                    .icon(IconId::Folder),
                TreeRow::new(3, "hero.scene")
                    .indent(2)
                    .icon(IconId::Scene)
                    .selected(true)
                    .suffix("1.2 KB"),
                TreeRow::new(4, "locked.material").indent(2).disabled(true),
            ]);
            tree.show(ui);

            let mut list_item = ListItem::new("crate.material")
                .icon(IconId::Material)
                .suffix("meta");
            list_item.show(ui);

            let mut row = PropertyRow::new("Roughness").reset_enabled(true);
            let mut value = 0.5f32;
            row.show(ui, |ui| ui.add(SliderRow::new("", &mut value, 0.0..=1.0)));

            let mut section = FormSection::new("Properties");
            section.show(ui, |ui| {
                ui.label("section body");
            });

            let mut status = StatusBar::new("7 assets", "selected: hero.scene");
            status.show(ui);
        });

        let mut empty = EmptyState::new("No document open", "Select an asset").icon(IconId::File);
        empty.show(ui);

        let mut dialog = ConfirmDialog::new("Discard changes?", "Unsaved changes will be lost.")
            .danger(true)
            .confirm_label("Discard");
        dialog.show(ui);

        let mut menu = ContextMenu::new("smoke-menu", pos2(40.0, 60.0)).items(vec![
            ContextMenuItem::new("rename", "Rename").icon(IconId::File),
            ContextMenuItem::new("delete", "Delete").danger(true),
        ]);
        menu.show(ui);

        let mut toast = Toast::new("saved", "Document saved")
            .tone(ToastTone::Success)
            .dismissible(true);
        toast.show(ui);
    });
}

#[test]
fn tree_click_reports_row_id() {
    let mut harness = themed_harness();
    let output = Rc::new(RefCell::new(TreeOutput::default()));
    let mut draw = {
        let output = output.clone();
        move |ui: &mut Ui| {
            let mut tree = TreeView::new("click-test").rows(vec![
                TreeRow::new(1, "one"),
                TreeRow::new(2, "two").indent(1),
                TreeRow::new(3, "three").selected(true),
            ]);
            *output.borrow_mut() = tree.show(ui);
        }
    };
    harness.frame(Vec::new(), &mut draw);
    let row_rect = output
        .borrow()
        .row_rects
        .iter()
        .find(|(id, _)| *id == 2)
        .map(|(_, rect)| *rect)
        .expect("row 2 must have a hit rect");
    harness.click(row_rect.center(), &mut draw);
    let output = output.borrow().clone();
    assert_eq!(
        output.clicked,
        Some(2),
        "clicked row id should reach the feature component"
    );
    assert_eq!(output.double_clicked, None);
}

#[test]
fn tree_expander_click_is_a_view_toggle_not_a_row_click() {
    let mut harness = themed_harness();
    let output = Rc::new(RefCell::new(TreeOutput::default()));
    let mut draw = {
        let output = output.clone();
        move |ui: &mut Ui| {
            let mut tree = TreeView::new("expander-test").rows(vec![
                TreeRow::new(1, "folder").expander(Expander::Expanded),
                TreeRow::new(2, "child").indent(1),
            ]);
            *output.borrow_mut() = tree.show(ui);
        }
    };
    harness.frame(Vec::new(), &mut draw);
    let row_rect = output.borrow().row_rects[0].1;
    // Chevron sits at the left edge of the row: click near the left padding.
    let expander_pos = pos2(row_rect.left() + 10.0, row_rect.center().y);
    harness.click(expander_pos, &mut draw);
    let output = output.borrow().clone();
    assert_eq!(output.expander_toggled, Some(1));
    assert_eq!(
        output.clicked, None,
        "expander toggle must not be reported as a row selection"
    );
}

#[test]
fn toolbar_click_reports_item_id() {
    let mut harness = themed_harness();
    let output = Rc::new(RefCell::new(ToolbarOutput::default()));
    let mut draw = {
        let output = output.clone();
        move |ui: &mut Ui| {
            let mut toolbar = Toolbar::new(vec![
                ToolbarItem::button("save")
                    .icon(IconId::Save)
                    .tooltip("Save"),
                ToolbarItem::button("revert")
                    .icon(IconId::Revert)
                    .tooltip("Revert"),
            ]);
            *output.borrow_mut() = toolbar.show(ui);
        }
    };
    harness.frame(Vec::new(), &mut draw);
    let rect = output
        .borrow()
        .rects
        .iter()
        .find(|(id, _)| *id == "save")
        .map(|(_, rect)| *rect)
        .expect("save rect");
    harness.click(rect.center(), &mut draw);
    assert_eq!(output.borrow().clicked, Some("save"));
}

#[test]
fn confirm_dialog_buttons_are_clickable() {
    let mut harness = themed_harness();
    let output = Rc::new(RefCell::new(
        ui_widgets_prototype::ConfirmDialogOutput::default(),
    ));
    let mut draw = {
        let output = output.clone();
        move |ui: &mut Ui| {
            let mut dialog =
                ConfirmDialog::new("Discard changes?", "Unsaved changes will be lost.");
            *output.borrow_mut() = dialog.show(ui);
        }
    };
    // Windows/Areas anchored to the screen need one settle frame: the first
    // pass draws with a zero-size assumption, the anchor is applied from the
    // measured size on the next pass. This is another headless-test gotcha
    // (recorded in the README).
    harness.frame(Vec::new(), &mut draw);
    harness.frame(Vec::new(), &mut draw);
    let confirm_rect = output.borrow().confirm_rect;
    assert!(
        confirm_rect.is_positive(),
        "dialog must lay out its confirm button"
    );
    harness.click(confirm_rect.center(), &mut draw);
    let output = output.borrow().clone();
    assert!(output.confirm, "confirm click should be reported as data");
    assert!(!output.cancel);
}
