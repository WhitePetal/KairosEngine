use crate::kairos_test_harness::types::StepResult;
use crate::{ecs::world::World, log::Log};

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
        other => StepResult::err(format!(
            "unsupported query type: '{other}'. v1 supports: 'all'"
        )),
    }
}

/// Assert that a wgpu resource is valid.
///
/// v1: stub — always passes. GPU resource validation requires a live
/// render pipeline and will be implemented in a future iteration.
pub fn assert_wgpu_valid(_args: &toml::Value) -> StepResult {
    StepResult::ok()
}

/// Assert that a key in a TOML file matches an expected value.
///
/// args: `file` (path to TOML file), `key` (top-level key name), `value` (expected string).
pub fn assert_toml_value_equals(args: &toml::Value) -> StepResult {
    let file = match args.get("file").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return StepResult::err("toml_value_equals requires 'file' argument"),
    };
    let key = match args.get("key").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => return StepResult::err("toml_value_equals requires 'key' argument"),
    };
    let expected = match args.get("value").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return StepResult::err("toml_value_equals requires 'value' argument"),
    };

    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => return StepResult::err(format!("failed to read file '{file}': {e}")),
    };

    let table: toml::Table = match toml::from_str(&content) {
        Ok(t) => t,
        Err(e) => return StepResult::err(format!("failed to parse TOML '{file}': {e}")),
    };

    let actual = match lookup_dotted(&table, key) {
        Some(v) => format!("{}", v),
        None => return StepResult::err(format!("key '{key}' not found in '{file}'")),
    };

    // Compare: TOML values are displayed without quotes for strings
    let actual_normalized = actual.trim().trim_matches('"');
    let expected_normalized = expected.trim().trim_matches('"');

    if actual_normalized == expected_normalized {
        StepResult::ok()
    } else {
        StepResult::err(format!(
            "toml_value_equals failed: key '{key}' expected '{expected_normalized}', got '{actual_normalized}'"
        ))
    }
}

/// Traverse a TOML table by dotted key path (e.g. "render_state.cull_mod").
/// Single-segment keys behave like a plain `table.get(key)`.
fn lookup_dotted<'a>(table: &'a toml::Table, key: &str) -> Option<&'a toml::Value> {
    let mut current = table;
    let mut segments = key.split('.').peekable();
    while let Some(segment) = segments.next() {
        let is_last = segments.peek().is_none();
        match current.get(segment) {
            Some(toml::Value::Table(inner)) if !is_last => current = inner,
            Some(value) if is_last => return Some(value),
            _ => return None,
        }
    }
    None
}

/// Snapshot of the open MaterialInspector's runtime-material texture state.
///
/// Gathered by the dispatch layer (which has engine access) and handed to
/// [`assert_material_inspector_texture_loaded`] as plain data so the
/// assertion itself stays pure and unit-testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialInspectorTextureState {
    /// No MaterialInspector is currently open.
    NoInspector,
    /// The runtime Material asset has not finished loading yet.
    MaterialNotLoaded,
    /// The runtime Material has no texture assigned.
    NoTexture,
    /// A texture handle is assigned but the texture asset never resolved
    /// (e.g. the referenced file does not exist on disk).
    TextureUnresolved,
    /// The texture handle resolves to a loaded texture with these dimensions.
    Loaded { width: u32, height: u32 },
}

