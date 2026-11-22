//! Host side of the project slice: everything about the project that touches the
//! file system.
//!
//! `EditorState::project` owns the data; this service owns the work. The app hands
//! it every `ProjectEffect` and, in the host phase of each frame
//! (`EditorApp::begin_frame`), takes back what completed as actions — which the app
//! applies through the same reducer that handles the UI's own actions.
//!
//! The pieces are split by responsibility: [`scanner`] reads directories (the only
//! `std::fs` in the slice), [`watcher`] reports that something under the root
//! changed (filtered by the same rules the walk uses), and [`ignore`] reads and
//! seeds `.kairosignore`. This module is the seam that boots them, turns effects
//! into work, and turns completed work back into actions.
//!
//! Nothing here touches `EditorState`: the host owns the file system and the
//! threads, the reducer owns the state. That split is the reason a scan cannot
//! leave the tree half-updated.

pub(crate) mod ignore;
pub(crate) mod scanner;
pub(crate) mod watcher;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use kairos_editor_core::project::{ScanPolicy, ScanReason};
use kairos_editor_core::{Action, ProjectAction, ProjectEffect};

/// How a scan request is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Production: the walk runs on its own thread and completes through `poll`.
    Background,
    /// Unit tests: the same walk runs on the calling thread, so a test can assert
    /// on the completion without waiting for a scheduler.
    #[cfg(test)]
    Inline,
}

/// Owns the project's host work: the scan thread and the watcher (M1), the file
/// operations (M3).
pub(crate) struct ProjectService {
    mode: Mode,
    watcher: Option<watcher::Watcher>,
    scan: Option<ScanJob>,
}

/// One in-flight background scan.
struct ScanJob {
    generation: u64,
    /// Set when a newer request arrives: the walk stops at its next directory
    /// instead of finishing a result that will be discarded.
    cancel: Arc<AtomicBool>,
    result: Receiver<scanner::ScanResult>,
}

