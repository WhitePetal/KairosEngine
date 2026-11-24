//! Tests for the layout file: capture/restore round trips, the validation
//! matrix, and the debounced writer. The dock itself is built through its real
//! public operations (split/append/detach/focus), so a restore is compared
//! against a layout the user could actually produce.

use super::*;
use kairos_editor_docking::tree::NodeIndex;

const SCENE: PanelId = PanelId("scene");
const INSPECTOR: PanelId = PanelId("inspector");
const PROJECT: PanelId = PanelId("project");
const CONSOLE: PanelId = PanelId("console");

fn instance(kind: PanelId) -> PanelInstanceId {
    PanelInstanceId::new(kind, 0)
}

/// The panels a test build knows, with one live instance each.
fn known_panels() -> Vec<PanelId> {
    vec![SCENE, INSPECTOR, PROJECT, CONSOLE]
}

fn resolve(name: &str) -> Option<PanelInstanceId> {
    known_panels()
        .into_iter()
        .find(|kind| kind.0 == name)
        .map(instance)
}

/// A layout with a split main surface, two tabs in the left leaf, one in the
/// right leaf, one floating window and the left leaf focused.
fn sample_dock() -> DockState<PanelInstanceId> {
    let mut dock: DockState<PanelInstanceId> = DockState::new();
    dock.append_tab(instance(SCENE));
    dock.append_tab(instance(INSPECTOR));
    dock.main_surface_mut().split_right(
        NodeIndex::root(),
        Fraction::new(0.7),
        vec![instance(PROJECT)],
    );
    dock.open_in_window(
        instance(CONSOLE),
        Rect::from_min_size(pos2(1200.0, 80.0), vec2(420.0, 300.0)),
    );
    dock.focus_tab(instance(SCENE));
    dock
}

fn empty_file() -> LayoutFile {
    LayoutFile {
        version: VERSION,
        main: SurfaceFile::default(),
        windows: Vec::new(),
        focus: None,
        panels: PanelsFile::default(),
    }
}

fn file_with_main(nodes: Vec<NodeFile>) -> LayoutFile {
    LayoutFile {
        main: SurfaceFile { nodes },
        ..empty_file()
    }
}

#[test]
fn capture_and_restore_round_trip_the_dock() {
    let dock = sample_dock();
    let captured = LayoutFile::capture(&dock, 0.33).expect("the sample dock is capturable");

    assert_eq!(captured.version, VERSION);
    assert_eq!(captured.focus.as_deref(), Some("scene"));
    assert_eq!(captured.panels.project.tree_split, 0.33);
    assert_eq!(captured.main.nodes.len(), 3, "split + two leaves");
    assert_eq!(captured.windows.len(), 1);

    let text = toml::to_string_pretty(&captured).expect("serialize");
    assert!(
        !text.contains("rect"),
        "frame geometry must not be persisted: {text}"
    );
    assert!(
        !text.contains("viewport"),
        "frame geometry must not be persisted"
    );
    let parsed: LayoutFile = toml::from_str(&text).expect("deserialize");

    let restored = parsed.restore(&known_panels()).expect("the file is valid");
    let rebuilt = restored.dock_with(&resolve).expect("live ids resolve");
    assert_eq!(rebuilt.tabs().len(), 4);
    assert_eq!(rebuilt.focused_tab(), Some(instance(SCENE)));
    assert_eq!(rebuilt.valid_surface_indices().len(), 2);

    let Surface::Main(main) = &rebuilt.surfaces()[0] else {
        panic!("main surface first");
    };
    let Node::Horizontal(split) = &main.nodes[0] else {
        panic!("the root is the saved horizontal split");
    };
    assert!((split.fraction.value() - 0.7).abs() < 1e-6);

    let Surface::Window(_, window) = &rebuilt.surfaces()[1] else {
        panic!("the second surface is the floating window");
    };
    assert_eq!(window.position(), Some(pos2(1200.0, 80.0)));
    assert_eq!(window.size(), Some(vec2(420.0, 300.0)));
}