/// Assert that the open MaterialInspector's runtime Material has a texture
/// handle that resolves to a loaded texture.
///
/// Optional integer args `width` / `height` additionally pin the loaded
/// texture's dimensions (e.g. the 2x2 white fallback texture).
pub fn assert_material_inspector_texture_loaded(
    state: MaterialInspectorTextureState,
    args: &toml::Value,
) -> StepResult {
    let expected_width = args
        .get("width")
        .and_then(|v| v.as_integer())
        .map(|v| v as u32);
    let expected_height = args
        .get("height")
        .and_then(|v| v.as_integer())
        .map(|v| v as u32);

    match state {
        MaterialInspectorTextureState::Loaded { width, height } => {
            if let Some(expected) = expected_width
                && expected != width
            {
                return StepResult::err(format!(
                    "material_inspector.texture_loaded: expected texture width {expected}, got {width}"
                ));
            }
            if let Some(expected) = expected_height
                && expected != height
            {
                return StepResult::err(format!(
                    "material_inspector.texture_loaded: expected texture height {expected}, got {height}"
                ));
            }
            StepResult::ok()
        }
        other => StepResult::err(format!(
            "material_inspector.texture_loaded: runtime material texture is not loaded (state: {other:?})"
        )),
    }
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
            return StepResult::err(format!("invalid number in expect: '{}'", parts[2]));
        }
    };

    let pass = match op {
        ">=" => actual >= expected,
        ">" => actual > expected,
        "==" => actual == expected,
        "<" => actual < expected,
        "<=" => actual <= expected,
        _ => {
            return StepResult::err(format!("unknown operator '{op}' in expect: '{condition}'"));
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
        let args: toml::Value = toml::from_str("path = '/nonexistent/path/xyz.asset'").unwrap();
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

        let args: toml::Value = toml::from_str("pattern = 'Texture loaded'").unwrap();
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
        let args: toml::Value = toml::from_str("query = 'all'\nexpect = 'count == 0'").unwrap();
        let result = assert_ecs_query(&world, &args);
        assert!(result.ok);
    }

    #[test]
    fn ecs_query_fails_when_count_mismatch() {
        let world = World::new();
        let args: toml::Value = toml::from_str("query = 'all'\nexpect = 'count >= 1'").unwrap();
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

    // --- toml_value_equals ---

    #[test]
    fn toml_value_equals_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.texture");
        std::fs::write(&path, "format = \"BC7\"\nwidth = 256\n").unwrap();

        let args: toml::Value = toml::from_str(&format!(
            "file = '{}'\nkey = 'format'\nvalue = 'BC7'",
            path.display().to_string().replace('\\', "/")
        ))
        .unwrap();
        let result = assert_toml_value_equals(&args);
        assert!(result.ok, "{}", result.message);
    }

    #[test]
    fn toml_value_equals_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.texture");
        std::fs::write(&path, "format = \"R8Unorm\"\n").unwrap();

        let args: toml::Value = toml::from_str(&format!(
            "file = '{}'\nkey = 'format'\nvalue = 'BC7'",
            path.display().to_string().replace('\\', "/")
        ))
        .unwrap();
        let result = assert_toml_value_equals(&args);
        assert!(!result.ok);
    }

    #[test]
    fn toml_value_equals_key_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.texture");
        std::fs::write(&path, "width = 256\n").unwrap();

        let args: toml::Value = toml::from_str(&format!(
            "file = '{}'\nkey = 'format'\nvalue = 'BC7'",
            path.display().to_string().replace('\\', "/")
        ))
        .unwrap();
        let result = assert_toml_value_equals(&args);
        assert!(!result.ok);
        assert!(result.message.contains("not found"));
    }

    #[test]
    fn toml_value_equals_dotted_key_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.mat");
        std::fs::write(
            &path,
            "shader_path = \"a.wgsl\"\n[render_state]\ncull_mod = \"Front\"\ndepth_write = false\n",
        )
        .unwrap();

        let file = path.display().to_string().replace('\\', "/");
        for (key, value) in [
            ("render_state.cull_mod", "Front"),
            ("render_state.depth_write", "false"),
            ("shader_path", "a.wgsl"),
        ] {
            let args: toml::Value =
                toml::from_str(&format!("file = '{file}'\nkey = '{key}'\nvalue = '{value}'"))
                    .unwrap();
            let result = assert_toml_value_equals(&args);
            assert!(result.ok, "key {key}: {}", result.message);
        }
    }

    #[test]
    fn toml_value_equals_dotted_key_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.mat");
        std::fs::write(&path, "[render_state]\ncull_mod = \"Back\"\n").unwrap();

        let args: toml::Value = toml::from_str(&format!(
            "file = '{}'\nkey = 'render_state.cull_mod'\nvalue = 'Front'",
            path.display().to_string().replace('\\', "/")
        ))
        .unwrap();
        let result = assert_toml_value_equals(&args);
        assert!(!result.ok);
    }

    #[test]
    fn toml_value_equals_dotted_key_through_non_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.mat");
        std::fs::write(&path, "shader_path = \"a.wgsl\"\n").unwrap();

        let args: toml::Value = toml::from_str(&format!(
            "file = '{}'\nkey = 'shader_path.cull_mod'\nvalue = 'Front'",
            path.display().to_string().replace('\\', "/")
        ))
        .unwrap();
        let result = assert_toml_value_equals(&args);
        assert!(!result.ok);
        assert!(result.message.contains("not found"));
    }

    // --- material_inspector.texture_loaded ---

    #[test]
    fn material_texture_loaded_passes_with_matching_dims() {
        let state = MaterialInspectorTextureState::Loaded {
            width: 2,
            height: 2,
        };
        let args: toml::Value = toml::from_str("width = 2\nheight = 2").unwrap();
        let result = assert_material_inspector_texture_loaded(state, &args);
        assert!(result.ok, "{}", result.message);
    }

    #[test]
    fn material_texture_loaded_passes_without_dims() {
        let state = MaterialInspectorTextureState::Loaded {
            width: 64,
            height: 64,
        };
        let args = toml::Value::Table(toml::Table::new());
        let result = assert_material_inspector_texture_loaded(state, &args);
        assert!(result.ok, "{}", result.message);
    }

    #[test]
    fn material_texture_loaded_fails_on_width_mismatch() {
        let state = MaterialInspectorTextureState::Loaded {
            width: 2,
            height: 2,
        };
        let args: toml::Value = toml::from_str("width = 4").unwrap();
        let result = assert_material_inspector_texture_loaded(state, &args);
        assert!(!result.ok);
        assert!(result.message.contains("width"));
    }

    #[test]
    fn material_texture_loaded_fails_on_height_mismatch() {
        let state = MaterialInspectorTextureState::Loaded {
            width: 2,
            height: 2,
        };
        let args: toml::Value = toml::from_str("height = 4").unwrap();
        let result = assert_material_inspector_texture_loaded(state, &args);
        assert!(!result.ok);
        assert!(result.message.contains("height"));
    }

    #[test]
    fn material_texture_loaded_fails_when_not_resolved() {
        let args = toml::Value::Table(toml::Table::new());
        for state in [
            MaterialInspectorTextureState::NoInspector,
            MaterialInspectorTextureState::MaterialNotLoaded,
            MaterialInspectorTextureState::NoTexture,
            MaterialInspectorTextureState::TextureUnresolved,
        ] {
            let result = assert_material_inspector_texture_loaded(state, &args);
            assert!(!result.ok, "state {state:?} should not pass");
        }
    }
}
