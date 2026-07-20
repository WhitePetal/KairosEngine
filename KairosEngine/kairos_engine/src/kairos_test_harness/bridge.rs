use crate::kairos_editor::KairosEngine;
use crate::kairos_test_harness::{assertions::CrashTracker, dispatch, types::StepResult};
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
    crash_tracker: CrashTracker,
}

impl Bridge {
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer);
        Self {
            tx,
            rx,
            crash_tracker: CrashTracker::new(),
        }
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

    fn handle(&mut self, cmd: EngineCommand, engine: &mut KairosEngine) {
        match cmd {
            EngineCommand::Echo { message, response } => {
                let echoed = format!("echo: {message}");
                let _ = response.send(echoed);
            }
            EngineCommand::ExecuteStep { step, response } => {
                let result = execute_step(&step, engine, &mut self.crash_tracker);
                // Track errors for no_crash assertion
                if !result.ok {
                    self.crash_tracker.mark_error();
                }
                let _ = response.send(result);
            }
        }
    }
}

/// Route a test step to the appropriate handler based on its action.
fn execute_step(
    step: &TestStep,
    engine: &mut KairosEngine,
    crash_tracker: &mut CrashTracker,
) -> StepResult {
    match step.action.as_str() {
        "call" => dispatch::dispatch_call(step, engine),
        "assert" => dispatch::dispatch_assert(step, engine, crash_tracker),
        other => StepResult::err(format!("unknown action: '{other}'")),
    }
}
