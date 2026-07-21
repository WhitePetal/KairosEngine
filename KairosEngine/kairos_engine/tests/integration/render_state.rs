use std::path::PathBuf;

use kairos_engine::graphics::{
    compare_function::CompareFunction,
    render_state::{
        BlendPreset, CullMode, RenderState, WrappedBlendFactor, WrappedBlendOperation,
        WrappedPrimitiveTopology,
    },
};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

// ============================================================
// Helper: wrap a value in a struct for TOML round-trip
// (TOML cannot serialize standalone enums)
// ============================================================

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Wrap<T> {
    value: T,
}

fn toml_standalone_roundtrip<T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + PartialEq>(
    value: &T,
) -> T {
    let wrapper = Wrap { value };
    let toml_str = toml::to_string(&wrapper).expect("serialize");
    let rt: Wrap<T> = toml::from_str(&toml_str).expect("deserialize");
    rt.value
}

// ============================================================
// CullMode
// ============================================================

#[test]
fn cull_mode_serde_roundtrip() {
    for mode in [CullMode::None, CullMode::Back, CullMode::Front] {
        let rt = toml_standalone_roundtrip(&mode);
        assert_eq!(rt, mode, "round-trip failed for {:?}", mode);
    }
}

#[test]
fn cull_mode_label() {
    assert_eq!(CullMode::None.label(), "None");
    assert_eq!(CullMode::Back.label(), "Back");
    assert_eq!(CullMode::Front.label(), "Front");
}

#[test]
fn cull_mode_to_wgpu_roundtrip() {
    for mode in [CullMode::None, CullMode::Back, CullMode::Front] {
        let wgpu: Option<wgpu::Face> = mode.into();
        let back: CullMode = wgpu.into();
        assert_eq!(back, mode, "wgpu round-trip failed for {:?}", mode);
    }
}

#[test]
fn cull_mode_serialization_backward_compat() {
    // Old .mat files used wgpu Face format: "back" / "front" / absent
    // Test via a struct field to match real .mat deserialization
    #[derive(Deserialize)]
    struct TestStruct {
        cull_mod: CullMode,
    }

    let back: TestStruct = toml::from_str("cull_mod = \"back\"").expect("deserialize back");
    assert_eq!(back.cull_mod, CullMode::Back);

    let front: TestStruct = toml::from_str("cull_mod = \"front\"").expect("deserialize front");
    assert_eq!(front.cull_mod, CullMode::Front);

    let none: TestStruct = toml::from_str("cull_mod = \"none\"").expect("deserialize none");
    assert_eq!(none.cull_mod, CullMode::None);
}

#[test]
fn cull_mode_default_is_back() {
    assert_eq!(CullMode::default(), CullMode::Back);
}

// ============================================================
// WrappedPrimitiveTopology
// ============================================================

#[test]
fn topology_serde_roundtrip() {
    let cases = [
        WrappedPrimitiveTopology::PointList,
        WrappedPrimitiveTopology::LineList,
        WrappedPrimitiveTopology::LineStrip,
        WrappedPrimitiveTopology::TriangleList,
        WrappedPrimitiveTopology::TriangleStrip,
    ];
    for t in cases {
        let rt = toml_standalone_roundtrip(&t);
        assert_eq!(rt, t, "round-trip failed for {:?}", t);
    }
}

#[test]
fn topology_label() {
    assert_eq!(
        WrappedPrimitiveTopology::TriangleList.label(),
        "Triangle List"
    );
    assert_eq!(WrappedPrimitiveTopology::LineStrip.label(), "Line Strip");
}

#[test]
fn topology_to_wgpu_roundtrip() {
    let cases = [
        WrappedPrimitiveTopology::PointList,
        WrappedPrimitiveTopology::LineList,
        WrappedPrimitiveTopology::LineStrip,
        WrappedPrimitiveTopology::TriangleList,
        WrappedPrimitiveTopology::TriangleStrip,
    ];
    for t in cases {
        let wgpu: wgpu::PrimitiveTopology = t.into();
        let back: WrappedPrimitiveTopology = wgpu.into();
        assert_eq!(back, t, "wgpu round-trip failed for {:?}", t);
    }
}

