//! Serialization boundary: pretty JSON and Chrome Trace Event Format.
//!
//! The in-memory model is deliberately transport-neutral. These exports are
//! additive views over it:
//!
//! * JSON is the schema the Profiler panel's JSON tab shows and the format a
//!   future capture loader reads;
//! * Chrome Trace Event Format (`X` complete events) opens in
//!   <https://ui.perfetto.dev> or `chrome://tracing` without any Kairos tooling.

use std::{fs::File, io::BufWriter, io::Write, path::Path};

use serde::Serialize;
use serde_json::{Value, json};

use crate::model::{FrameSnapshot, ProfileSession, SCHEMA_VERSION, ScopeInfo};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileDocument<'a> {
    schema_version: u32,
    origin_ns: Option<i64>,
    dropped_frames: u64,
    discarded_passes: u64,
    scopes: &'a [ScopeInfo],
    frames: Vec<&'a FrameSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameDocument<'a> {
    schema_version: u32,
    scopes: &'a [ScopeInfo],
    frame: &'a FrameSnapshot,
}

impl ProfileSession {
    fn document(&self) -> ProfileDocument<'_> {
        ProfileDocument {
            schema_version: SCHEMA_VERSION,
            origin_ns: self.origin_ns,
            dropped_frames: self.dropped_frames,
            discarded_passes: self.discarded_passes,
            scopes: &self.scopes,
            frames: self.frames.iter().map(|frame| frame.as_ref()).collect(),
        }
    }

    fn frame_document(&self, index: usize) -> Option<FrameDocument<'_>> {
        let frame = self.frames.get(index)?;
        Some(FrameDocument {
            schema_version: SCHEMA_VERSION,
            scopes: &self.scopes,
            frame: frame.as_ref(),
        })
    }

    /// Pretty JSON for the whole capture: schema version, scope table, frames
    /// and capture statistics.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.document())
            .expect("profile document is made of serializable fields")
    }

    /// Pretty JSON for one frame plus the marker table. `None` when the frame
    /// index is out of range.
    pub fn frame_json(&self, index: usize) -> Option<String> {
        let document = self.frame_document(index)?;
        Some(
            serde_json::to_string_pretty(&document)
                .expect("frame document is made of serializable fields"),
        )
    }

    /// Write [`ProfileSession::to_json`] to `path`.
    pub fn write_json(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &self.document())
            .map_err(std::io::Error::other)?;
        writer.flush()
    }

    /// Chrome Trace Event Format JSON for every kept frame.
    ///
    /// Times are microseconds relative to the capture origin. Every scope is a
    /// complete (`ph: "X"`) event on its thread; metadata events name the
    /// process and threads. Sub-microsecond spans get `dur: 0` and keep exact
    /// nanoseconds in `args.durationNs`.
    pub fn to_chrome_trace(&self) -> String {
        let mut events: Vec<Value> = Vec::new();
        events.push(json!({
            "ph": "M",
            "pid": 1,
            "name": "process_name",
            "args": { "name": "KairosEngine" },
        }));

        let mut thread_names: std::collections::BTreeMap<u32, &str> =
            std::collections::BTreeMap::new();
        for frame in &self.frames {
            for thread in &frame.threads {
                thread_names
                    .entry(thread.id)
                    .or_insert(thread.name.as_str());
            }
        }
        for (thread_id, name) in &thread_names {
            events.push(json!({
                "ph": "M",
                "pid": 1,
                "tid": thread_id,
                "name": "thread_name",
                "args": { "name": name },
            }));
        }

        for frame in &self.frames {
            for thread in &frame.threads {
                for sample in &thread.samples {
                    let scope = self.scope(sample.scope);
                    let name = scope.map_or("<unknown scope>", |scope| scope.name.as_str());
                    let category = scope
                        .and_then(|scope| {
                            (!scope.location.is_empty()).then_some(scope.location.as_str())
                        })
                        .unwrap_or("kairos");
                    let timestamp_us = frame.start_ns.saturating_add(sample.start_ns) / 1_000;
                    let duration_us = sample.duration_ns / 1_000;

                    let mut args = json!({
                        "frame": frame.index,
                        "scope": sample.scope,
                        "depth": sample.depth,
                        "durationNs": sample.duration_ns,
                    });
                    if let Some(data) = sample.data.as_deref() {
                        args["data"] = json!(data);
                    }

                    events.push(json!({
                        "ph": "X",
                        "pid": 1,
                        "tid": thread.id,
                        "ts": timestamp_us,
                        "dur": duration_us,
                        "name": name,
                        "cat": category,
                        "args": args,
                    }));
                }
            }
        }

        let trace = json!({
            "traceEvents": events,
            "displayTimeUnit": "ms",
        });
        serde_json::to_string_pretty(&trace).expect("trace events are serializable")
    }

    /// Write [`ProfileSession::to_chrome_trace`] to `path`.
    pub fn write_chrome_trace(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        std::fs::write(path, self.to_chrome_trace())
    }
}
