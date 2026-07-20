use crate::kairos_editor::KairosEngine;
use crate::kairos_test_harness::{
    assertions::{self, CrashTracker},
    input_injector,
    types::{StepResult, TestStep},
};

/// Dispatch a test step's `call` action to the appropriate engine function.
pub fn dispatch_call(step: &TestStep, _engine: &mut KairosEngine) -> StepResult {
    let target = step.target.as_deref().unwrap_or("");

    match target {
        "system.ping" => StepResult::ok(),
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
