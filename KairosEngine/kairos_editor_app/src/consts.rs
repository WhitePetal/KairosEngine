//! Product identity of the editor shell.
//!
//! One place for what this binary calls itself. The window title (`main.rs`)
//! and the About panel's `EditorInfo` both read these constants, so the title
//! bar and the About window can never disagree about the version again — the
//! legacy About window compiled the version of its own crate (0.1.0) while the
//! title bar showed the shell's.

pub const APP_NAME: &str = "KairosEngine";

/// Product version of the editor shell.
///
/// Deliberately a product literal, not `env!("CARGO_PKG_VERSION")`: the
/// workspace still declares 0.1.0 while the 0.2.0 shell is what this binary is.
/// Unifying the two is a release decision (see
/// `docs/plan/editor-ui-architecture/about-panel.md` §6), not something a
/// panel or a constant should silently decide.
pub const VERSION: &str = "0.2.0";

/// One sentence describing the engine, shown by the About panel.
pub const DESCRIPTION: &str = "A pure Rust game engine built from first principles — Data-Oriented Design, ECS architecture, high performance, high flexibility, and high extensibility.";

/// SPDX expression of the dual licence; the full texts are `LICENSE-MIT` and
/// `LICENSE-APACHE` at the repository root.
pub const LICENSE: &str = "MIT OR Apache-2.0";

pub const REPOSITORY_URL: &str = "https://github.com/WhitePetal/KairosEngine";
pub const DISCUSSIONS_URL: &str = "https://github.com/WhitePetal/KairosEngine/discussions";
/// Third-party acknowledgements (`Thanks.md` at the repository root).
pub const CREDITS_URL: &str = "https://github.com/WhitePetal/KairosEngine/blob/main/Thanks.md";
