//! Provides the [`Name`] [`Component`], used for identifying an [`Entity`].

use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
    ops::Deref,
};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, Visitor},
};

use crate::{
    ecs::{
        component::{Component, Mutable},
        entity::Entity,
    },
    hash::FixedHashed,
};

/// A wrapper over Hashed. This exists to make Name("value".into()) possible, which plays nicely with contexts like the `bsn!` macro.
#[derive(Clone)]
pub struct HashedStr(FixedHashed<Cow<'static, str>>);

// TODO!
#[derive(Clone)]
pub struct Name(pub HashedStr);

impl Component for Name {
    const STORAGE_TYPE: super::component::StorageType = super::component::StorageType::Table;

    type Mutability = Mutable;
}

impl Default for Name {
    fn default() -> Self {
        Name::new("")
    }
}

impl From<&'static str> for HashedStr {
    fn from(value: &'static str) -> Self {
        Self(FixedHashed::new(Cow::Borrowed(value)))
    }
}

impl From<String> for HashedStr {
    fn from(value: String) -> Self {
        Self(FixedHashed::new(Cow::Owned(value)))
    }
}

impl Name {
    /// Creates a new [`Name`] from any string-like type.
    ///
    /// The internal hash will be computed immediately.
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(HashedStr(FixedHashed::new(name.into())))
    }

    /// Sets the entity's name.
    ///
    /// The internal hash will be re-computed.
    #[inline(always)]
    pub fn set(&mut self, name: impl Into<Cow<'static, str>>) {
        *self = Name::new(name);
    }

    /// Updates the name of the entity in place.
    ///
    /// This will allocate a new string if the name was previously
    /// created from a borrow.
    #[inline(always)]
    pub fn mutate(&mut self, func: impl FnOnce(&mut String)) {
        self.0.0.mutate(|cow_str| match cow_str {
            Cow::Borrowed(borrowed) => {
                let mut owned = borrowed.to_owned();
                func(&mut owned);
                *cow_str = Cow::Owned(owned);
            }
            Cow::Owned(owned) => {
                func(owned);
            }
        });
    }

    /// Gets the name of the entity as a `&str`.
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        &self.0.0
    }
    /// Get the precomputed hash of this names string, useful for raw entry operations on [`PreHashMap`](bevy_utils::PreHashMap)
    #[inline(always)]
    pub fn pre_hash(&self) -> u64 {
        self.0.0.hash()
    }
}

impl std::fmt::Display for Name {
    #[inline(always)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&*self.0.0, f)
    }
}

impl std::fmt::Debug for Name {
    #[inline(always)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0.0, f)
    }
}

/// Convenient query for giving a human friendly name to an entity.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Component)] pub struct Score(f32);
/// fn increment_score(mut scores: Query<(NameOrEntity, &mut Score)>) {
///     for (name, mut score) in &mut scores {
///         score.0 += 1.0;
///         if score.0.is_nan() {
///             log::error!("Score for {name} is invalid");
///         }
///     }
/// }
/// # bevy_ecs::system::assert_is_system(increment_score);
/// ```
///
/// # Implementation
///
/// The `Display` impl for `NameOrEntity` returns the `Name` where there is one
/// or {index}v{generation} for entities without one.
// TODO!
// #[derive(QueryData)]
// #[query_data(derive(Debug))]
pub struct NameOrEntity {
    /// A [`Name`] that the entity might have that is displayed if available.
    pub name: Option<&'static Name>,
    /// The unique identifier of the entity as a fallback.
    pub entity: Entity,
}

// impl<'w, 's> core::fmt::Display for NameOrEntityItem<'w, 's> {
//     #[inline(always)]
//     fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
//         match self.name {
//             Some(name) => core::fmt::Display::fmt(name, f),
//             None => core::fmt::Display::fmt(&self.entity, f),
//         }
//     }
// }

// Conversions from strings

impl From<&str> for Name {
    #[inline(always)]
    fn from(name: &str) -> Self {
        Name::new(name.to_owned())
    }
}

impl From<String> for Name {
    #[inline(always)]
    fn from(name: String) -> Self {
        Name::new(name)
    }
}

// Conversions to strings

impl AsRef<str> for Name {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        &self.0.0
    }
}

impl From<&Name> for String {
    #[inline(always)]
    fn from(val: &Name) -> String {
        val.as_str().to_owned()
    }
}

impl From<Name> for String {
    #[inline(always)]
    fn from(val: Name) -> String {
        val.as_str().to_owned()
    }
}

impl Hash for Name {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.0.0, state);
    }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        if self.0.0.hash() != other.0.0.hash() {
            // Makes the common case of two strings not been equal very fast
            return false;
        }

        self.0.0.eq(&other.0.0)
    }
}

impl Eq for Name {}

impl PartialOrd for Name {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Name {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.0.cmp(&other.0.0)
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(NameVisitor)
    }
}

struct NameVisitor;

impl<'de> Visitor<'de> for NameVisitor {
    type Value = Name;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter.write_str(core::any::type_name::<Name>())
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Name::new(v.to_string()))
    }

    fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(Name::new(v))
    }
}
