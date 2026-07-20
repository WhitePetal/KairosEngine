use crate::kairos_editor::KairosEngine;
use crate::kairos_test_harness::types::{StepResult, TestStep};

/// Dispatch a test step's `call` action to the appropriate engine function.
///
/// Returns `StepResult::ok()` if the call succeeded, or `StepResult::err(...)`
/// if the target is unknown or the call failed.
pub fn dispatch_call(step: &TestStep, _engine: &mut KairosEngine) -> StepResult {
    let target = step.target.as_deref().unwrap_or("");

    match target {
        "system.ping" => StepResult::ok(),
        "" => StepResult::err("call step missing 'target' field"),
        other => StepResult::err(format!("unknown call target: '{other}'")),
        }
    }
