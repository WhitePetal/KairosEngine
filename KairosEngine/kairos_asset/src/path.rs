//! Asset paths: the address of an asset inside the asset sources.
//!
//! An [`AssetPath`] has three parts:
//!
//! - [`source`](AssetPath::source): the [`AssetSourceId`] to resolve against.
//!   When unset, the default source is used (the process working directory, see
//!   ADR 0004).
//! - [`path`](AssetPath::path): the path within that source.
//! - [`label`](AssetPath::label): an optional named sub-asset produced by a
//!   loader, addressed by appending `#label` to the same path.
//!
//! Paths are generally written as strings (`"res/models/Suzanne.mesh"`,
//! `"custom://a/b.ron#Mesh0"`) and parsed with [`AssetPath::parse`]. The
//! [`From`] impls exist so literals can be used directly, but they still go
//! through the same parser: kairos, like bevy, treats `#` and `://` as syntax.
//!
//! This mirrors `bevy_asset`'s `AssetPath`, including its internal [`CowArc`]
//! storage: owned path/label values are `Arc`-ed, so cloning a path — which the
//! loader does on every request — is a reference-count bump, while a static
//! literal stays static instead of being promoted to an allocation.

use core::fmt::{Debug, Display};
use std::path::{Path, PathBuf};

use atomicow::CowArc;
use serde::{Deserialize, Serialize, de::Visitor};

use crate::io::AssetSourceId;

/// A path to an asset in the asset sources.
///
/// See the [module docs](self) for the three parts. `AssetPath` compares and
/// hashes by its parsed components, not by its string form, so `"a/b"` and
/// `"./a/b"` are distinct.
#[derive(Eq, PartialEq, Hash, Clone, Default)]
pub struct AssetPath<'a> {
    source: AssetSourceId<'a>,
    path: CowArc<'a, Path>,
    label: Option<CowArc<'a, str>>,
}

impl<'a> Debug for AssetPath<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self, f)
    }
}

impl<'a> Display for AssetPath<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let AssetSourceId::Name(name) = self.source() {
            write!(f, "{name}://")?;
        }
        write!(f, "{}", self.path.display())?;
        if let Some(label) = &self.label {
            write!(f, "#{label}")?;
        }
        Ok(())
    }
}

/// An error returned when a string cannot be parsed as an [`AssetPath`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseAssetPathError {
    /// The source section contains the label delimiter `#`, e.g.
    /// `bad#source://file.test`.
    InvalidSourceSyntax,
    /// The label section contains the source delimiter `://`, e.g.
    /// `source://file.test#bad://label`.
    InvalidLabelSyntax,
    /// A `://` delimiter has no source name before it, e.g. `://file.test`.
    MissingSource,
    /// A `#` delimiter has no label after it, e.g. `file.test#`.
    MissingLabel,
}

impl Display for ParseAssetPathError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidSourceSyntax => {
                write!(f, "Asset source must not contain a `#` character")
            }
            Self::InvalidLabelSyntax => {
                write!(f, "Asset label must not contain a `://` substring")
            }
            Self::MissingSource => write!(
                f,
                "Asset source must be at least one character. Either specify the source before \
                 the '://' or remove the `://`"
            ),
            Self::MissingLabel => write!(
                f,
                "Asset label must be at least one character. Either specify the label after the \
                 '#' or remove the '#'"
            ),
        }
    }
}

impl std::error::Error for ParseAssetPathError {}