#[test]
fn capture_keeps_tab_order_and_active_tab() {
    let dock = sample_dock();
    let captured = LayoutFile::capture(&dock, 0.28).expect("capture");
    let left = &captured.main.nodes[1];
    assert_eq!(left.kind, NodeKind::Leaf);
    assert_eq!(left.tabs, vec!["scene", "inspector"]);
    assert_eq!(left.active, 0, "focus_tab(scene) made scene active");
    let right = &captured.main.nodes[2];
    assert_eq!(right.tabs, vec!["project"]);
    assert_eq!(right.active, 0);
}

#[test]
fn an_empty_dock_captures_as_an_empty_tree() {
    let dock: DockState<PanelInstanceId> = DockState::new();
    let captured = LayoutFile::capture(&dock, 0.28).expect("an empty dock is capturable");
    assert!(captured.main.nodes.is_empty());
    assert!(captured.windows.is_empty());

    let restored = captured.restore(&known_panels()).expect("valid");
    assert!(restored.open().is_empty());
    let rebuilt = restored.dock_with(&resolve).expect("empty dock restores");
    assert!(rebuilt.tabs().is_empty());
    assert_eq!(rebuilt.focused_tab(), None);
}

#[test]
fn tree_split_is_clamped_and_nan_falls_back_to_default() {
    assert_eq!(clamp_tree_split(9.0), MAX_TREE_SPLIT);
    assert_eq!(clamp_tree_split(0.0), MIN_TREE_SPLIT);
    assert_eq!(clamp_tree_split(f32::NAN), default_tree_split());

    let mut file = empty_file();
    file.panels.project.tree_split = 9.0;
    let restored = file.restore(&known_panels()).expect("valid");
    assert_eq!(restored.tree_split(), MAX_TREE_SPLIT);
}

#[test]
fn restore_rejects_unknown_panels() {
    let file = file_with_main(vec![NodeFile::leaf(&["nope"], 0)]);
    let error = file.restore(&known_panels()).expect_err("unknown panel");
    assert!(error.contains("nope"), "{error}");
}

#[test]
fn restore_rejects_a_panel_that_appears_twice() {
    let mut file = file_with_main(vec![NodeFile::leaf(&["scene"], 0)]);
    file.windows.push(WindowFile {
        nodes: vec![NodeFile::leaf(&["scene"], 0)],
        position: [10.0, 10.0],
        size: [300.0, 200.0],
        minimized: false,
        maximized: false,
    });
    let error = file.restore(&known_panels()).expect_err("duplicate panel");
    assert!(error.contains("more than once"), "{error}");
}

#[test]
fn restore_rejects_a_malformed_node_tree() {
    // A split without its right child.
    let file = file_with_main(vec![
        NodeFile::split(NodeKind::Horizontal, 0.5),
        NodeFile::leaf(&["scene"], 0),
    ]);
    assert!(file.restore(&known_panels()).is_err());

    // A trailing node no parent points at.
    let file = file_with_main(vec![
        NodeFile::leaf(&["scene"], 0),
        NodeFile::leaf(&["console"], 0),
    ]);
    assert!(file.restore(&known_panels()).is_err());

    // A leaf whose active tab is out of range.
    let file = file_with_main(vec![NodeFile::leaf(&["scene"], 3)]);
    assert!(file.restore(&known_panels()).is_err());

    // A floating window with no tab and one with invalid geometry.
    let mut file = empty_file();
    file.windows.push(WindowFile {
        nodes: Vec::new(),
        position: [0.0, 0.0],
        size: [10.0, 10.0],
        minimized: false,
        maximized: false,
    });
    assert!(file.restore(&known_panels()).is_err());

    let mut file = empty_file();
    file.windows.push(WindowFile {
        nodes: vec![NodeFile::leaf(&["scene"], 0)],
        position: [0.0, 0.0],
        size: [0.0, 10.0],
        minimized: false,
        maximized: false,
    });
    assert!(file.restore(&known_panels()).is_err());
}

#[test]
fn degenerate_split_fractions_are_clamped() {
    let file = file_with_main(vec![
        NodeFile::split(NodeKind::Horizontal, 0.0),
        NodeFile::leaf(&["scene"], 0),
        NodeFile::leaf(&["project"], 0),
    ]);
    let restored = file
        .restore(&known_panels())
        .expect("finite fractions are valid");
    let dock = restored.dock_with(&resolve).expect("live ids resolve");
    let Surface::Main(main) = &dock.surfaces()[0] else {
        panic!("main surface first");
    };
    let Node::Horizontal(split) = &main.nodes[0] else {
        panic!("the root is the horizontal split");
    };
    assert!(
        (split.fraction.value() - MIN_SPLIT_FRACTION).abs() < 1e-6,
        "a degenerate fraction must not collapse a pane"
    );
}

