use serde::{Deserialize, Serialize};

/// A single step in a TOML test file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    pub action: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub args: Option<toml::Value>,
}

/// Result of executing a single test step.
#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub message: String,
    /// When > 0, the bridge should pause draining for this many frames
    /// (engine continues to run normally). Used by system.wait_frames.
    #[serde(skip)]
    pub wait_frames: usize,
}

impl StepResult {
    pub fn ok() -> Self {
        Self {
            ok: true,
            message: String::new(),
            wait_frames: 0,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: msg.into(),
            wait_frames: 0,
        }
    }

    /// Create a result that also pauses the bridge for N frames.
    pub fn with_wait_frames(count: usize) -> Self {
        Self {
            ok: true,
            message: String::new(),
            wait_frames: count,
        }
    }
}

/// Overall result from running a complete TOML test file.
#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub status: String,
    pub total_steps: usize,
    pub completed_steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TestResult {
    pub fn pass(total: usize) -> Self {
        Self {
            status: "passed".into(),
            total_steps: total,
            completed_steps: total,
            error: None,
        }
    }

    pub fn fail(step: usize, total: usize, error: String) -> Self {
        Self {
            status: "failed".into(),
            total_steps: total,
            completed_steps: step,
            error: Some(error),
        }
    }
}

/// Incoming WebSocket request from the agent.
#[derive(Debug, Deserialize)]
pub struct WsRequest {
    pub cmd: String,
    #[serde(default)]
    pub file: Option<String>,
}

/// Outgoing WebSocket response to the agent.
#[derive(Debug, Serialize)]
pub struct WsResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TestResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl WsResponse {
    pub fn echo(message: String) -> Self {
        Self {
            status: "echo".into(),
            result: None,
            error: Some(message),
        }
    }

    pub fn from_test_result(result: TestResult) -> Self {
        let status = result.status.clone();
        Self {
            status,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            status: "error".into(),
            result: None,
            error: Some(msg.into()),
        }
    }
}

/// TOML test file root structure.
#[derive(Debug, Deserialize)]
pub struct TestFile {
    pub step: Vec<TestStep>,
}
