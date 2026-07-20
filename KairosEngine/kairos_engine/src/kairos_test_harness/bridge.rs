use crate::kairos_editor::KairosEngine;
use crate::kairos_test_harness::{dispatch, types::StepResult};
use crate::kairos_test_harness::types::TestStep;
use tokio::sync::{mpsc, oneshot};

/// Commands sent from the WebSocket server (tokio) to the main thread.
pub enum EngineCommand {
    /// Echo a message back. Used for connectivity verification.
    Echo {
        message: String,
        response: oneshot::Sender<String>,
    },
    /// Execute a single test step (from a TOML file) against the engine.
    ExecuteStep {
        step: TestStep,
        response: oneshot::Sender<StepResult>,
    },
}

/// Bidirectional bridge between the tokio WS server and the main thread.
pub struct Bridge {
    tx: mpsc::Sender<EngineCommand>,
    rx: mpsc::Receiver<EngineCommand>,
}

impl Bridge {
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer);
        Self { tx, rx }
    }

    pub fn sender(&self) -> mpsc::Sender<EngineCommand> {
        self.tx.clone()
    }

    /// Drain all pending commands from the queue.
    /// Called every frame by the main thread, with access to engine state.
    pub fn drain(&mut self, engine: &mut KairosEngine) {
        while let Ok(cmd) = self.rx.try_recv() {
            self.handle(cmd, engine);
        }
    }

    fn handle(&self, cmd: EngineCommand, engine: &mut KairosEngine) {
        match cmd {
            EngineCommand::Echo { message, response } => {
                let echoed = format!("echo: {message}");
                let _ = response.send(echoed);
            }
            EngineCommand::ExecuteStep { step, response } => {
                let result = execute_step(&step, engine);
                let _ = response.send(result);
            }
        }
    }
}

/// Route a test step to the appropriate handler based on its action.
fn execute_step(step: &TestStep, engine: &mut KairosEngine) -> StepResult {
    match step.action.as_str() {
        "call" => dispatch::dispatch_call(step, engine),
        other => StepResult::err(format!("unknown action: '{other}'")),
    }
}