#[test]
fn topology_serialization_backward_compat() {
    // Old .mat files used kebab-case wgpu format
    #[derive(Deserialize)]
    struct TestStruct {
        topology: WrappedPrimitiveTopology,
    }

    let tl: TestStruct =
        toml::from_str("topology = \"triangle-list\"").expect("deserialize triangle-list");
    assert_eq!(tl.topology, WrappedPrimitiveTopology::TriangleList);

    let ls: TestStruct =
        toml::from_str("topology = \"line-strip\"").expect("deserialize line-strip");
    assert_eq!(ls.topology, WrappedPrimitiveTopology::LineStrip);
}

// ============================================================
// BlendPreset
// ============================================================

#[test]
fn blend_preset_serde_roundtrip() {
    // Test the known presets + a truly custom blend state
    let truly_custom = BlendPreset::Custom(wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::DstAlpha,
            operation: wgpu::BlendOperation::Subtract,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Min,
        },
    });

    let presets = [
        BlendPreset::Replace,
        BlendPreset::Add,
        BlendPreset::Multiply,
        BlendPreset::AlphaBlend,
        truly_custom,
    ];
    for (i, p) in presets.iter().enumerate() {
        let rt = toml_standalone_roundtrip(p);
        assert_eq!(rt, *p, "round-trip failed for preset {}", i);
    }
}

#[test]
fn blend_preset_presets_roundtrip_via_serde() {
    // Known presets should survive serialization unchanged
    let presets = [
        BlendPreset::Replace,
        BlendPreset::Add,
        BlendPreset::Multiply,
        BlendPreset::AlphaBlend,
    ];
    for p in presets {
        let rt = toml_standalone_roundtrip(&p);
        assert_eq!(rt, p, "preset {:?} should survive round-trip unchanged", p);
    }
}

#[test]
fn blend_preset_label() {
    assert_eq!(BlendPreset::Replace.label(), "Replace");
    assert_eq!(BlendPreset::Add.label(), "Add");
    assert_eq!(BlendPreset::Multiply.label(), "Multiply");
    assert_eq!(BlendPreset::AlphaBlend.label(), "Alpha Blend");
    assert_eq!(
        BlendPreset::Custom(wgpu::BlendState::REPLACE).label(),
        "Custom"
    );
}

#[test]
fn blend_preset_to_wgpu_consistency() {
    // Replace -> wgpu BlendState -> Replace
    let bs: Option<wgpu::BlendState> = BlendPreset::Replace.into();
    let back: BlendPreset = bs.into();
    assert_eq!(back, BlendPreset::Replace);

    // AlphaBlend -> wgpu BlendState -> AlphaBlend
    let bs: Option<wgpu::BlendState> = BlendPreset::AlphaBlend.into();
    let back: BlendPreset = bs.into();
    assert_eq!(back, BlendPreset::AlphaBlend);
}

#[test]
fn blend_preset_backward_compat_replace() {
    // The existing .mat files use wgpu's REPLACE blend state serialization
    let toml_str = r#"
[color]
srcFactor = "one"
dstFactor = "zero"
operation = "add"

[alpha]
srcFactor = "one"
dstFactor = "zero"
operation = "add"
"#;
    let preset: BlendPreset = toml::from_str(toml_str).expect("deserialize REPLACE");
    assert_eq!(preset, BlendPreset::Replace);
}

#[test]
fn blend_preset_backward_compat_alpha_blend() {
    let toml_str = r#"
[color]
srcFactor = "src-alpha"
dstFactor = "one-minus-src-alpha"
operation = "add"

[alpha]
srcFactor = "one"
dstFactor = "one-minus-src-alpha"
operation = "add"
"#;
    let preset: BlendPreset = toml::from_str(toml_str).expect("deserialize ALPHA_BLENDING");
    assert_eq!(preset, BlendPreset::AlphaBlend);
}