impl<'a> AssetPath<'a> {
    /// Parses an asset path from its string form.
    ///
    /// The accepted forms are:
    /// - an asset at the root: `"scene.gltf"`
    /// - an asset nested in folders: `"some/path/scene.gltf"`
    /// - an asset with a label: `"some/path/scene.gltf#Mesh0"`
    /// - an asset from a named source: `"custom://some/path/scene.gltf#Mesh0"`
    ///
    /// # Panics
    ///
    /// Panics if the path is malformed. Use [`AssetPath::try_parse`] for the
    /// fallible variant.
    pub fn parse(asset_path: &'a str) -> AssetPath<'a> {
        Self::try_parse(asset_path).unwrap()
    }

    /// Parses an asset path from its string form, returning a
    /// [`ParseAssetPathError`] when the input is malformed.
    ///
    /// See [`AssetPath::parse`] for the accepted forms.
    pub fn try_parse(asset_path: &'a str) -> Result<AssetPath<'a>, ParseAssetPathError> {
        let (source, path, label) = Self::parse_internal(asset_path)?;
        Ok(Self {
            source: match source {
                Some(source) => AssetSourceId::Name(CowArc::Borrowed(source)),
                None => AssetSourceId::Default,
            },
            path: CowArc::Borrowed(path),
            label: label.map(CowArc::Borrowed),
        })
    }

    /// Splits a raw string into its `(source, path, label)` parts.
    ///
    /// A `://` opens a source, the last `#` opens a label, and the path is
    /// whatever sits between them. Nesting is rejected: a `#` inside the source
    /// and a `://` inside the label are both errors.
    fn parse_internal(
        asset_path: &str,
    ) -> Result<(Option<&str>, &Path, Option<&str>), ParseAssetPathError> {
        let chars = asset_path.char_indices();
        let mut source_range = None;
        let mut path_range = 0..asset_path.len();
        let mut label_range = None;

        let mut source_delimiter_chars_matched = 0;
        let mut last_found_source_index = 0;
        for (index, char) in chars {
            match char {
                ':' => {
                    source_delimiter_chars_matched = 1;
                }
                '/' => match source_delimiter_chars_matched {
                    1 => {
                        source_delimiter_chars_matched = 2;
                    }
                    2 => {
                        // A second `/` closes the `://` delimiter. Record the
                        // first source we see and reject a `#` that came before
                        // it (that would mean a `#` inside the source).
                        if source_range.is_none() {
                            if label_range.is_some() {
                                return Err(ParseAssetPathError::InvalidSourceSyntax);
                            }
                            source_range = Some(0..index - 2);
                            path_range.start = index + 1;
                        }
                        last_found_source_index = index - 2;
                        source_delimiter_chars_matched = 0;
                    }
                    _ => {}
                },
                '#' => {
                    path_range.end = index;
                    label_range = Some(index + 1..asset_path.len());
                    source_delimiter_chars_matched = 0;
                }
                _ => {
                    source_delimiter_chars_matched = 0;
                }
            }
        }

        if let Some(range) = label_range.clone() {
            // A `://` after the `#` means the label contains a source delimiter.
            if range.start <= last_found_source_index {
                return Err(ParseAssetPathError::InvalidLabelSyntax);
            }
        }

        let source = match source_range {
            Some(source_range) => {
                if source_range.is_empty() {
                    return Err(ParseAssetPathError::MissingSource);
                }
                Some(&asset_path[source_range])
            }
            None => None,
        };

        let label = match label_range {
            Some(label_range) => {
                if label_range.is_empty() {
                    return Err(ParseAssetPathError::MissingLabel);
                }
                Some(&asset_path[label_range])
            }
            None => None,
        };

        let path = Path::new(&asset_path[path_range]);
        Ok((source, path, label))
    }

    /// Creates an owned [`AssetPath`] from a [`PathBuf`], using the default source.
    #[inline]
    pub fn from_path_buf(path_buf: PathBuf) -> AssetPath<'a> {
        AssetPath {
            path: CowArc::from(path_buf),
            source: AssetSourceId::Default,
            label: None,
        }
    }

    /// Creates a borrowed [`AssetPath`] from a [`Path`], using the default source.
    #[inline]
    pub fn from_path(path: &'a Path) -> AssetPath<'a> {
        AssetPath {
            path: CowArc::Borrowed(path),
            source: AssetSourceId::Default,
            label: None,
        }
    }

    /// The source this path resolves against. [`AssetSourceId::Default`] means
    /// the default source.
    #[inline]
    pub fn source(&self) -> &AssetSourceId<'_> {
        &self.source
    }

    /// The sub-asset label, if one is set.
    #[inline]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The sub-asset label as an owned [`CowArc`], if one is set.
    #[inline]
    pub fn label_cow(&self) -> Option<CowArc<'a, str>> {
        self.label.clone()
    }

