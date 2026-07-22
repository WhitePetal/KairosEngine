use crate::kairos_editor::KairosEngine;
use crate::kairos_editor::ui::Message;
use crate::kairos_test_harness::{
    assertions::{self, CrashTracker},
    input_injector,
    types::{StepResult, TestStep},
};

/// Dispatch a test step's `call` action to the appropriate engine function.
pub fn dispatch_call(step: &TestStep, engine: &mut KairosEngine) -> StepResult {
    let target = step.target.as_deref().unwrap_or("");

    match target {
        "system.ping" => StepResult::ok(),
        "system.query_widget" => {
            let args = match step.args.as_ref() {
                Some(a) => a,
                None => return StepResult::err("query_widget requires args with 'id' field"),
            };
            let id = match args.get("id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return StepResult::err("query_widget requires 'id' argument"),
            };
            match engine.widget_rect(id) {
                Some(rect) => {
                    let json = format!(
                        r#"{{"x_min":{:.1},"y_min":{:.1},"x_max":{:.1},"y_max":{:.1}}}"#,
                        rect.min.x, rect.min.y, rect.max.x, rect.max.y
                    );
                    StepResult {
                        ok: true,
                        message: json,
                        wait_frames: 0,
                    }
                }
                None => StepResult::err(format!("widget not found: '{id}'")),
            }
        }
        "ui.open_inspector" => {
            engine.push_ui_message(Message::OpenInspectorTab);
            StepResult::ok()
        }
        "project.select_asset" => {
            let args = match step.args.as_ref() {
                Some(a) => a,
                None => return StepResult::err("select_asset requires args with 'path' field"),
            };
            let path_str = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return StepResult::err("select_asset requires 'path' argument"),
            };
            let path = std::path::Path::new(path_str);
            let node_idx = {
                use crate::kairos_editor::ui::project_window::ProjectWindow;
                let ui_ctx = engine.ui_context_mut();
                let project = match ui_ctx.get_window_mut::<ProjectWindow>() {
                    Some(p) => p,
                    None => return StepResult::err("ProjectWindow is not open"),
                };
                match project.find_node_by_path(path) {
                    Some(idx) => idx,
                    None => return StepResult::err(format!("asset not found: {}", path.display())),
                }
            };
            engine.push_ui_message(Message::SelectProjectNode(Some(node_idx)));
            StepResult::ok()
        }
        "system.wait_frames" => {
            let args = match step.args.as_ref() {
                Some(a) => a,
                None => return StepResult::err("wait_frames requires args with 'count' field"),
            };
            let count: usize = match args.get("count").and_then(|v| v.as_integer()) {
                Some(n) if n > 0 => n as usize,
                _ => return StepResult::err("wait_frames requires 'count' > 0"),
            };
            StepResult::with_wait_frames(count)
        }
        "ui.focus_widget" => {
            let args = match step.args.as_ref() {
                Some(a) => a,
                None => return StepResult::err("focus_widget requires args with 'id' field"),
            };
            let id = match args.get("id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return StepResult::err("focus_widget requires 'id' argument"),
            };
            match engine.widget_egui_id(id) {
                Some(egui_id) => {
                    engine.request_focus(egui_id);
                    StepResult::ok()
                }
                None => StepResult::err(format!(
                    "widget egui ID not found: '{id}'. Widget may not have been rendered yet."
                )),
            }
        }
        "" => StepResult::err("call step missing 'target' field"),
        other => StepResult::err(format!("unknown call target: '{other}'")),
    }
}

/// Dispatch a test step's `assert` action to the appropriate assertion function.
pub fn dispatch_assert(
    step: &TestStep,
    engine: &mut KairosEngine,
    crash_tracker: &mut CrashTracker,
) -> StepResult {
    let args = step.args.as_ref();

    let target = step.target.as_deref().unwrap_or("");
    match target {
        "no_crash" => assertions::assert_no_crash(crash_tracker),
        "resource_exists" => {
            let args = match args {
                Some(a) => a,
                None => return StepResult::err("resource_exists requires args"),
            };
            assertions::assert_resource_exists(args)
        }
        "log_contains" => {
            let args = match args {
                Some(a) => a,
                None => return StepResult::err("log_contains requires args"),
            };
            let log = engine.log_mut();
            assertions::assert_log_contains(log, args)
        }
        "ecs_query" => {
            let args = match args {
                Some(a) => a,
                None => return StepResult::err("ecs_query requires args"),
            };
            let world = &engine.engine_mut().world;
            assertions::assert_ecs_query(world, args)
        }
        "wgpu_valid" => {
            let default_args = toml::Value::Table(toml::Table::new());
            let args = args.unwrap_or(&default_args);
            assertions::assert_wgpu_valid(args)
        }
        "toml_value_equals" => {
            let args = match args {
                Some(a) => a,
                None => return StepResult::err("toml_value_equals requires args"),
            };
            assertions::assert_toml_value_equals(args)
        }
        "" => StepResult::err("assert step missing 'target' field"),
        other => StepResult::err(format!("unknown assertion: '{other}'")),
    }
}

