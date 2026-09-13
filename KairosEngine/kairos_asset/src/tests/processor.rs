//! Tests for the in-memory processed-asset graph.

use std::collections::HashSet;

use crate::meta::{ProcessDependencyInfo, ProcessedInfo};
use crate::path::AssetPath;
use crate::processor::{ProcessStatus, ProcessorAssetInfos};

/// A [`ProcessedInfo`] whose own hash and `full_hash` are `hash`/`full_hash`
/// bytes repeated, with the given dependencies.
fn processed_info(hash: u8, full_hash: u8, dependencies: &[(&'static str, u8)]) -> ProcessedInfo {
    ProcessedInfo {
        hash: [hash; 32],
        full_hash: [full_hash; 32],
        process_dependencies: dependencies
            .iter()
            .map(|(path, full_hash)| ProcessDependencyInfo {
                full_hash: [*full_hash; 32],
                path: AssetPath::from(*path),
            })
            .collect(),
    }
}

#[test]
fn the_three_states_are_representable() {
    let mut infos = ProcessorAssetInfos::default();
    let path = AssetPath::from("asset.bin");

    // Absent from the graph means non-existent.
    assert!(infos.get(&path).is_none());
    assert_ne!(ProcessStatus::NonExistent, ProcessStatus::Processed);
    assert_ne!(ProcessStatus::Failed, ProcessStatus::Processed);

    infos.mark_failed(path.clone());
    assert_eq!(
        infos.get(&path).unwrap().status(),
        Some(ProcessStatus::Failed)
    );

    infos.insert_processed(path.clone(), processed_info(1, 1, &[]));
    assert_eq!(
        infos.get(&path).unwrap().status(),
        Some(ProcessStatus::Processed)
    );

    infos.remove(&path);
    assert!(infos.get(&path).is_none(), "a removed asset is non-existent");
}

#[test]
fn the_graph_is_queryable_in_both_directions() {
    let mut infos = ProcessorAssetInfos::default();
    let a = AssetPath::from("a.bin");
    let b = AssetPath::from("b.bin");
    let c = AssetPath::from("c.bin");

    // b depends on a; c depends on a and b.
    infos.insert_processed(a.clone(), processed_info(1, 10, &[]));
    infos.insert_processed(b.clone(), processed_info(2, 20, &[("a.bin", 10)]));
    infos.insert_processed(c.clone(), processed_info(3, 30, &[("a.bin", 10), ("b.bin", 20)]));

    // Forward: c's own recorded dependencies.
    let c_dependencies: HashSet<AssetPath<'static>> = infos
        .get(&c)
        .unwrap()
        .processed_info()
        .unwrap()
        .process_dependencies
        .iter()
        .map(|dependency| dependency.path.clone())
        .collect();
    assert_eq!(c_dependencies, [a.clone(), b.clone()].into_iter().collect());

    // Reverse: a's dependents are every asset processed against it.
    let a_dependents: HashSet<AssetPath<'static>> = infos.get(&a).unwrap().dependents().clone();
    assert_eq!(a_dependents, [b.clone(), c.clone()].into_iter().collect());

    let b_dependents: HashSet<AssetPath<'static>> = infos.get(&b).unwrap().dependents().clone();
    assert_eq!(b_dependents, [c.clone()].into_iter().collect());
}

#[test]
fn reprocessing_an_asset_returns_its_dependents_for_requeueing() {
    let mut infos = ProcessorAssetInfos::default();
    let a = AssetPath::from("a.bin");
    let b = AssetPath::from("b.bin");

    infos.insert_processed(a.clone(), processed_info(1, 10, &[]));
    infos.insert_processed(b.clone(), processed_info(2, 20, &[("a.bin", 10)]));

    let requeued = infos.insert_processed(a.clone(), processed_info(1, 11, &[]));
    assert_eq!(requeued, vec![b.clone()]);
}

#[test]
fn a_dependent_that_appears_before_its_dependency_is_resolved() {
    let mut infos = ProcessorAssetInfos::default();
    let a = AssetPath::from("a.bin");
    let b = AssetPath::from("b.bin");

    // b is processed first: a does not exist yet, so the edge is parked.
    infos.insert_processed(b.clone(), processed_info(2, 20, &[("a.bin", 10)]));
    assert!(infos.get(&a).is_none());

    // When a appears, b becomes its dependent.
    infos.insert_processed(a.clone(), processed_info(1, 10, &[]));
    assert_eq!(
        infos.get(&a).unwrap().dependents().clone(),
        [b.clone()].into_iter().collect()
    );
}

#[test]
fn unchanged_input_is_skipped() {
    let mut infos = ProcessorAssetInfos::default();
    let a = AssetPath::from("a.bin");
    let b = AssetPath::from("b.bin");

    infos.insert_processed(a.clone(), processed_info(1, 10, &[]));
    infos.insert_processed(b.clone(), processed_info(2, 20, &[("a.bin", 10)]));

    assert!(infos.is_up_to_date(&b, [2u8; 32]));
}

#[test]
fn a_changed_hash_or_dependency_is_not_skipped() {
    let mut infos = ProcessorAssetInfos::default();
    let a = AssetPath::from("a.bin");
    let b = AssetPath::from("b.bin");

    infos.insert_processed(a.clone(), processed_info(1, 10, &[]));
    infos.insert_processed(b.clone(), processed_info(2, 20, &[("a.bin", 10)]));

    // The asset's own bytes/meta changed.
    assert!(!infos.is_up_to_date(&b, [9u8; 32]));

    // The asset is unchanged, but its dependency's `full_hash` moved.
    infos.insert_processed(a.clone(), processed_info(1, 11, &[]));
    assert!(!infos.is_up_to_date(&b, [2u8; 32]));
}

#[test]
fn a_missing_asset_or_dependency_is_not_skipped() {
    let mut infos = ProcessorAssetInfos::default();
    let a = AssetPath::from("a.bin");
    let b = AssetPath::from("b.bin");

    // b is processed against a dependency that does not exist.
    infos.insert_processed(b.clone(), processed_info(2, 20, &[("a.bin", 10)]));
    assert!(!infos.is_up_to_date(&b, [2u8; 32]));

    // An asset that was never processed is never up to date.
    assert!(!infos.is_up_to_date(&AssetPath::from("ghost.bin"), [0u8; 32]));

    // Nor is one whose dependency has since been removed.
    infos.insert_processed(a.clone(), processed_info(1, 10, &[]));
    assert!(infos.is_up_to_date(&b, [2u8; 32]));
    infos.remove(&a);
    assert!(!infos.is_up_to_date(&b, [2u8; 32]));
}

#[test]
fn stale_dependency_edges_are_cleared_on_reprocess() {
    let mut infos = ProcessorAssetInfos::default();
    let a = AssetPath::from("a.bin");
    let b = AssetPath::from("b.bin");

    infos.insert_processed(a.clone(), processed_info(1, 10, &[]));
    infos.insert_processed(b.clone(), processed_info(2, 20, &[("a.bin", 10)]));
    assert!(infos.get(&a).unwrap().dependents().contains(&b));

    // b is reprocessed without depending on a.
    infos.insert_processed(b.clone(), processed_info(2, 21, &[]));
    assert!(!infos.get(&a).unwrap().dependents().contains(&b));
}