#[test]
fn blend_preset_default_is_replace() {
    assert_eq!(BlendPreset::default(), BlendPreset::Replace);
}

// ============================================================
// WrappedBlendFactor
// ============================================================

#[test]
fn blend_factor_serde_roundtrip() {
    for factor in WrappedBlendFactor::iter() {
        let rt = toml_standalone_roundtrip(&factor);
        assert_eq!(rt, factor, "round-trip failed for {:?}", factor);
    }
}

#[test]
fn blend_factor_label() {
    assert_eq!(WrappedBlendFactor::One.label(), "One");
    assert_eq!(WrappedBlendFactor::Zero.label(), "Zero");
    assert_eq!(WrappedBlendFactor::SrcAlpha.label(), "Src Alpha");
}

#[test]
fn blend_factor_to_wgpu_roundtrip() {
    for factor in WrappedBlendFactor::iter() {
        let wgpu: wgpu::BlendFactor = factor.into();
        let back: WrappedBlendFactor = wgpu.into();
        assert_eq!(back, factor, "wgpu round-trip failed for {:?}", factor);
    }
}

#[test]
fn blend_factor_iter_covers_all() {
    let count = WrappedBlendFactor::iter().count();
    assert_eq!(count, 17, "expected 17 blend factors, got {}", count);
}

// ============================================================
// WrappedBlendOperation
// ============================================================

#[test]
fn blend_operation_serde_roundtrip() {
    for op in WrappedBlendOperation::iter() {
        let rt = toml_standalone_roundtrip(&op);
        assert_eq!(rt, op, "round-trip failed for {:?}", op);
    }
}

#[test]
fn blend_operation_label() {
    assert_eq!(WrappedBlendOperation::Add.label(), "Add");
    assert_eq!(WrappedBlendOperation::Subtract.label(), "Subtract");
}

#[test]
fn blend_operation_to_wgpu_roundtrip() {
    for op in WrappedBlendOperation::iter() {
        let wgpu: wgpu::BlendOperation = op.into();
        let back: WrappedBlendOperation = wgpu.into();
        assert_eq!(back, op, "wgpu round-trip failed for {:?}", op);
    }
}

#[test]
fn blend_operation_iter_covers_all() {
    let count = WrappedBlendOperation::iter().count();
    assert_eq!(count, 5, "expected 5 blend operations, got {}", count);
}

// ============================================================
// RenderState — full struct backward compatibility
// ============================================================

#[test]
fn render_state_full_serde_roundtrip() {
    let rs = RenderState::default();
    let toml_str = toml::to_string(&rs).expect("serialize");
    let rt: RenderState = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(rt, rs);
}

#[test]
fn render_state_deserialize_existing_mat_format() {
    // Simulate the format found in res/materials/material.mat
    let toml_str = r#"
depth_test = "LessEqual"
depth_write = true
cull_mod = "back"
topology = "triangle-list"

[blend_mod.color]
srcFactor = "one"
dstFactor = "zero"
operation = "add"

[blend_mod.alpha]
srcFactor = "one"
dstFactor = "zero"
operation = "add"
"#;
    let rs: RenderState = toml::from_str(toml_str).expect("deserialize material.mat format");
    assert_eq!(rs.depth_test, Some(CompareFunction::LessEqual));
    assert!(rs.depth_write);
    assert_eq!(rs.cull_mod, CullMode::Back);
    assert_eq!(rs.topology, WrappedPrimitiveTopology::TriangleList);
    assert_eq!(rs.blend_mod, BlendPreset::Replace);
}