/// Dispatch a test step's `input` action to inject keyboard/mouse events.
pub fn dispatch_input(step: &TestStep, engine: &mut KairosEngine) -> StepResult {
    let args = match step.args.as_ref() {
        Some(a) => a,
        None => return StepResult::err("input step requires args"),
    };

    // Handle click_widget: query widget rect and click at center (or center + offset)
    if args.get("event").and_then(|v| v.as_str()) == Some("click_widget") {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return StepResult::err("click_widget requires 'id' argument"),
        };
        let dx = args.get("dx").and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
        let dy = args.get("dy").and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
        let rect = match engine.widget_rect(id) {
            Some(r) => r,
            None => return StepResult::err(format!("widget not found: '{id}'")),
        };
        let pos = egui::pos2(
            (rect.min.x + rect.max.x) / 2.0 + dx,
            (rect.min.y + rect.max.y) / 2.0 + dy,
        );
        // Inject events into egui's input pipeline so the click actually
        // reaches egui widgets (not just the game InputEngine).
        engine.push_egui_event(egui::Event::PointerMoved(pos));
        engine.push_egui_event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
        engine.push_egui_event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
        return StepResult::ok();
    }

    // Handle drag_drop: simulate dragging from source widget to target widget.
    if args.get("event").and_then(|v| v.as_str()) == Some("drag_drop") {
        let source_id = match args.get("source_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return StepResult::err("drag_drop requires 'source_id' argument"),
        };
        let target_id = match args.get("target_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return StepResult::err("drag_drop requires 'target_id' argument"),
        };
        let source_rect = match engine.widget_rect(source_id) {
            Some(r) => r,
            None => return StepResult::err(format!("source widget not found: '{source_id}'")),
        };
        let target_rect = match engine.widget_rect(target_id) {
            Some(r) => r,
            None => return StepResult::err(format!("target widget not found: '{target_id}'")),
        };
        let source_center = egui::pos2(
            (source_rect.min.x + source_rect.max.x) / 2.0,
            (source_rect.min.y + source_rect.max.y) / 2.0,
        );
        let target_center = egui::pos2(
            (target_rect.min.x + target_rect.max.x) / 2.0,
            (target_rect.min.y + target_rect.max.y) / 2.0,
        );

        // Clear any existing drag payload
        // Inject egui events to simulate drag: move to source → press → move to target → release
        engine.push_egui_event(egui::Event::PointerMoved(source_center));
        engine.push_egui_event(egui::Event::PointerButton {
            pos: source_center,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
        engine.push_egui_event(egui::Event::PointerMoved(target_center));
        engine.push_egui_event(egui::Event::PointerButton {
            pos: target_center,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
        return StepResult::ok();
    }

    // Handle keyboard events: inject into egui for UI navigation (arrow keys, enter, etc.)
    if args.get("device").and_then(|v| v.as_str()) == Some("keyboard") {
        let key_str = match args.get("key").and_then(|v| v.as_str()) {
            Some(k) => k,
            None => return StepResult::err("keyboard input requires 'key' argument"),
        };
        let pressed = match args.get("event").and_then(|v| v.as_str()) {
            Some("press") => true,
            Some("release") => false,
            _ => return StepResult::err("keyboard requires 'event' = 'press' or 'release'"),
        };
        let egui_key = match key_str {
            "ArrowUp" => egui::Key::ArrowUp,
            "ArrowDown" => egui::Key::ArrowDown,
            "ArrowLeft" => egui::Key::ArrowLeft,
            "ArrowRight" => egui::Key::ArrowRight,
            "Enter" => egui::Key::Enter,
            "Escape" => egui::Key::Escape,
            "Tab" => egui::Key::Tab,
            "Home" => egui::Key::Home,
            "End" => egui::Key::End,
            _ => {
                // Fall through to InputEngine for game keys (W, A, S, D, etc.)
                let input_engine = &mut engine.engine_mut().input_engine;
                return input_injector::inject(args, input_engine);
            }
        };
        engine.push_egui_event(egui::Event::Key {
            key: egui_key,
            repeat: false,
            pressed,
            modifiers: egui::Modifiers::default(),
            physical_key: None,
        });
        return StepResult::ok();
    }

    let input_engine = &mut engine.engine_mut().input_engine;
    input_injector::inject(args, input_engine)
}