    /// The path of the asset within its source.
    #[inline]
    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    /// This path without its label.
    #[inline]
    pub fn without_label(&self) -> AssetPath<'_> {
        Self {
            source: self.source.clone(),
            path: self.path.clone(),
            label: None,
        }
    }

    /// Removes the label from this path, if one is set.
    #[inline]
    pub fn remove_label(&mut self) {
        self.label = None;
    }

    /// Takes the label out of this path, if one is set.
    #[inline]
    pub fn take_label(&mut self) -> Option<CowArc<'a, str>> {
        self.label.take()
    }

    /// Returns this path with `label`, replacing any label already set.
    #[inline]
    pub fn with_label(self, label: impl Into<CowArc<'a, str>>) -> AssetPath<'a> {
        AssetPath {
            source: self.source,
            path: self.path,
            label: Some(label.into()),
        }
    }

    /// Returns this path resolved against `source`, replacing any source
    /// already set.
    #[inline]
    pub fn with_source(self, source: impl Into<AssetSourceId<'a>>) -> AssetPath<'a> {
        AssetPath {
            source: source.into(),
            path: self.path,
            label: self.label,
        }
    }

    /// The parent folder of this path, or `None` at the root.
    ///
    /// The label is dropped: a label names a sub-asset of the file, so the
    /// parent folder is the same as the unlabeled path's.
    pub fn parent(&self) -> Option<AssetPath<'a>> {
        let path = match &self.path {
            CowArc::Borrowed(path) => CowArc::Borrowed(path.parent()?),
            CowArc::Static(path) => CowArc::Static(path.parent()?),
            CowArc::Owned(path) => CowArc::from(path.parent()?.to_path_buf()),
        };
        Some(AssetPath {
            source: self.source.clone(),
            label: None,
            path,
        })
    }

    /// Converts this into an owned value, cloning anything borrowed.
    pub fn into_owned(self) -> AssetPath<'static> {
        AssetPath {
            source: self.source.into_owned(),
            path: self.path.into_owned(),
            label: self.label.map(CowArc::into_owned),
        }
    }

    /// Clones this into an owned value. Equivalent to
    /// `self.clone().into_owned()`.
    #[inline]
    pub fn clone_owned(&self) -> AssetPath<'static> {
        self.clone().into_owned()
    }

    /// Resolves `path` relative to `self`, as a path inside the source.
    ///
    /// - A label-only `path` (default source, empty path, label set) replaces
    ///   `self`'s label.
    /// - A `path` beginning with `/` is treated as rooted at the asset source,
    ///   not the filesystem.
    /// - An explicit source in `path` replaces the base source.
    /// - Relative segments are concatenated and normalized, keeping extra `..`
    ///   when the base underflows.
    ///
    /// See also [`AssetPath::resolve_str`].
    pub fn resolve(&self, path: &AssetPath<'_>) -> AssetPath<'static> {
        self.resolve_with(path, false)
    }

    /// Like [`AssetPath::resolve`] but using embedded (RFC 1808) semantics: a
    /// relative path is appended to the parent folder of the base, and a base
    /// that looks like a file has its last segment replaced.
    pub fn resolve_embed(&self, path: &AssetPath<'_>) -> AssetPath<'static> {
        self.resolve_with(path, true)
    }

    fn resolve_with(&self, path: &AssetPath<'_>, embedded: bool) -> AssetPath<'static> {
        let is_label_only = matches!(path.source(), AssetSourceId::Default)
            && path.path().as_os_str().is_empty()
            && path.label().is_some();

        if is_label_only {
            return self
                .clone_owned()
                .with_label(path.label().unwrap().to_owned());
        }

        let explicit_source = match path.source() {
            AssetSourceId::Default => None,
            AssetSourceId::Name(name) => Some(name.as_ref()),
        };
        self.resolve_from_parts(embedded, explicit_source, path.path(), path.label())
    }

    /// Parses `path` as an [`AssetPath`], then resolves it relative to `self`.
    pub fn resolve_str(&self, path: &str) -> Result<AssetPath<'static>, ParseAssetPathError> {
        self.resolve_internal(path, false)
    }

    /// Parses `path` as an [`AssetPath`], then resolves it relative to `self`
    /// using embedded (RFC 1808) semantics.
    pub fn resolve_embed_str(&self, path: &str) -> Result<AssetPath<'static>, ParseAssetPathError> {
        self.resolve_internal(path, true)
    }

    fn resolve_from_parts(
        &self,
        embedded: bool,
        source: Option<&str>,
        rpath: &Path,
        rlabel: Option<&str>,
    ) -> AssetPath<'static> {
        let mut base_path = PathBuf::from(self.path());
        if embedded && !self.path.to_string_lossy().ends_with('/') {
            // The base names a file, so an embedded relative reference resolves
            // against its folder. An empty base is left alone (RFC 1808).
            base_path.pop();
        }

        let mut is_absolute = false;
        let rpath = match rpath.strip_prefix("/") {
            Ok(path) => {
                is_absolute = true;
                path
            }
            Err(_) => rpath,
        };

        let mut result_path = if !is_absolute && source.is_none() {
            base_path
        } else {
            PathBuf::new()
        };
        result_path.push(rpath);
        result_path = normalize_path(result_path.as_path());

        AssetPath {
            source: match source {
                Some(source) => AssetSourceId::Name(CowArc::from(source.to_owned())),
                None => self.source.clone_owned(),
            },
            path: CowArc::from(result_path),
            label: rlabel.map(|label| CowArc::from(label.to_owned())),
        }
    }

    fn resolve_internal(
        &self,
        path: &str,
        embedded: bool,
    ) -> Result<AssetPath<'static>, ParseAssetPathError> {
        if let Some(label) = path.strip_prefix('#') {
            // A label-only string replaces the base's label outright.
            Ok(self.clone_owned().with_label(label.to_owned()))
        } else {
            let (source, rpath, rlabel) = AssetPath::parse_internal(path)?;
            Ok(self.resolve_from_parts(embedded, source, rpath, rlabel))
        }
    }

    /// The full extension, including any earlier dots: `"config.ron"` for
    /// `"my_asset.config.ron"`. Query strings after a `?` are stripped.
    pub fn get_full_extension(&self) -> Option<&str> {
        let file_name = self.path().file_name()?.to_str()?;
        let index = file_name.find('.')?;
        let mut extension = &file_name[index + 1..];

        if let Some(offset) = extension.find('?') {
            extension = &extension[..offset];
        }

        Some(extension)
    }

    /// The extension after the last dot: `"ron"` for `"my_asset.config.ron"`.
    /// Query strings after a `?` are stripped.
    pub fn get_extension(&self) -> Option<&str> {
        let full_extension = self.get_full_extension()?;
        Some(match full_extension.rfind(".") {
            None => full_extension,
            Some(index) => &full_extension[(index + 1)..],
        })
    }

    /// Whether this path escapes its [`AssetSource`](crate::io::AssetSource)
    /// root, by starting at the filesystem root or climbing above it with `..`.
    ///
    /// ```
    /// # use kairos_asset::AssetPath;
    /// assert!(!AssetPath::parse("gui/thingy.png").is_unapproved());
    /// assert!(AssetPath::parse("../thingy.png").is_unapproved());
    /// assert!(AssetPath::parse("folder/../../thingy.png").is_unapproved());
    /// assert!(AssetPath::parse("/home/thingy.png").is_unapproved());
    /// ```
    pub fn is_unapproved(&self) -> bool {
        use std::path::Component;
        let mut simplified = PathBuf::new();
        for component in self.path.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => return true,
                Component::CurDir => {}
                Component::ParentDir => {
                    if !simplified.pop() {
                        return true;
                    }
                }
                Component::Normal(os_str) => simplified.push(os_str),
            }
        }
        false
    }
}

