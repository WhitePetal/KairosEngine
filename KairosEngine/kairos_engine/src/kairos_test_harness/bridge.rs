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
    /// Remaining frames to wait before draining new commands.
    /// Set by system.wait_frames; the engine's frame loop counts down.
    pending_wait_frames: usize,
}

impl Bridge {
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer);
        Self {
            tx,
            rx,
            crash_tracker: CrashTracker::new(),
            pending_wait_frames: 0,
        }
    }

    pub fn sender(&self) -> mpsc::Sender<EngineCommand> {
        self.tx.clone()
    }

    /// Drain pending commands from the queue.
    /// Called at the END of each frame so widget rects are already recorded.
    /// If system.wait_frames set a delay, we skip draining until it counts down.
    pub fn drain(&mut self, engine: &mut KairosEngine) {
        if self.pending_wait_frames > 0 {
            self.pending_wait_frames -= 1;
            return;
        }
        while let Ok(cmd) = self.rx.try_recv() {
            self.handle(cmd, engine);
            // If a step requested a frame wait, stop draining so the engine
            // can run those frames before the next command arrives.
            if self.pending_wait_frames > 0 {
                break;
            }
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
                // Apply any frame-wait requested by the step.
                if result.wait_frames > 0 {
                    self.pending_wait_frames = result.wait_frames;
                }
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
        "input" => dispatch::dispatch_input(step, engine),
        other => StepResult::err(format!("unknown action: '{other}'")),
    }
}
