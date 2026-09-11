//! Asset metadata: the `.meta` sidecars that say which loader handles an asset
//! and with what settings.
//!
//! Every asset file may have a sidecar beside it — `foo` → `foo.meta`, see
//! [`get_meta_path`](crate::next::io::get_meta_path) — carrying an
//! [`AssetMeta`]. The sidecar records an [`AssetAction`]:
//!
//! - `Load { loader, settings }`: the named loader reads the file.
//! - `Process { processor, settings }`: a processor transforms it first.
//! - `Ignore`: the asset is skipped.
//!
//! The loader is named by its fully-qualified `std::any::type_name`, because
//! kairos dropped the `TypePath` bound from `Asset` (see [`loader_name`]).
//!
//! Sidecars are **RON**: [`AssetAction`] is a tagged enum, which RON expresses
//! natively where TOML would need a workaround (ADR 0002). The per-asset-type
//! TOML wrappers of the legacy stack are retired in favour of this one format.
//!
//! Only `Ignore` has behaviour in the new core so far; the processing pipeline
//! is deferred, so `Process` exists as a type but nothing consumes it.

use core::any::Any;
use std::collections::HashSet;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::next::path::AssetPath;

/// The version of the `.meta` format. Bump it on any breaking change to the
/// sidecar layout or its hashing.
pub const META_FORMAT_VERSION: &str = "1.0";

/// A mutation applied to a loaded [`AssetMetaDyn`], used to override a
/// loader's settings without rewriting the sidecar.
pub type MetaTransform = Box<dyn Fn(&mut dyn AssetMetaDyn) + Send + Sync>;

/// A content hash, as recorded by the (deferred) asset processor.
pub type AssetHash = [u8; 32];

/// Marker for the settings types a loader, processor, or saver accepts.
///
/// Any `Send + Sync + 'static` type is settings, so implementors write nothing.
/// The trait exists to name that bound and to allow downcasting through
/// [`dyn Settings`](Settings#impl-dyn-Settings).
pub trait Settings: Any + Send + Sync {}
impl<T: Any + Send + Sync> Settings for T {}

impl dyn Settings {
    /// Whether this settings value is of type `S`.
    pub fn is<S: Settings>(&self) -> bool {
        (self as &dyn Any).is::<S>()
    }

    /// Downcasts this settings value to `S`, if it is one.
    pub fn downcast_ref<S: Settings>(&self) -> Option<&S> {
        (self as &dyn Any).downcast_ref::<S>()
    }

    /// Downcasts this settings value to `S` mutably, if it is one.
    pub fn downcast_mut<S: Settings>(&mut self) -> Option<&mut S> {
        (self as &mut dyn Any).downcast_mut::<S>()
    }
}

/// The name a loader is recorded under in a `.meta` sidecar.
///
/// kairos's assets and loaders carry no `TypePath`, so the sidecar stores the
/// fully-qualified [`std::any::type_name`]. That value is not stable across
/// compiler versions, which is acceptable for a sidecar written and read by
/// the same build.
pub fn loader_name<L>() -> &'static str {
    core::any::type_name::<L>()
}

/// How the asset system should handle one asset file.
///
/// The generic parameters are the loader's and processor's settings types;
/// both are `()` when that action is unused. This mirrors bevy's
/// `AssetAction<L::Settings, P::Settings>` without requiring the loader and
/// processor traits here — those land with the loader and processor tickets.
#[derive(Serialize, Deserialize)]
pub enum AssetAction<LoaderSettings, ProcessSettings> {
    /// Load the asset with the named loader and these settings.
    Load {
        /// The loader's [`loader_name`].
        loader: String,
        /// Settings handed to the loader.
        settings: LoaderSettings,
    },
    /// Process the asset with the named processor and these settings.
    ///
    /// Only the type surface exists for now; the processor itself is deferred.
    Process {
        /// The processor's name.
        processor: String,
        /// Settings handed to the processor.
        settings: ProcessSettings,
    },
    /// Do not load the asset.
    Ignore,
}

impl<LoaderSettings, ProcessSettings> AssetAction<LoaderSettings, ProcessSettings> {
    /// The loader's name, for [`AssetAction::Load`].
    pub fn loader_name(&self) -> Option<&str> {
        match self {
            AssetAction::Load { loader, .. } => Some(loader),
            _ => None,
        }
    }

