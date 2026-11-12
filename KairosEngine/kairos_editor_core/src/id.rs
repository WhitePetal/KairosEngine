//! Stable panel identities: store vocabulary, not Rust types.
//!
//! A panel kind is a string, so adding a panel never touches this crate. An
//! instance id adds the serial that lets the dock open several panels of the
//! same kind (`viewport#0`, `viewport#1`) without changing any API.

/// Stable kind of a panel (independent of how many instances are open).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelId(pub &'static str);

impl std::fmt::Display for PanelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Stable identity of one open panel instance. This is the id stored in the
/// dock: kind identifies the implementation, serial distinguishes instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelInstanceId {
    pub kind: PanelId,
    pub serial: u32,
}

impl PanelInstanceId {
    pub const fn new(kind: PanelId, serial: u32) -> Self {
        Self { kind, serial }
    }
}
