use crate::kairos_editor::KairosEngine;
use crate::kairos_editor::ui::inspector::texture::TextureInspector;
use crate::kairos_editor::ui::inspector_window::InspectorWindow;
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
                    StepResult { ok: true, message: json }
                }
                None => StepResult::err(format!("widget not found: '{id}'")),
            }
        }
        "texture_inspector.set_format" => {
            let args = match step.args.as_ref() {
                Some(a) => a,
                None => return StepResult::err("set_format requires args with 'format' field"),
            };
            let format_str = match args.get("format").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return StepResult::err("set_format requires 'format' argument"),
            };
            let format_toml = format!("\"{}\"", format_str);
            let format: crate::graphics::texture::format::TextureFormat = match toml::from_str(&format_toml) {
                Ok(f) => f,
                Err(_) => return StepResult::err(format!("unknown texture format: '{}'", format_str)),
            };
            match get_texture_inspector(engine) {
                Some(inspector) => match inspector.set_format(format) {
                    Ok(()) => StepResult::ok(),
                    Err(msg) => StepResult::err(msg),
                },
                None => StepResult::err("TextureInspector is not active"),
            }
        }
        "texture_inspector.apply" => {
            match get_texture_inspector(engine) {
                Some(inspector) => {
                    inspector.apply();
                    StepResult::ok()
                }
                None => StepResult::err("TextureInspector is not active"),
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

fn get_texture_inspector(engine: &mut KairosEngine) -> Option<&mut TextureInspector> {
    engine
        .ui_context_mut()
        .get_window_mut::<InspectorWindow>()
        .and_then(|w| w.get_inspector_mut::<TextureInspector>())
}

/// Dispatch a test step's `input` action to inject keyboard/mouse events.
pub fn dispatch_input(step: &TestStep, engine: &mut KairosEngine) -> StepResult {
    let args = match step.args.as_ref() {
        Some(a) => a,
        None => return StepResult::err("input step requires args"),
    };
    let input_engine = &mut engine.engine_mut().input_engine;
    input_injector::inject(args, input_engine)
}
