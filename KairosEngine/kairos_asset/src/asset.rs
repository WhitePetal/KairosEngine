//! The [`Asset`] trait and its dependency visitor.
//!
//! Unlike `bevy_asset`, `kairos` has no `TypePath`/reflection machinery, so
//! [`Asset`] is simply a sendable, `'static` value whose handles can be
//! discovered by [`VisitAssetDependencies`]. There is no `#[derive(Asset)]`
//! macro; implement the two traits directly.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use crate::handle::{Handle, UntypedHandle};
use crate::id::UntypedAssetId;

/// A loadable piece of content the engine owns and hands out by handle.
///
/// The trait itself has no methods: it is the bound that asset stores, loaders
/// and handles are built around. `Send + Sync + 'static` lets an asset cross
/// threads and live in a world resource; [`VisitAssetDependencies`] lets the
/// loader discover the other assets it depends on.
pub trait Asset: VisitAssetDependencies + Send + Sync + 'static {}

/// Visits the assets that a value depends on.
///
/// A dependency must finish loading before the value that depends on it is
/// considered ready. Implementations recurse into any handles the value holds.
/// The provided implementation visits nothing, so leaf assets can `impl
/// VisitAssetDependencies for MyAsset {}`.
pub trait VisitAssetDependencies {
    /// Calls `visit` once for each asset this value depends on.
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

impl<A: Asset> VisitAssetDependencies for Handle<A> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.id().untyped());
    }
}

impl VisitAssetDependencies for UntypedHandle {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.id());
    }
}

impl VisitAssetDependencies for UntypedAssetId {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(*self);
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for Option<V> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        if let Some(dependency) = self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for Box<V> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.as_ref().visit_dependencies(visit);
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for Vec<V> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for VecDeque<V> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies, const N: usize> VisitAssetDependencies for [V; N] {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for [V] {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies, S> VisitAssetDependencies for HashSet<V, S> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for BTreeSet<V> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<K, V: VisitAssetDependencies, S> VisitAssetDependencies for HashMap<K, V, S> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self.values() {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<K, V: VisitAssetDependencies> VisitAssetDependencies for BTreeMap<K, V> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for dependency in self.values() {
            dependency.visit_dependencies(visit);
        }
    }
}
