use std::path::PathBuf;

use kairos_engine::graphics::{
    compare_function::CompareFunction,
    material::SerializedMaterial,
    render_state::{
        BlendComponent, BlendFactor, BlendOperation, BlendPreset, BlendState, CullMode,
        PrimitiveTopology, RenderState,
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

fn toml_standalone_roundtrip<
    T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + PartialEq,
>(
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
fn cull_mode_default_is_back() {
    assert_eq!(CullMode::default(), CullMode::Back);
}

// ============================================================
// PrimitiveTopology
// ============================================================

#[test]
fn topology_serde_roundtrip() {
    let cases = [
        PrimitiveTopology::PointList,
        PrimitiveTopology::LineList,
        PrimitiveTopology::LineStrip,
        PrimitiveTopology::TriangleList,
        PrimitiveTopology::TriangleStrip,
    ];
    for t in cases {
        let rt = toml_standalone_roundtrip(&t);
        assert_eq!(rt, t, "round-trip failed for {:?}", t);
    }
}

#[test]
fn topology_label() {
    assert_eq!(PrimitiveTopology::PointList.label(), "PointList");
    assert_eq!(PrimitiveTopology::LineList.label(), "LineList");
    assert_eq!(PrimitiveTopology::LineStrip.label(), "LineStrip");
    assert_eq!(PrimitiveTopology::TriangleList.label(), "TriangleList");
    assert_eq!(PrimitiveTopology::TriangleStrip.label(), "TriangleStrip");
}

#[test]
fn topology_to_wgpu_roundtrip() {
    let cases = [
        PrimitiveTopology::PointList,
        PrimitiveTopology::LineList,
        PrimitiveTopology::LineStrip,
        PrimitiveTopology::TriangleList,
        PrimitiveTopology::TriangleStrip,
    ];
    for t in cases {
        let wgpu: wgpu::PrimitiveTopology = t.into();
        let back: PrimitiveTopology = wgpu.into();
        assert_eq!(back, t, "wgpu round-trip failed for {:?}", t);
    }
}

// ============================================================
// BlendFactor
// ============================================================

#[test]
fn blend_factor_serde_roundtrip() {
    for factor in BlendFactor::iter() {
        let rt = toml_standalone_roundtrip(&factor);
        assert_eq!(rt, factor, "round-trip failed for {:?}", factor);
    }
}

#[test]
fn blend_factor_label() {
    assert_eq!(BlendFactor::One.label(), "One");
    assert_eq!(BlendFactor::Zero.label(), "Zero");
    assert_eq!(BlendFactor::SrcAlpha.label(), "SrcAlpha");
}

#[test]
fn blend_factor_to_wgpu_roundtrip() {
    for factor in BlendFactor::iter() {
        let wgpu: wgpu::BlendFactor = factor.into();
        let back: BlendFactor = wgpu.into();
        assert_eq!(back, factor, "wgpu round-trip failed for {:?}", factor);
    }
}

#[test]
fn blend_factor_iter_covers_all() {
    let count = BlendFactor::iter().count();
    assert_eq!(count, 17, "expected 17 blend factors, got {}", count);
}

// ============================================================
// BlendOperation
// ============================================================

#[test]
fn blend_operation_serde_roundtrip() {
    for op in BlendOperation::iter() {
        let rt = toml_standalone_roundtrip(&op);
        assert_eq!(rt, op, "round-trip failed for {:?}", op);
    }
}

#[test]
fn blend_operation_label() {
    assert_eq!(BlendOperation::Add.label(), "Add");
    assert_eq!(BlendOperation::Subtract.label(), "Subtract");
    assert_eq!(BlendOperation::ReverseSubtract.label(), "ReverseSubtract");
    assert_eq!(BlendOperation::Min.label(), "Min");
    assert_eq!(BlendOperation::Max.label(), "Max");
}

#[test]
fn blend_operation_to_wgpu_roundtrip() {
    for op in BlendOperation::iter() {
        let wgpu: wgpu::BlendOperation = op.into();
        let back: BlendOperation = wgpu.into();
        assert_eq!(back, op, "wgpu round-trip failed for {:?}", op);
    }
}

#[test]
fn blend_operation_iter_covers_all() {
    let count = BlendOperation::iter().count();
    assert_eq!(count, 5, "expected 5 blend operations, got {}", count);
}

// ============================================================
// BlendState
// ============================================================

#[test]
fn blend_state_serde_roundtrip() {
    // Test known blend states round-trip through serialization
    let states = [
        BlendState::REPLACE,
        BlendState::ALPHA_BLENDING,
        BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::DstAlpha,
                operation: BlendOperation::Subtract,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Min,
            },
        },
    ];
    for (i, s) in states.iter().enumerate() {
        let rt = toml_standalone_roundtrip(s);
        assert_eq!(rt, *s, "round-trip failed for blend state {}", i);
    }
}

#[test]
fn blend_state_default_is_replace() {
    assert_eq!(BlendState::default(), BlendState::REPLACE);
}

#[test]
fn blend_state_to_wgpu() {
    let bs = BlendState::REPLACE;
    let wgpu_opt: Option<wgpu::BlendState> = bs.into();
    assert_eq!(wgpu_opt, Some(wgpu::BlendState::REPLACE));

    let wgpu_direct: wgpu::BlendState = bs.into();
    assert_eq!(wgpu_direct, wgpu::BlendState::REPLACE);
}

#[test]
fn blend_state_wgpu_roundtrip() {
    let states = [
        wgpu::BlendState::REPLACE,
        wgpu::BlendState::ALPHA_BLENDING,
        wgpu::BlendState {
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
        },
    ];
    for (i, &s) in states.iter().enumerate() {
        let ours: BlendState = s.into();
        let back: wgpu::BlendState = ours.into();
        assert_eq!(back, s, "wgpu round-trip failed for blend state {}", i);
    }
}

// ============================================================
// BlendPreset — helper for material inspector
// ============================================================

#[test]
fn blend_preset_label() {
    assert_eq!(BlendPreset::Replace.label(), "Replace");
    assert_eq!(BlendPreset::Add.label(), "Add");
    assert_eq!(BlendPreset::Multiply.label(), "Multiply");
    assert_eq!(BlendPreset::AlphaBlend.label(), "AlphaBlend");
    assert_eq!(BlendPreset::Custom(BlendState::REPLACE).label(), "Custom");
}

#[test]
fn blend_preset_to_blend_state_consistency() {
    // Replace -> BlendState -> Replace
    let bs: BlendState = BlendPreset::Replace.into();
    let back = BlendPreset::from_blend_state(bs);
    assert_eq!(back, BlendPreset::Replace);

    // AlphaBlend -> BlendState -> AlphaBlend
    let bs: BlendState = BlendPreset::AlphaBlend.into();
    let back = BlendPreset::from_blend_state(bs);
    assert_eq!(back, BlendPreset::AlphaBlend);

    // Add -> BlendState -> Add
    let bs: BlendState = BlendPreset::Add.into();
    let back = BlendPreset::from_blend_state(bs);
    assert_eq!(back, BlendPreset::Add);

    // Multiply -> BlendState -> Multiply
    let bs: BlendState = BlendPreset::Multiply.into();
    let back = BlendPreset::from_blend_state(bs);
    assert_eq!(back, BlendPreset::Multiply);
}

#[test]
fn blend_preset_custom_roundtrip() {
    let custom = BlendPreset::Custom(BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::DstAlpha,
            operation: BlendOperation::Subtract,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Min,
        },
    });
    let bs: BlendState = custom.into();
    let back = BlendPreset::from_blend_state(bs);
    match back {
        BlendPreset::Custom(state) => {
            assert_eq!(state.color.src_factor, BlendFactor::SrcAlpha);
            assert_eq!(state.color.dst_factor, BlendFactor::DstAlpha);
            assert_eq!(state.color.operation, BlendOperation::Subtract);
            assert_eq!(state.alpha.src_factor, BlendFactor::One);
            assert_eq!(state.alpha.dst_factor, BlendFactor::Zero);
            assert_eq!(state.alpha.operation, BlendOperation::Min);
        }
        other => panic!("expected Custom, got {:?}", other),
    }
}

#[test]
fn blend_preset_default_is_replace() {
    assert_eq!(BlendPreset::default(), BlendPreset::Replace);
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
fn render_state_default_values() {
    let rs = RenderState::default();
    assert_eq!(rs.depth_test, Some(CompareFunction::LessEqual));
    assert!(rs.depth_write);
    assert_eq!(rs.cull_mod, CullMode::Back);
    assert_eq!(rs.blend_mod, None);
    assert_eq!(rs.topology, PrimitiveTopology::TriangleList);
}

// ============================================================
// Real .mat file backward compatibility
// ============================================================

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
    // CARGO_MANIFEST_DIR = kairos_engine/, res/ is at workspace root (one level up)
    let mats_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("res/materials");
    if !mats_dir.exists() {
        panic!("materials directory not found at: {}", mats_dir.display());
    }
    let mut failures = Vec::new();
    verify_mat_files(&mats_dir, &mut failures);
    assert!(
        failures.is_empty(),
        "Failed to deserialize .mat files:\n{}",
        failures.join("\n")
    );
}
