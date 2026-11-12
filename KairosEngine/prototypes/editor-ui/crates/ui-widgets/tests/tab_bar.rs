//! Headless tests for the pure TabBar compound: one-frame render coverage plus
//! synthetic pointer tests proving that activation and close clicks come back as
//! plain indices (which the panel host maps to core Actions).

use std::cell::RefCell;
use std::rc::Rc;

use egui::Ui;
use ui_widgets_prototype::testing::Harness;
use ui_widgets_prototype::{TabBar, TabBarItem, TabBarOutput, Theme};

fn themed_harness() -> Harness {
    let harness = Harness::default();
    Theme::dark().install(&harness.ctx);
    harness
}

fn items() -> Vec<TabBarItem> {
    vec![
        TabBarItem::new("Project"),
        TabBarItem::new("Console"),
        TabBarItem::new("Inspector"),
    ]
}

/// Render a tab bar with the given items/active index and return the captured
/// output plus the clickable draw closure.
fn tab_bar_draw(
    items: Vec<TabBarItem>,
    active: Option<usize>,
) -> (Rc<RefCell<TabBarOutput>>, impl FnMut(&mut Ui)) {
    let output = Rc::new(RefCell::new(TabBarOutput::default()));
    let draw = {
        let output = output.clone();
        move |ui: &mut Ui| {
            let mut bar = TabBar::new(items.clone());
            if let Some(active) = active {
                bar = bar.active(active);
            }
            *output.borrow_mut() = bar.show(ui);
        }
    };
    (output, draw)
}

#[test]
fn empty_tab_bar_renders_without_rects() {
    let mut harness = themed_harness();
    let output = Rc::new(RefCell::new(TabBarOutput::default()));
    {
        let output = output.clone();
        harness.frame(Vec::new(), move |ui: &mut Ui| {
            *output.borrow_mut() = TabBar::new(Vec::new()).show(ui);
        });
    }
    assert!(output.borrow().tab_rects.is_empty());
    assert!(output.borrow().close_rects.is_empty());
}

#[test]
fn tab_bar_reports_one_rect_per_tab_and_one_close_rect_per_closable_tab() {
    let mut harness = themed_harness();
    let (output, mut draw) = tab_bar_draw(items(), Some(1));
    harness.frame(Vec::new(), &mut draw);

    let output = output.borrow();
    assert_eq!(output.tab_rects.len(), 3);
    assert_eq!(output.close_rects.len(), 3);
    assert_eq!(output.activated, None);
    assert_eq!(output.close_clicked, None);
    // Tabs are laid out left to right, in item order.
    let xs: Vec<f32> = output
        .tab_rects
        .iter()
        .map(|(_, rect)| rect.left())
        .collect();
    assert!(
        xs.windows(2).all(|pair| pair[0] < pair[1]),
        "tabs must advance left to right"
    );
}

#[test]
fn non_closable_tab_has_no_close_rect() {
    let mut harness = themed_harness();
    let (output, mut draw) = tab_bar_draw(
        vec![
            TabBarItem::new("Pinned").closable(false),
            TabBarItem::new("Normal"),
        ],
        Some(0),
    );
    harness.frame(Vec::new(), &mut draw);

    let output = output.borrow();
    assert_eq!(output.tab_rects.len(), 2);
    assert_eq!(output.close_rects.len(), 1);
    assert_eq!(
        output.close_rects[0].0, 1,
        "only the closable tab reports a close rect"
    );
}

#[test]
fn tab_click_reports_activated_index() {
    let mut harness = themed_harness();
    let (output, mut draw) = tab_bar_draw(items(), Some(0));
    harness.frame(Vec::new(), &mut draw);

    let second = output.borrow().tab_rects[1].1;
    harness.click(second.center(), &mut draw);

    let output = output.borrow();
    assert_eq!(
        output.activated,
        Some(1),
        "clicking a tab reports its index"
    );
    assert_eq!(output.close_clicked, None);
}

#[test]
fn close_click_reports_close_index_and_never_activates() {
    let mut harness = themed_harness();
    let (output, mut draw) = tab_bar_draw(items(), Some(0));
    harness.frame(Vec::new(), &mut draw);

    let close = output.borrow().close_rects[2].1;
    harness.click(close.center(), &mut draw);

    let output = output.borrow();
    assert_eq!(
        output.close_clicked,
        Some(2),
        "clicking the close affordance closes that tab"
    );
    assert_eq!(
        output.activated, None,
        "a close click must not also activate the tab"
    );
}

#[test]
fn out_of_range_active_index_selects_nothing() {
    let mut harness = themed_harness();
    let (output, mut draw) = tab_bar_draw(items(), Some(99));
    harness.frame(Vec::new(), &mut draw);

    let output = output.borrow();
    assert_eq!(output.tab_rects.len(), 3);
    assert_eq!(
        output.activated, None,
        "a dead index must not crash or activate a tab"
    );
}
