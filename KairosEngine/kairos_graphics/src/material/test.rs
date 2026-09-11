//! Tests for the `Material` / `SerializedMaterial` loaders.

use std::{thread, time::Duration};

use kairos_asset::{AssetServer, Assets, install};
use kairos_ecs::schedule::ScheduleLabel;
use kairos_ecs::world::World;

use super::{
    Material, SerializedMaterial, install as install_material,
};
use crate::{render_state::RenderState, shader::ShaderAsset, shader::install as install_shader};

/// The two ad-hoc stages the asset drivers are installed into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Tracking;

impl ScheduleLabel for Tracking {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Events;

impl ScheduleLabel for Events {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

/// Builds a world with the core plus the shader and material stores.
fn installed_world() -> World {
    let mut world = World::new();
    install(&mut world, Tracking, Events);
    install_shader(&mut world);
    install_material(&mut world);
    world
}

/// A `.mat` load through the new core resolves its shader dependency handle.
#[test]
fn material_declares_and_resolves_its_shader_dependency() {
    let dir = tempfile::Builder::new()
        .tempdir_in(".")
        .expect("a temp dir in the cwd");
    let cwd = std::env::current_dir().expect("the cwd");

    let shader_full_path = dir.path().join("Probe.wgsl");
    std::fs::write(&shader_full_path, b"@vertex fn vs_main() {}").expect("write the shader");
    let shader_rel_path = shader_full_path
        .strip_prefix(&cwd)
        .expect("the temp dir is under the cwd")
        .to_path_buf();

    let mat_full_path = dir.path().join("Probe.mat");
    let mat_rel_path = mat_full_path
        .strip_prefix(&cwd)
        .expect("the temp dir is under the cwd")
        .to_path_buf();
    let serialized = SerializedMaterial {
        source_path: mat_rel_path.clone(),
        shader_path: shader_rel_path.clone(),
        render_state: RenderState::default(),
        texture_path: None,
    };
    std::fs::write(&mat_full_path, toml::to_string(&serialized).unwrap())
        .expect("write the material");

    let mut world = installed_world();
    let material_handle = world
        .resource::<AssetServer>()
        .load::<Material>(mat_rel_path);

    let mut resolved = false;
    for _ in 0..200 {
        world.run_schedule(Tracking);
        if let Some(material) = world.resource::<Assets<Material>>().get(material_handle.id())
            && let Some(shader_handle) = &material.shader
            && world
                .resource::<Assets<ShaderAsset>>()
                .get(shader_handle.id())
                .is_some()
        {
            resolved = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    assert!(
        resolved,
        "the material and its shader dependency must both reach their stores"
    );
}

/// A `SerializedMaterial` load keeps the editable, path-based form.
#[test]
fn serialized_material_loads_with_its_paths() {
    let dir = tempfile::Builder::new()
        .tempdir_in(".")
        .expect("a temp dir in the cwd");
    let cwd = std::env::current_dir().expect("the cwd");

    let mat_full_path = dir.path().join("Probe.mat");
    let mat_rel_path = mat_full_path
        .strip_prefix(&cwd)
        .expect("the temp dir is under the cwd")
        .to_path_buf();
    let serialized = SerializedMaterial {
        source_path: mat_rel_path.clone(),
        shader_path: "res/shaders/shader.wgsl".into(),
        render_state: RenderState::default(),
        texture_path: Some("res/textures/white.texture".into()),
    };
    std::fs::write(&mat_full_path, toml::to_string(&serialized).unwrap())
        .expect("write the material");

    let mut world = installed_world();
    let handle = world
        .resource::<AssetServer>()
        .load::<SerializedMaterial>(mat_rel_path);

    let mut texture_path = None;
    for _ in 0..200 {
        world.run_schedule(Tracking);
        if let Some(loaded) = world
            .resource::<Assets<SerializedMaterial>>()
            .get(handle.id())
        {
            texture_path = Some(loaded.texture_path.clone());
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(
        texture_path,
        Some(Some("res/textures/white.texture".into()))
    );
}
