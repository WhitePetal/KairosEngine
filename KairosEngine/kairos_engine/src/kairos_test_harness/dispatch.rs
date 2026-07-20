use crate::graphics::texture::format::TextureFormat;
use crate::kairos_editor::KairosEngine;
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
            let format = match format_str {
                "R8Unorm" => TextureFormat::R8Unorm,
                "R8Snorm" => TextureFormat::R8Snorm,
                "R8Uint" => TextureFormat::R8Uint,
                "R8Sint" => TextureFormat::R8Sint,
                "Rgba8Unorm" => TextureFormat::Rgba8Unorm,
                "Rgba8UnormSrgb" => TextureFormat::Rgba8UnormSrgb,
                "BC7" => TextureFormat::Bc7RgbaUnorm,
                "BC7Srgb" => TextureFormat::Bc7RgbaUnormSrgb,
                other => return StepResult::err(format!("unknown texture format: '{other}'")),
            };
            match engine.texture_inspector_set_format(format) {
                Ok(()) => StepResult::ok(),
                Err(e) => StepResult::err(e),
            }
        }
        "texture_inspector.apply" => {
            match engine.texture_inspector_apply() {
                Ok(()) => StepResult::ok(),
                Err(e) => StepResult::err(e),
            }
        }
        "ui.open_inspector" => {
            engine.open_inspector();
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
            for _ in 0..count {
                engine.engine_mut().assets_server.handle();
            }
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
            match engine.select_asset(path) {
                Ok(()) => StepResult::ok(),
                Err(e) => StepResult::err(e),
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
    let input_engine = &mut engine.engine_mut().input_engine;
    input_injector::inject(args, input_engine)
}
