//! Proves rule (1) of the widget kit: it cannot name domain types or do IO.
//! Cargo already enforces the dependency direction (egui is the only dep); this
//! test additionally guards the source against copy-pasted domain names / IO
//! APIs, which would otherwise compile while eroding the boundary.

use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] = &[
    "kairos_editor",
    "ProjectState",
    "DocumentState",
    "ProjectAction",
    "DocumentAction",
    "UiOut",
    "std::fs",
    "std::process",
    "std::net",
    "tokio",
    "reqwest",
    "serde",
];

#[test]
fn widgets_crate_has_no_domain_or_io_types() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("egui = \"=0.35.0\""));
    assert!(
        !manifest.contains("kairos-editor"),
        "widgets crate must not depend on components"
    );
    assert_eq!(
        manifest.matches("[dependencies]").count(),
        1,
        "exactly one dependency section expected"
    );

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0usize;
    for entry in fs::read_dir(&src).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        checked += 1;
        let source = fs::read_to_string(&path).expect("read widget source");
        // Comments may *document* the boundary (e.g. "cannot name ProjectState");
        // only real code must stay domain-free, so strip line comments first.
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in FORBIDDEN {
            assert!(
                !code.contains(forbidden),
                "{} mentions forbidden token '{}' -- widget kit must stay domain-free",
                path.display(),
                forbidden
            );
        }
    }
    assert!(
        checked >= 5,
        "expected to scan all widget modules, scanned {checked}"
    );
}
