use crate::{ecs::world::World, log::Log};
use crate::kairos_test_harness::types::StepResult;

/// Tracks whether the engine has crashed since the last `no_crash` check.
#[derive(Debug, Default)]
pub struct CrashTracker {
    /// Set to true when any operation causes an error.
    has_errored: bool,
}

impl CrashTracker {
    pub fn new() -> Self {
        Self { has_errored: false }
    }

    /// Mark that an error occurred (called when a step fails).
    pub fn mark_error(&mut self) {
        self.has_errored = true;
    }

    /// Check and reset the crash state.
    /// Returns Ok if no crash occurred since last check, Err otherwise.
    pub fn check_and_reset(&mut self) -> StepResult {
        if self.has_errored {
            self.has_errored = false;
            StepResult::err("engine has crashed or errored since the last no_crash check")
        } else {
            StepResult::ok()
        }
    }
}

/// Assert that no crash/error has occurred since the last `no_crash` check.
pub fn assert_no_crash(tracker: &mut CrashTracker) -> StepResult {
    tracker.check_and_reset()
}

/// Assert that a file or resource exists at the given path.
pub fn assert_resource_exists(args: &toml::Value) -> StepResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return StepResult::err("missing 'path' argument for resource_exists"),
    };

    if std::path::Path::new(path).exists() {
        StepResult::ok()
    } else {
        StepResult::err(format!("resource not found: '{path}'"))
    }
}

/// Assert that the engine log buffer contains a given pattern.
pub fn assert_log_contains(log: &Log, args: &toml::Value) -> StepResult {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return StepResult::err("missing 'pattern' argument for log_contains"),
    };

    let found = log.iter().any(|msg| msg.message.contains(pattern));

    if found {
        StepResult::ok()
    } else {
        StepResult::err(format!("log does not contain pattern: '{pattern}'"))
    }
}

/// Assert an ECS query condition.
///
/// v1 supports:
/// - `query = "all"` with `expect = "count >= N"` or similar comparisons.
pub fn assert_ecs_query(world: &World, args: &toml::Value) -> StepResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => return StepResult::err("missing 'query' argument for ecs_query"),
    };

    let expect = match args.get("expect").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return StepResult::err("missing 'expect' argument for ecs_query"),
    };

    match query {
        "all" => {
            let count = world.iter().count();
            evaluate_count_condition(count, expect)
        }
        other => StepResult::err(format!("unsupported query type: '{other}'. v1 supports: 'all'")),
    }
}

/// Assert that a wgpu resource is valid.
///
/// v1: stub — always passes. GPU resource validation requires a live
/// render pipeline and will be implemented in a future iteration.
pub fn assert_wgpu_valid(_args: &toml::Value) -> StepResult {
    StepResult::ok()
}

