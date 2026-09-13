//! The processor registry: which [`Process`] implementations the processor
//! knows about, indexed by full name, short name, and default file extension.
//!
//! This is a deliberately small port of `bevy_asset`'s `Processors`. kairos has
//! no `TypePath`, so a processor is named by
//! [`processor_name`](crate::meta::processor_name) and its short name is the
//! final path segment of that name (generic arguments dropped). A short name
//! that collides with another processor's is recorded as ambiguous and only
//! resolvable by the fully-qualified name.
//!
//! The registry is the registration surface: `register_processor`,
//! `set_default_processor`, and the two lookups. The `AssetProcessor` (a later
//! slice) owns an instance and re-exposes these methods.

use std::sync::Arc;

use hashbrown::hash_map::Entry;
use kairos_collections::FixedHashMap as HashMap;
use thiserror::Error;

use super::process::{ErasedProcessor, Process, ProcessError};

/// Extracts a processor's short name from its [`std::any::type_name`].
///
/// The short name is the final path segment with any generic arguments
/// dropped, so `my_crate::MyProcessor<Foo>` becomes `MyProcessor`. It is the
/// key a `.meta` sidecar may use to address a processor without spelling out
/// its module path.
///
/// This is a deliberate kairos deviation from bevy's `TypePath::short_type_path`,
/// which keeps generic arguments: kairos derives names from `type_name`, where a
/// generic processor's arguments carry full module paths and make unusable short
/// names. Non-generic processors — the common case — match bevy exactly.
pub(crate) fn short_type_name(full_type_name: &str) -> &str {
    let without_generics = match full_type_name.find('<') {
        Some(index) => &full_type_name[..index],
        None => full_type_name,
    };
    without_generics
        .rsplit("::")
        .next()
        .unwrap_or(without_generics)
}

/// Every processor registered with the processor.
#[derive(Default)]
pub struct Processors {
    /// The processors, indexed by their full [`processor_name`](crate::meta::processor_name).
    type_name_to_processor: HashMap<&'static str, Arc<dyn ErasedProcessor>>,
    /// Which processor answers to each short name.
    short_name_to_processor: HashMap<&'static str, ShortNameProcessorEntry>,
    /// The full name of the default processor for each file extension.
    extension_to_default_processor: HashMap<Box<str>, &'static str>,
}

enum ShortNameProcessorEntry {
    /// Exactly one processor has this short name.
    Unique {
        /// The processor's full name.
        type_name: &'static str,
        /// The processor itself.
        processor: Arc<dyn ErasedProcessor>,
    },
    /// Several processors share this short name; they must be named fully.
    Ambiguous(Vec<&'static str>),
}

impl Processors {
    /// Registers `processor`, indexing it by full name and short name.
    ///
    /// Registering a second processor under a short name already in use marks
    /// that short name ambiguous, mirroring bevy.
    pub fn register_processor<P: Process>(&mut self, processor: P) {
        let processor: Arc<dyn ErasedProcessor> = Arc::new(processor);
        let type_name = processor.type_path();
        let short_name = processor.short_type_path();
        self.type_name_to_processor
            .insert(type_name, processor.clone());
        match self.short_name_to_processor.entry(short_name) {
            Entry::Vacant(entry) => {
                entry.insert(ShortNameProcessorEntry::Unique {
                    type_name,
                    processor,
                });
            }
            Entry::Occupied(mut entry) => match entry.get_mut() {
                ShortNameProcessorEntry::Unique {
                    type_name: first, ..
                } => {
                    let first = *first;
                    entry.insert(ShortNameProcessorEntry::Ambiguous(vec![first, type_name]));
                }
                ShortNameProcessorEntry::Ambiguous(names) => names.push(type_name),
            },
        }
    }

    /// Makes `P` the default processor for `extension`.
    ///
    /// `P` must have been registered with [`Processors::register_processor`]
    /// under the same name for the lookup to resolve it.
    pub fn set_default_processor<P: Process>(&mut self, extension: &str) {
        self.extension_to_default_processor
            .insert(extension.into(), crate::meta::processor_name::<P>());
    }

    /// The default processor for `extension`, if one is registered.
    pub fn get_default_processor(&self, extension: &str) -> Option<Arc<dyn ErasedProcessor>> {
        let type_name = self.extension_to_default_processor.get(extension)?;
        self.type_name_to_processor.get(type_name).cloned()
    }

    /// The processor named `processor_type_name` (full name or unambiguous
    /// short name).
    pub fn get_processor(
        &self,
        processor_type_name: &str,
    ) -> Result<Arc<dyn ErasedProcessor>, GetProcessorError> {
        if let Some(entry) = self.short_name_to_processor.get(processor_type_name) {
            return match entry {
                ShortNameProcessorEntry::Unique { processor, .. } => Ok(processor.clone()),
                ShortNameProcessorEntry::Ambiguous(names) => Err(GetProcessorError::Ambiguous {
                    processor_short_name: processor_type_name.to_owned(),
                    ambiguous_processor_names: names.clone(),
                }),
            };
        }
        self.type_name_to_processor
            .get(processor_type_name)
            .cloned()
            .ok_or_else(|| GetProcessorError::Missing(processor_type_name.to_owned()))
    }
}

/// An error from resolving a processor by name or short name.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum GetProcessorError {
    /// No processor is registered under the requested name.
    #[error("The processor '{0}' does not exist")]
    Missing(String),
    /// The requested short name matches several processors; name one fully.
    #[error(
        "The processor '{processor_short_name}' is ambiguous between several processors: {ambiguous_processor_names:?}"
    )]
    Ambiguous {
        /// The short name that was requested.
        processor_short_name: String,
        /// The full names of the conflicting processors.
        ambiguous_processor_names: Vec<&'static str>,
    },
}

impl From<GetProcessorError> for ProcessError {
    fn from(error: GetProcessorError) -> Self {
        match error {
            GetProcessorError::Missing(name) => Self::MissingProcessor(name),
            GetProcessorError::Ambiguous {
                processor_short_name,
                ambiguous_processor_names,
            } => Self::AmbiguousProcessor {
                processor_short_name,
                ambiguous_processor_names,
            },
        }
    }
}