    /// The processor's name, for [`AssetAction::Process`].
    pub fn processor_name(&self) -> Option<&str> {
        match self {
            AssetAction::Process { processor, .. } => Some(processor),
            _ => None,
        }
    }

    /// Whether this action is [`AssetAction::Ignore`].
    pub fn is_ignore(&self) -> bool {
        matches!(self, AssetAction::Ignore)
    }
}

/// The full contents of a `.meta` sidecar.
///
/// Serialize it with [`AssetMetaDyn::serialize`] and parse it with
/// [`AssetMeta::deserialize`]. When only the loader or processor name is needed
/// (before its settings type is known), parse
/// [`AssetMetaMinimal`] instead.
#[derive(Serialize, Deserialize)]
pub struct AssetMeta<LoaderSettings, ProcessSettings> {
    /// The version of the meta format, for migration checks.
    pub meta_format_version: String,
    /// Written by the asset processor after it processes the asset. It should
    /// not appear in hand-authored sidecars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_info: Option<ProcessedInfo>,
    /// How this asset should be handled.
    pub asset: AssetAction<LoaderSettings, ProcessSettings>,
}

impl<LoaderSettings, ProcessSettings> AssetMeta<LoaderSettings, ProcessSettings> {
    /// Creates a meta at the current [`META_FORMAT_VERSION`] with no processed
    /// info.
    pub fn new(asset: AssetAction<LoaderSettings, ProcessSettings>) -> Self {
        Self {
            meta_format_version: META_FORMAT_VERSION.to_string(),
            processed_info: None,
            asset,
        }
    }
}

impl<LoaderSettings: DeserializeOwned, ProcessSettings: DeserializeOwned>
    AssetMeta<LoaderSettings, ProcessSettings>
{
    /// Deserializes a sidecar from its RON bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, DeserializeMetaError> {
        Ok(ron::de::from_bytes(bytes)?)
    }
}

/// A type-erased [`AssetMeta`], for handling sidecars before the loader and
/// processor types are known.
///
/// `AssetMetaDyn` is also a [`Any`], so a caller can recover the concrete
/// [`AssetMeta`] when it needs the typed settings.
pub trait AssetMetaDyn: Any + Send + Sync {
    /// The loader settings, for an [`AssetAction::Load`].
    fn loader_settings(&self) -> Option<&dyn Settings>;
    /// The loader settings mutably, for an [`AssetAction::Load`].
    fn loader_settings_mut(&mut self) -> Option<&mut dyn Settings>;
    /// The processor settings, for an [`AssetAction::Process`].
    fn process_settings(&self) -> Option<&dyn Settings>;
    /// Serializes this meta as pretty RON.
    fn serialize(&self) -> Vec<u8>;
    /// The processed info, if this meta was written by the processor.
    fn processed_info(&self) -> &Option<ProcessedInfo>;
    /// The processed info mutably.
    fn processed_info_mut(&mut self) -> &mut Option<ProcessedInfo>;
}

impl<LoaderSettings, ProcessSettings> AssetMetaDyn
    for AssetMeta<LoaderSettings, ProcessSettings>
where
    LoaderSettings: Settings + Serialize,
    ProcessSettings: Settings + Serialize,
{
    fn loader_settings(&self) -> Option<&dyn Settings> {
        match &self.asset {
            AssetAction::Load { settings, .. } => Some(settings),
            _ => None,
        }
    }

    fn loader_settings_mut(&mut self) -> Option<&mut dyn Settings> {
        match &mut self.asset {
            AssetAction::Load { settings, .. } => Some(settings),
            _ => None,
        }
    }

    fn process_settings(&self) -> Option<&dyn Settings> {
        match &self.asset {
            AssetAction::Process { settings, .. } => Some(settings),
            _ => None,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        ron::ser::to_string_pretty(
            self,
            // Hard-code \n so the output is stable across platforms.
            ron::ser::PrettyConfig::default().new_line("\n"),
        )
        .expect("AssetMeta is serializable")
        .into_bytes()
    }

    fn processed_info(&self) -> &Option<ProcessedInfo> {
        &self.processed_info
    }

    fn processed_info_mut(&mut self) -> &mut Option<ProcessedInfo> {
        &mut self.processed_info
    }
}

/// A leaner [`AssetMeta`] that can be parsed before the loader's settings type
/// is known.
#[derive(Serialize, Deserialize)]
pub struct AssetMetaMinimal {
    /// How the asset should be handled, without settings.
    pub asset: AssetActionMinimal,
}

impl AssetMetaMinimal {
    /// Deserializes a minimal sidecar from its RON bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, DeserializeMetaError> {
        ron::de::from_bytes(bytes).map_err(DeserializeMetaError::DeserializeMinimal)
    }
}