impl ProjectService {
    /// Boot the project's host side: read (or seed) `.kairosignore`, start the
    /// watcher, and answer with the actions that begin the first scan.
    ///
    /// The order matters and is the app's, not this module's: `EditorApp::new`
    /// pushes these actions into the inbox in this order, so the reducer has the
    /// policy before anything asks for a walk. The watcher starts after the ignore
    /// file is seeded, so seeding it does not look like an external change.
    pub(crate) fn boot(root: PathBuf) -> (Self, Vec<Action>) {
        let mut policy = ScanPolicy::default();
        policy.ignore = ignore::load_or_seed(&root);

        let watcher = match watcher::Watcher::new(&root, &policy) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                // Watching must degrade, not abort the editor: the tree loses
                // automatic refresh and keeps everything else.
                kairos_log::warn!(
                    target: "kairos_editor",
                    "project watcher: cannot watch {}: {error}",
                    root.display()
                );
                None
            }
        };

        let service = Self {
            mode: Mode::Background,
            watcher,
            scan: None,
        };
        let actions = vec![
            Action::Project(ProjectAction::PolicyLoaded { policy }),
            Action::Project(ProjectAction::ScanRequested {
                reason: ScanReason::Startup,
                focus: None,
            }),
        ];
        (service, actions)
    }

    /// A service that answers scans on the calling thread and watches nothing.
    /// For unit tests: same walk, no scheduler and no filesystem events.
    #[cfg(test)]
    pub(crate) fn inline() -> Self {
        Self {
            mode: Mode::Inline,
            watcher: None,
            scan: None,
        }
    }

    /// Host work that completed since the last frame, as actions.
    ///
    /// Drained once per frame in `EditorApp::begin_frame`. A scan that is still
    /// running stays in `self.scan`; a finished one becomes its completion, and
    /// the watcher may add a rescan request after it. Completions and requests
    /// travel the same inbox as UI actions, in arrival order.
    pub(crate) fn poll(&mut self, _now: Instant) -> Vec<Action> {
        let mut actions = Vec::new();

        if let Some(job) = self.scan.take() {
            match job.result.try_recv() {
                Ok(result) => actions.push(completion(result)),
                Err(TryRecvError::Empty) => self.scan = Some(job),
                Err(TryRecvError::Disconnected) => {
                    // The thread died without sending (a panic in the walk).
                    // Answering with a visible failure keeps the slice from being
                    // stuck in `running` forever.
                    actions.push(Action::Project(ProjectAction::Scanned {
                        generation: job.generation,
                        entries: Vec::new(),
                        truncated: false,
                        elapsed_ms: 0,
                        library_nodes: 0,
                        errors: vec![String::from(
                            "the scan thread stopped without a result",
                        )],
                    }));
                }
            }
        }

        if let Some(watcher) = &self.watcher
            && watcher.take_change()
        {
            actions.push(Action::Project(ProjectAction::ScanRequested {
                reason: ScanReason::ExternalChange,
                focus: None,
            }));
        }

        actions
    }

    /// Execute one effect, returning the actions it produced immediately.
    ///
    /// A background scan produces nothing here: its completion arrives through
    /// [`Self::poll`] on a later frame. Inline mode answers directly, which is what
    /// keeps the request → effect → completion protocol testable without timing.
    pub(crate) fn execute(&mut self, effect: ProjectEffect, _now: Instant) -> Vec<Action> {
        match effect {
            ProjectEffect::Scan {
                root,
                policy,
                generation,
            } => {
                // A newer request supersedes the old walk: let it stop early.
                if let Some(previous) = self.scan.take() {
                    previous.cancel.store(true, Ordering::Relaxed);
                }
                match self.mode {
                    #[cfg(test)]
                    Mode::Inline => {
                        let cancel = AtomicBool::new(false);
                        let result = scanner::scan(&root, &policy, generation, &cancel);
                        vec![completion(result)]
                    }
                    Mode::Background => {
                        let cancel = Arc::new(AtomicBool::new(false));
                        let thread_cancel = Arc::clone(&cancel);
                        let (sender, result) = mpsc::channel();
                        let spawned = std::thread::Builder::new()
                            .name(String::from("project-scan"))
                            .spawn(move || {
                                let result =
                                    scanner::scan(&root, &policy, generation, &thread_cancel);
                                // A dropped receiver means the scan was
                                // superseded or the editor exited.
                                let _ = sender.send(result);
                            });
                        match spawned {
                            Ok(_thread) => {
                                self.scan = Some(ScanJob {
                                    generation,
                                    cancel,
                                    result,
                                });
                                Vec::new()
                            }
                            Err(error) => {
                                kairos_log::warn!(
                                    target: "kairos_editor",
                                    "project scan: cannot start the scan thread: {error}"
                                );
                                vec![Action::Project(ProjectAction::Scanned {
                                    generation,
                                    entries: Vec::new(),
                                    truncated: false,
                                    elapsed_ms: 0,
                                    library_nodes: 0,
                                    errors: vec![format!(
                                        "cannot start the scan thread: {error}",
                                    )],
                                })]
                            }
                        }
                    }
                }
            }
            // Opening is not the project's state: the host hands the file to the
            // OS and only reports the failure back. Synchronous on purpose —
            // `that_detached` does not block on the spawned program, and an
            // error means the OS refused to start one at all (M5 replaces this
            // with the typed `OpenWith` dispatch, IDE detection included).
            ProjectEffect::Open { root, path } => {
                let absolute = root.join(path.as_str());
                match open::that_detached(&absolute) {
                    Ok(()) => Vec::new(),
                    Err(error) => vec![Action::Project(ProjectAction::Failed {
                        message: format!("cannot open {}: {error}", absolute.display()),
                    })],
                }
            }
        }
    }
}

/// Turn a finished walk into the completion the reducer expects.
fn completion(result: scanner::ScanResult) -> Action {
    Action::Project(ProjectAction::Scanned {
        generation: result.generation,
        entries: result.entries,
        truncated: result.truncated,
        elapsed_ms: result.elapsed_ms,
        library_nodes: result.library_nodes,
        errors: result.errors,
    })
}