#[test]
fn restore_rejects_an_unsupported_version() {
    let mut file = empty_file();
    file.version = VERSION + 1;
    assert!(file.restore(&known_panels()).is_err());
}

#[test]
fn restore_returns_none_when_a_known_panel_has_no_instance() {
    let file = file_with_main(vec![NodeFile::leaf(&["scene"], 0)]);
    let restored = file.restore(&known_panels()).expect("valid");
    // The build knows "scene" but the host did not create an instance for it.
    assert!(restored.dock_with(&|_| None).is_none());
}

#[test]
fn the_file_round_trips_through_disk_atomically() {
    let dir = tempfile::tempdir().expect("tempdir");
    let captured = LayoutFile::capture(&sample_dock(), 0.33).expect("capture");
    let path = layout_path(dir.path());
    save(&path, &captured).expect("save");

    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.starts_with("# Editor layout"), "{text}");
    assert!(text.contains("version = 1"), "{text}");
    let loaded = load(dir.path()).expect("saved file loads");
    assert_eq!(loaded, captured);

    let leftovers: Vec<String> = std::fs::read_dir(path.parent().expect("parent"))
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "no temp file may survive a save: {leftovers:?}"
    );
}

#[test]
fn a_missing_or_corrupt_file_is_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        load(dir.path()).is_none(),
        "no file means the default layout"
    );

    let path = layout_path(dir.path());
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");

    std::fs::write(&path, "not toml at all").expect("write garbage");
    assert!(load(dir.path()).is_none());

    std::fs::write(&path, "version = 2\n").expect("write future version");
    assert!(load(dir.path()).is_none());

    std::fs::write(&path, vec![b'#'; (MAX_FILE_BYTES + 1) as usize]).expect("write oversized");
    assert!(load(dir.path()).is_none());
}

#[test]
fn the_writer_debounces_coalesces_and_flushes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = layout_path(dir.path());
    let baseline = LayoutFile::capture(&sample_dock(), 0.28).expect("capture");
    let mut persist = LayoutPersist::new(path.clone(), Some(baseline.clone()));
    let start = Instant::now();

    // The boot snapshot is the baseline: neither writing nor scheduling.
    persist.observe(Some(baseline.clone()), start);
    assert!(!persist.has_pending());
    assert!(!path.exists());

    let mut changed = baseline.clone();
    changed.focus = Some(String::from("console"));
    persist.observe(Some(changed.clone()), start);
    assert!(persist.has_pending());

    persist.poll(start + Duration::from_millis(400));
    assert!(!path.exists(), "the debounce has not elapsed yet");

    // A newer change restarts the window; the last value wins.
    let mut newest = changed.clone();
    newest.focus = Some(String::from("project"));
    persist.observe(Some(newest.clone()), start + Duration::from_millis(300));
    persist.poll(start + Duration::from_millis(700));
    assert!(!path.exists(), "the newer change restarted the debounce");

    persist.poll(start + Duration::from_millis(800));
    assert!(
        path.exists(),
        "the layout is written once the dock is still"
    );
    assert!(!persist.has_pending());
    let loaded = load(dir.path()).expect("written file loads");
    assert_eq!(loaded.focus.as_deref(), Some("project"));

    // A change right before exit is flushed, not lost.
    let mut last = newest;
    last.focus = Some(String::from("scene"));
    persist.observe(Some(last), start + Duration::from_millis(900));
    assert!(persist.has_pending());
    persist.flush(start + Duration::from_millis(901));
    let loaded = load(dir.path()).expect("flushed file loads");
    assert_eq!(loaded.focus.as_deref(), Some("scene"));
}

#[test]
fn a_disabled_writer_never_touches_the_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = layout_path(dir.path());
    let mut persist = LayoutPersist::disabled();
    let baseline = LayoutFile::capture(&sample_dock(), 0.28).expect("capture");

    persist.observe(Some(baseline), Instant::now());
    assert!(!persist.has_pending());
    persist.poll(Instant::now() + Duration::from_secs(5));
    persist.flush(Instant::now());
    assert!(!path.exists());
}