#[test]
fn render_state_deserialize_missing_cull_mod() {
    // Some .mat files (e.g. axes.mat) don't have cull_mod — should default to Back
    let toml_str = r#"
depth_test = "LessEqual"
depth_write = true
topology = "triangle-list"

[blend_mod.color]
srcFactor = "one"
dstFactor = "zero"
operation = "add"

[blend_mod.alpha]
srcFactor = "one"
dstFactor = "zero"
operation = "add"
"#;
    let rs: RenderState = toml::from_str(toml_str).expect("deserialize without cull_mod");
    assert_eq!(rs.cull_mod, CullMode::Back); // default
}

#[test]
fn render_state_deserialize_missing_blend_mod() {
    // Materials without blend_mod should default to Replace
    let toml_str = r#"
depth_test = "LessEqual"
depth_write = true
cull_mod = "back"
topology = "triangle-list"
"#;
    let rs: RenderState = toml::from_str(toml_str).expect("deserialize without blend_mod");
    assert_eq!(rs.blend_mod, BlendPreset::Replace); // default
}

#[test]
fn render_state_deserialize_custom_blend() {
    // A custom blend that doesn't match any preset should become Custom
    let toml_str = r#"
depth_test = "LessEqual"
depth_write = true
cull_mod = "back"
topology = "triangle-list"

[blend_mod.color]
srcFactor = "src-alpha"
dstFactor = "dst-alpha"
operation = "subtract"

[blend_mod.alpha]
srcFactor = "one"
dstFactor = "zero"
operation = "min"
"#;
    let rs: RenderState = toml::from_str(toml_str).expect("deserialize custom blend");
    match rs.blend_mod {
        BlendPreset::Custom(state) => {
            assert_eq!(state.color.src_factor, wgpu::BlendFactor::SrcAlpha);
            assert_eq!(state.color.dst_factor, wgpu::BlendFactor::DstAlpha);
            assert_eq!(state.color.operation, wgpu::BlendOperation::Subtract);
            assert_eq!(state.alpha.src_factor, wgpu::BlendFactor::One);
            assert_eq!(state.alpha.dst_factor, wgpu::BlendFactor::Zero);
            assert_eq!(state.alpha.operation, wgpu::BlendOperation::Min);
        }
        other => panic!("expected Custom, got {:?}", other),
    }
}

#[test]
fn render_state_default_values() {
    let rs = RenderState::default();
    assert_eq!(rs.depth_test, Some(CompareFunction::LessEqual));
    assert!(rs.depth_write);
    assert_eq!(rs.cull_mod, CullMode::Back);
    assert_eq!(rs.blend_mod, BlendPreset::Replace);
    assert_eq!(rs.topology, WrappedPrimitiveTopology::TriangleList);
}

// ============================================================
// Real .mat file backward compatibility
// ============================================================

#[derive(Debug, Deserialize)]
struct SerializedMaterial {
    #[allow(dead_code)]
    source_path: PathBuf,
    #[allow(dead_code)]
    shader_path: PathBuf,
    #[allow(dead_code)]
    render_state: RenderState,
    #[allow(dead_code)]
    texture_path: Option<PathBuf>,
}

/// Walk a directory and verify every .mat file deserializes without error.
fn verify_mat_files(dir: &PathBuf, failures: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        if path.is_dir() {
            verify_mat_files(&path, failures);
        } else if path.extension().and_then(|e| e.to_str()) == Some("mat") {
            let content = std::fs::read_to_string(&path).expect("read .mat");
            match toml::from_str::<SerializedMaterial>(&content) {
                Ok(_) => {}
                Err(e) => failures.push(format!("{}: {}", path.display(), e)),
            }
        }
    }
}

#[test]
fn all_existing_mat_files_deserialize() {
    let mats_dir = PathBuf::from("res/materials");
    if !mats_dir.exists() {
        // When running from workspace root or different CWD
        return;
    }
    let mut failures = Vec::new();
    verify_mat_files(&mats_dir, &mut failures);
    assert!(
        failures.is_empty(),
        "Failed to deserialize .mat files:\n{}",
        failures.join("\n")
    );
}
