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

pub(crate) mod fs;
pub(crate) mod ignore;
pub(crate) mod scanner;
pub(crate) mod thumbnail;
pub(crate) mod watcher;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use kairos_editor_core::project::{FsOp, FsOpId, FsOpKind, FsOutcome, ScanPolicy, ScanReason};
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
/// operations (M3) and the delete thread.
pub(crate) struct ProjectService {
    mode: Mode,
    watcher: Option<watcher::Watcher>,
    scan: Option<ScanJob>,
    /// Deletes in flight, oldest first.
    ///
    /// A vector rather than the scan's single job: a newer delete must not
    /// cancel an older one (both are user intent, and the file system may
    /// already have half of the first one in the bin), and the worker is cheap
    /// enough that two threads are better than a lost completion.
    trash: Vec<TrashJob>,
}

/// One in-flight background scan.
struct ScanJob {
    generation: u64,
    /// Set when a newer request arrives: the walk stops at its next directory
    /// instead of finishing a result that will be discarded.
    cancel: Arc<AtomicBool>,
    result: Receiver<scanner::ScanResult>,
}

/// One delete running on its own thread.
struct TrashJob {
    /// The request this answers; the reducer uses it to attribute the result.
    op: FsOpId,
    result: Receiver<Result<FsOutcome, String>>,
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
            trash: Vec::new(),
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
            trash: Vec::new(),
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
                        errors: vec![String::from("the scan thread stopped without a result")],
                    }));
                }
            }
        }

        // A delete finishes on its own thread; the answer is an ordinary
        // completion, so the reducer cannot tell it from a synchronous one.
        let mut still_running = Vec::with_capacity(self.trash.len());
        for job in self.trash.drain(..) {
            match job.result.try_recv() {
                Ok(result) => actions.push(fs_completion(job.op, result)),
                Err(TryRecvError::Empty) => still_running.push(job),
                Err(TryRecvError::Disconnected) => actions.push(fs_completion(
                    job.op,
                    Err(String::from("the delete thread stopped without a result")),
                )),
            }
        }
        self.trash = still_running;

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
                                    errors: vec![format!("cannot start the scan thread: {error}",)],
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
            //
            // Create, rename and move are synchronous: one syscall each, so the
            // completion is ready before the effect returns. Delete is the one
            // operation that gets a worker thread — the OS recycle bin is a COM
            // shell call on Windows and can take hundreds of milliseconds on a
            // big directory, and the `trash` crate initialises the COM apartment
            // of the thread it runs on, which is a mode the host's own thread
            // should not be switched into.
            ProjectEffect::Fs(op) => {
                if matches!(&op.kind, FsOpKind::Trash { .. }) {
                    self.execute_trash(op)
                } else {
                    let result = fs::execute(&op.root, &op.companions, &op.kind);
                    if let Err(error) = &result {
                        kairos_log::warn!(target: "kairos_editor", "project {op_id}: {error}", op_id = op.id);
                    }
                    vec![fs_completion(op.id, result)]
                }
            }
            ProjectEffect::Reveal { root, path } => {
                let absolute = root.join(path.as_str());
                match reveal_in_file_manager(&absolute) {
                    Ok(()) => Vec::new(),
                    Err(error) => vec![Action::Project(ProjectAction::Failed {
                        message: format!("cannot reveal {}: {error}", absolute.display()),
                    })],
                }
            }
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

    /// Start (or, in tests, run) the one operation that leaves the frame thread.
    ///
    /// The worker owns the request and answers with an ordinary
    /// [`ProjectAction::FsCompleted`], so the reducer has one completion shape
    /// whether the host work was a syscall or a thread. The thread name is the
    /// only attribution a crash log gets, so it says what the thread was doing.
    fn execute_trash(&mut self, op: FsOp) -> Vec<Action> {
        match self.mode {
            #[cfg(test)]
            Mode::Inline => {
                let result = fs::execute(&op.root, &op.companions, &op.kind);
                if let Err(error) = &result {
                    kairos_log::warn!(target: "kairos_editor", "project {op_id}: {error}", op_id = op.id);
                }
                vec![fs_completion(op.id, result)]
            }
            Mode::Background => {
                let FsOp {
                    id,
                    root,
                    companions,
                    kind,
                } = op;
                let (sender, result) = mpsc::channel();
                let spawned = std::thread::Builder::new()
                    .name(String::from("project-trash"))
                    .spawn(move || {
                        // The `trash` crate initialises this thread's platform
                        // state (on Windows, a COM apartment) itself, which is
                        // exactly why the call cannot share the host's thread.
                        let outcome = fs::execute(&root, &companions, &kind);
                        let _ = sender.send(outcome);
                    });
                match spawned {
                    Ok(_thread) => {
                        self.trash.push(TrashJob { op: id, result });
                        Vec::new()
                    }
                    Err(error) => {
                        kairos_log::warn!(
                            target: "kairos_editor",
                            "project {id}: cannot start the delete thread: {error}"
                        );
                        vec![fs_completion(
                            id,
                            Err(format!("cannot start the delete thread: {error}")),
                        )]
                    }
                }
            }
        }
    }
}

/// The completion for one file operation, whichever host path ran it.
fn fs_completion(op: FsOpId, result: Result<FsOutcome, String>) -> Action {
    Action::Project(ProjectAction::FsCompleted { op, result })
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
/// Hand one path to the OS file manager with the file selected where the
/// platform supports it.
///
/// Spawned and forgotten: the editor cannot wait for a file manager, and a
/// failure means the OS refused to start one at all, which the panel reports.
#[cfg(target_os = "windows")]
fn reveal_in_file_manager(path: &Path) -> std::io::Result<()> {
    // `explorer /select,<path>` is the documented way to open the containing
    // folder with the file highlighted; the path is quoted because the single
    // argument is parsed again by the shell.
    std::process::Command::new("explorer")
        .arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .map(drop)
}

#[cfg(target_os = "macos")]
fn reveal_in_file_manager(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map(drop)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_in_file_manager(path: &Path) -> std::io::Result<()> {
    // No widespread "reveal this file" verb: open the containing directory.
    let directory = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open")
        .arg(directory)
        .spawn()
        .map(drop)
}
