use tokio::sync::{mpsc, oneshot};

/// Commands sent from the WebSocket server (tokio) to the main thread.
pub enum EngineCommand {
    /// Echo a message back. Used to verify the bridge is working.
    Echo {
        message: String,
        response: oneshot::Sender<String>,
    },
}

/// Bidirectional bridge between the tokio WS server and the main thread.
///
/// - `sender` (mpsc::Sender): cloned and handed to the WS server for sending
///   commands to the main thread.
/// - `receiver` (mpsc::Receiver): drained by the main thread each frame.
pub struct Bridge {
    tx: mpsc::Sender<EngineCommand>,
    rx: mpsc::Receiver<EngineCommand>,
}

impl Bridge {
    /// Create a new bridge with the given channel buffer size.
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer);
        Self { tx, rx }
    }

    /// Return a clone of the sender, for handing to the WS server.
    pub fn sender(&self) -> mpsc::Sender<EngineCommand> {
        self.tx.clone()
    }

    /// Drain all pending commands from the queue.
    /// Called every frame by the main thread.
    pub fn drain(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            self.handle(cmd);
        }
    }

    fn handle(&self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::Echo { message, response } => {
                let echoed = format!("echo: {message}");
                let _ = response.send(echoed);
            }
        }
    }
}
