//! Kairos Supervisor — thin watchdog process for the KairosEngine test harness.
//!
//! Starts the engine as a child process, monitors its lifecycle, and
//! captures crash information (exit code + stderr) to `kairos_crash.log`
//! when the engine terminates abnormally.
//!
//! ## Usage
//!
//! ```text
//! kairos_supervisor <engine-binary> [engine-args...]
//! ```
//!
//! ## Exit codes
//!
//! - 0: engine exited cleanly
//! - 1: engine crashed (crash log written)
//! - 2: supervisor usage error

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;

const CRASH_LOG_FILE: &str = "kairos_crash.log";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!(
            "kairos_supervisor — thin watchdog for KairosEngine test harness\n\n\
             Usage: kairos_supervisor <engine-binary> [engine-args...]\n\n\
             Starts the engine as a child process. On crash, writes stderr to {CRASH_LOG_FILE}."
        );
        std::process::exit(2);
    }

    let engine_path = &args[0];
    let engine_args = &args[1..];

    // Spawn the engine, capturing stderr for crash analysis
    let mut child = match Command::new(engine_path)
        .args(engine_args)
        .stdout(Stdio::inherit()) // engine stdout passes through
        .stdin(Stdio::null())
        .stderr(Stdio::piped()) // capture stderr for crash log
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to start engine '{engine_path}': {e}");
            eprintln!("{msg}");
            std::fs::write(CRASH_LOG_FILE, &msg).ok();
            std::process::exit(1);
        }
    };

    // Read stderr in a background thread so the pipe doesn't fill up
    // and block the child process.
    let mut stderr_pipe = child.stderr.take().expect("stderr should be piped");
    let stderr_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        stderr_pipe.read_to_end(&mut buf).ok();
        String::from_utf8_lossy(&buf).into_owned()
    });

    // Wait for the engine to exit
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Failed to wait on engine process: {e}");
            eprintln!("{msg}");
            std::fs::write(CRASH_LOG_FILE, &msg).ok();
            std::process::exit(1);
        }
    };

    // Wait for stderr capture to finish
    let stderr_output = match stderr_thread.join() {
        Ok(s) => s,
        Err(_) => String::from("<failed to capture stderr>"),
    };

    if status.success() {
        // Engine exited cleanly — pass through the exit code
        std::process::exit(0);
    } else {
        // Engine crashed — write crash log
        let exit_code = status.code().unwrap_or(-1);
        let log_entry = format!(
            "Engine crashed with exit code: {exit_code}\n\n--- stderr ---\n{stderr_output}\n--- end stderr ---\n"
        );

        eprintln!("Engine crashed (exit code {exit_code}). See {CRASH_LOG_FILE} for details.");

        if let Err(e) = std::fs::write(CRASH_LOG_FILE, &log_entry) {
            eprintln!("Failed to write crash log: {e}");
        }

        std::process::exit(1);
    }
}
