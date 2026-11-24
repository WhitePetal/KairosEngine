//! Skin values the Preferences panel edits.
//!
//! A style struct lives with its component (`PreferencesPanelStyle`,
//! `ConsoleStyle`, ...). One shape crosses the `Action`/`Effect` boundary:
//! [`Skin`] — a whole typed style, erased behind [`StyleBlob`]. The typed
//! editor page draws its own copy of the owner's style and hands the value
//! back in one piece; core routes the box and never interprets it.
//!
//! There is deliberately no per-token value type (no `StyleValue`, no dotted
//! path): the widgets borrow the tokens directly (`NumberField<N>`,
//! `EnumField<E>`), so a single token never has to be erased to cross the
//! boundary. What cannot cross is the *page*, which needs its own copy while
//! the owner keeps drawing — hence one opaque whole-value payload.
//!
//! This crate stays free of egui and IO.

use std::any::Any;

/// A whole style value on its way back to its owner.
///
/// This is the one payload core does not interpret: the type lives in the
/// component crate (plain data, no egui, no IO), and core only routes the box
/// to the panel instance or shared registry that owns it. `Action`/`Effect`
/// derive `Clone + Debug + PartialEq`, so the blob carries those three
/// operations instead of being inspectable data.
pub trait StyleBlob: Any {
    /// One copy of this value, still erased.
    fn clone_blob(&self) -> Box<dyn StyleBlob>;

    /// The value as an `Any`, for the owner's downcast.
    fn as_any(&self) -> &dyn Any;

    /// The value as an owned `Any`, for the owner's replace.
    fn into_any(self: Box<Self>) -> Box<dyn Any>;

    /// Structural equality with another style value.
    fn equals(&self, other: &dyn StyleBlob) -> bool;

    /// The value's type, for logs and panic messages.
    fn debug_blob(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

/// A style value crossing the `Action`/`Effect` boundary.
///
/// The page draws a typed copy of the style and hands the whole value back; the
/// owner downcasts it to the concrete type it owns. Comparing two `Skin`s
/// compares the styles, so action lists in tests still read as values.
pub struct Skin(Box<dyn StyleBlob>);

impl Skin {
    /// Wrap a concrete style value.
    pub fn new(value: impl StyleBlob + 'static) -> Self {
        Self(Box::new(value))
    }

    /// Wrap a value that is already erased.
    pub fn from_blob(value: Box<dyn StyleBlob>) -> Self {
        Self(value)
    }

    /// The erased value, for an owner that downcasts it.
    pub fn into_any(self) -> Box<dyn Any> {
        self.0.into_any()
    }
}

impl Clone for Skin {
    fn clone(&self) -> Self {
        Self(self.0.clone_blob())
    }
}

impl std::fmt::Debug for Skin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.debug_blob(f)
    }
}

impl PartialEq for Skin {
    fn eq(&self, other: &Self) -> bool {
        self.0.equals(other.0.as_ref())
    }
}

/// Stable id of a shared style: appearance owned by the editor shell rather than
/// by one panel instance (the toolbar, the color picker).
///
/// The value is a plain string for the same reason PanelId is: adding a shared
/// style must not change core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedStyleId(pub &'static str);

impl std::fmt::Display for SharedStyleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