/// A settings-less [`AssetAction`], for discovering which loader or processor
/// handles an asset.
#[derive(Serialize, Deserialize)]
pub enum AssetActionMinimal {
    /// The asset is loaded by the named loader.
    Load {
        /// The loader's [`loader_name`].
        loader: String,
    },
    /// The asset is processed by the named processor.
    Process {
        /// The processor's name.
        processor: String,
    },
    /// The asset is ignored.
    Ignore,
}

/// Information written by the asset processor about a processed asset.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct ProcessedInfo {
    /// Hash of the asset bytes and the asset's `.meta`.
    pub hash: AssetHash,
    /// Hash of the asset bytes, the `.meta`, and the `full_hash` of every
    /// process dependency.
    pub full_hash: AssetHash,
    /// The process dependencies used to produce this asset.
    pub process_dependencies: Vec<ProcessDependencyInfo>,
}

/// One dependency used to process an asset.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessDependencyInfo {
    /// The dependency's `full_hash`.
    pub full_hash: AssetHash,
    /// The dependency's path.
    pub path: AssetPath<'static>,
}

/// A leaner [`ProcessedInfo`] that can be read on its own.
#[derive(Serialize, Deserialize)]
pub struct ProcessedInfoMinimal {
    /// The processed info, if present.
    pub processed_info: Option<ProcessedInfo>,
}

/// Whether, and for which paths, an asset's `.meta` sidecar is consulted.
///
/// Defaults to [`Always`](AssetMetaCheck::Always): read the sidecar when one
/// exists and fall back to the loader's default meta when it does not.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum AssetMetaCheck {
    /// Always look for a sidecar.
    #[default]
    Always,
    /// Only look for a sidecar at the listed paths.
    Paths(HashSet<AssetPath<'static>>),
    /// Never look for a sidecar; always use the loader's default meta.
    Never,
}

/// An error from parsing a `.meta` sidecar.
#[derive(Debug)]
#[non_exhaustive]
pub enum DeserializeMetaError {
    /// The typed meta could not be deserialized.
    DeserializeSettings(ron::error::SpannedError),
    /// The minimal meta could not be deserialized.
    DeserializeMinimal(ron::error::SpannedError),
}

impl From<ron::error::SpannedError> for DeserializeMetaError {
    fn from(error: ron::error::SpannedError) -> Self {
        Self::DeserializeSettings(error)
    }
}

impl core::fmt::Display for DeserializeMetaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DeserializeSettings(error) => {
                write!(f, "Failed to deserialize asset meta: {error:?}")
            }
            Self::DeserializeMinimal(error) => {
                write!(f, "Failed to deserialize minimal asset meta: {error:?}")
            }
        }
    }
}

impl std::error::Error for DeserializeMetaError {}

/// Applies `settings` to the loader settings inside `meta`, if their types
/// match.
///
/// A type mismatch is a configuration error — the [`MetaTransform`] was aimed
/// at a different loader — so it is silently skipped rather than panicking.
pub fn meta_transform_settings<S: Settings>(
    meta: &mut dyn AssetMetaDyn,
    settings: &(impl Fn(&mut S) + Send + Sync + 'static),
) {
    if let Some(loader_settings) = meta.loader_settings_mut()
        && let Some(loader_settings) = loader_settings.downcast_mut::<S>()
    {
        settings(loader_settings);
    }
}

/// Builds a [`MetaTransform`] that applies `settings` to a loader's settings.
pub fn loader_settings_meta_transform<S: Settings>(
    settings: impl Fn(&mut S) + Send + Sync + 'static,
) -> MetaTransform {
    Box::new(move |meta| meta_transform_settings(meta, &settings))
}