// Only implemented for `'static` so a literal stays borrowed rather than
// allocating. Prefer this over `parse` for string literals.
impl From<&'static str> for AssetPath<'static> {
    #[inline]
    fn from(asset_path: &'static str) -> Self {
        let (source, path, label) = Self::parse_internal(asset_path).unwrap();
        AssetPath {
            source: source.into(),
            path: CowArc::Static(path),
            label: label.map(CowArc::Static),
        }
    }
}

impl<'a> From<&'a String> for AssetPath<'a> {
    #[inline]
    fn from(asset_path: &'a String) -> Self {
        AssetPath::parse(asset_path.as_str())
    }
}

impl From<String> for AssetPath<'static> {
    #[inline]
    fn from(asset_path: String) -> Self {
        AssetPath::parse(asset_path.as_str()).into_owned()
    }
}

impl From<&'static Path> for AssetPath<'static> {
    #[inline]
    fn from(path: &'static Path) -> Self {
        Self {
            source: AssetSourceId::Default,
            path: CowArc::Static(path),
            label: None,
        }
    }
}

impl From<PathBuf> for AssetPath<'static> {
    #[inline]
    fn from(path: PathBuf) -> Self {
        Self {
            source: AssetSourceId::Default,
            path: CowArc::from(path),
            label: None,
        }
    }
}

impl<'a, 'b> From<&'a AssetPath<'b>> for AssetPath<'b> {
    fn from(value: &'a AssetPath<'b>) -> Self {
        value.clone()
    }
}

impl<'a> From<AssetPath<'a>> for PathBuf {
    fn from(value: AssetPath<'a>) -> Self {
        value.path().to_path_buf()
    }
}

impl<'a> Serialize for AssetPath<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AssetPath<'static> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_string(AssetPathVisitor)
    }
}

struct AssetPathVisitor;

impl<'de> Visitor<'de> for AssetPathVisitor {
    type Value = AssetPath<'static>;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter.write_str("string AssetPath")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match AssetPath::try_parse(v) {
            Ok(path) => Ok(path.into_owned()),
            Err(err) => Err(E::custom(err)),
        }
    }
}

/// Normalizes a path by collapsing `.` and `..` segments where possible, per
/// [RFC 1808](https://datatracker.ietf.org/doc/html/rfc1808). A `..` that
/// underflows the path is preserved.
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut result_path = PathBuf::new();
    for elt in path.iter() {
        if elt == "." {
            // Skip
        } else if elt == ".." {
            if result_path.file_name().is_some() {
                assert!(result_path.pop());
            } else {
                result_path.push(elt);
            }
        } else {
            result_path.push(elt);
        }
    }
    result_path
}