/// Parse and evaluate a count condition like "count >= 1" or "count == 0".
fn evaluate_count_condition(actual: usize, condition: &str) -> StepResult {
    let condition = condition.trim();

    // Try patterns: "count >= N", "count > N", "count == N", "count < N", "count <= N"
    let parts: Vec<&str> = condition.split_whitespace().collect();
    if parts.len() < 3 || parts[0] != "count" {
        return StepResult::err(format!(
            "invalid expect format: '{condition}'. Expected 'count OP N'"
        ));
    }

    let op = parts[1];
    let expected: usize = match parts[2].parse() {
        Ok(n) => n,
        Err(_) => {
            return StepResult::err(format!(
                "invalid number in expect: '{}'",
                parts[2]
            ));
        }
    };

    let pass = match op {
        ">=" => actual >= expected,
        ">" => actual > expected,
        "==" => actual == expected,
        "<" => actual < expected,
        "<=" => actual <= expected,
        _ => {
            return StepResult::err(format!(
                "unknown operator '{op}' in expect: '{condition}'"
            ));
        }
    };

    if pass {
        StepResult::ok()
    } else {
        StepResult::err(format!(
            "ecs_query failed: actual count {actual} {op} {expected} is false"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- no_crash ---

    #[test]
    fn no_crash_passes_when_clean() {
        let mut tracker = CrashTracker::new();
        let result = assert_no_crash(&mut tracker);
        assert!(result.ok);
    }

    #[test]
    fn no_crash_fails_after_error() {
        let mut tracker = CrashTracker::new();
        tracker.mark_error();
        let result = assert_no_crash(&mut tracker);
        assert!(!result.ok);
        assert!(result.message.contains("crashed or errored"));
    }

    #[test]
    fn no_crash_resets_after_check() {
        let mut tracker = CrashTracker::new();
        tracker.mark_error();
        let first = assert_no_crash(&mut tracker);
        assert!(!first.ok);
        // After check, tracker is reset
        let second = assert_no_crash(&mut tracker);
        assert!(second.ok);
    }

    // --- resource_exists ---

    #[test]
    fn resource_exists_finds_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_asset.txt");
        std::fs::write(&file_path, "data").unwrap();

        let args: toml::Value = toml::from_str(&format!(
            "path = '{}'",
            file_path.display().to_string().replace('\\', "/")
        ))
        .unwrap();

        let result = assert_resource_exists(&args);
        assert!(result.ok, "should find file: {}", result.message);
    }

    #[test]
    fn resource_exists_fails_for_missing_file() {
        let args: toml::Value =
            toml::from_str("path = '/nonexistent/path/xyz.asset'").unwrap();
        let result = assert_resource_exists(&args);
        assert!(!result.ok);
        assert!(result.message.contains("resource not found"));
    }

    #[test]
    fn resource_exists_requires_path_arg() {
        let args: toml::Value = toml::from_str("other = 1").unwrap();
        let result = assert_resource_exists(&args);
        assert!(!result.ok);
        assert!(result.message.contains("missing 'path'"));
    }

    // --- log_contains ---

    #[test]
    fn log_contains_finds_pattern() {
        let mut log = Log::new();
        log.info("Texture loaded: cat.png");
        log.warning("Memory usage high");

        let args: toml::Value =
            toml::from_str("pattern = 'Texture loaded'").unwrap();
        let result = assert_log_contains(&log, &args);
        assert!(result.ok);
    }

    #[test]
    fn log_contains_misses_absent_pattern() {
        let mut log = Log::new();
        log.info("Something else");

        let args: toml::Value = toml::from_str("pattern = 'not present'").unwrap();
        let result = assert_log_contains(&log, &args);
        assert!(!result.ok);
    }

    // --- ecs_query ---

    #[test]
    fn ecs_query_counts_empty_world() {
        let world = World::new();
        let args: toml::Value =
            toml::from_str("query = 'all'\nexpect = 'count == 0'").unwrap();
        let result = assert_ecs_query(&world, &args);
        assert!(result.ok);
    }

    #[test]
    fn ecs_query_fails_when_count_mismatch() {
        let world = World::new();
        let args: toml::Value =
            toml::from_str("query = 'all'\nexpect = 'count >= 1'").unwrap();
        let result = assert_ecs_query(&world, &args);
        assert!(!result.ok);
    }

    // --- wgpu_valid (stub) ---

    #[test]
    fn wgpu_valid_stub_always_passes() {
        let args: toml::Value = toml::from_str("resource_type = 'Texture'").unwrap();
        let result = assert_wgpu_valid(&args);
        assert!(result.ok);
    }

    // --- evaluate_count_condition ---

    #[test]
    fn count_condition_gte() {
        assert!(evaluate_count_condition(5, "count >= 3").ok);
        assert!(evaluate_count_condition(3, "count >= 3").ok);
        assert!(!evaluate_count_condition(2, "count >= 3").ok);
    }

    #[test]
    fn count_condition_eq() {
        assert!(evaluate_count_condition(3, "count == 3").ok);
        assert!(!evaluate_count_condition(4, "count == 3").ok);
    }
}
