use crate::kairos_test_harness::{
    bridge::EngineCommand,
    types::{TestFile, TestResult},
};
use tokio::sync::{mpsc, oneshot};

/// Run a TOML test file asynchronously on the tokio runtime.
///
/// Reads the file, parses it, and sends each step to the main thread
/// via the bridge sender. Collects results and returns a `TestResult`.
pub async fn run_test_file(
    file_path: &str,
    sender: mpsc::Sender<EngineCommand>,
) -> TestResult {
    let content = match tokio::fs::read_to_string(file_path).await {
        Ok(c) => c,
        Err(e) => {
            return TestResult::fail(0, 0, format!("failed to read test file: {e}"));
        }
    };

    let test: TestFile = match toml::from_str(&content) {
        Ok(t) => t,
        Err(e) => {
            return TestResult::fail(0, 0, format!("failed to parse TOML: {e}"));
        }
    };

    let total = test.step.len();

    for (i, step) in test.step.into_iter().enumerate() {
        let (tx, rx) = oneshot::channel();

        if sender
            .send(EngineCommand::ExecuteStep {
                step,
                response: tx,
            })
            .await
            .is_err()
        {
            return TestResult::fail(i + 1, total, "bridge channel closed".into());
        }

        match rx.await {
            Ok(step_result) if step_result.ok => {
                // Step passed, continue
            }
            Ok(step_result) => {
                return TestResult::fail(i + 1, total, step_result.message);
            }
            Err(_) => {
                return TestResult::fail(
                    i + 1,
                    total,
                    "engine dropped the response channel".into(),
                );
            }
        }
    }

    TestResult::pass(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_toml() {
        let toml_str = r#"
[[step]]
action = "call"
target = "system.ping"

[[step]]
action = "call"
target = "system.ping"
"#;
        let test: TestFile =
            toml::from_str(toml_str).expect("valid TOML should parse");
        assert_eq!(test.step.len(), 2);
        assert_eq!(test.step[0].action, "call");
        assert_eq!(test.step[0].target.as_deref(), Some("system.ping"));
    }

    #[test]
    fn parse_toml_with_args() {
        let toml_str = r#"
[[step]]
action = "call"
target = "texture_inspector.select_format"
args = { format = "BC7" }
"#;
        let test: TestFile =
            toml::from_str(toml_str).expect("TOML with args should parse");
        assert_eq!(test.step.len(), 1);
        let args = test.step[0].args.as_ref().unwrap();
        assert_eq!(args["format"].as_str(), Some("BC7"));
    }

    #[test]
    fn parse_invalid_toml() {
        let toml_str = "this is not valid toml [[[";
        let result: Result<TestFile, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn parse_real_e2e_tests() {
        // Resolve paths relative to workspace root
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let files = [
            workspace_root.join("tests/runtime/smoke_test.toml"),
            workspace_root.join("tests/runtime/texture_format_change.toml"),
            workspace_root.join("tests/runtime/widget_click.toml"),
        ];
        for path in &files {
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            let test: TestFile = toml::from_str(&content)
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
            assert!(!test.step.is_empty(), "{} has no steps", path.display());
        }
    }
}
