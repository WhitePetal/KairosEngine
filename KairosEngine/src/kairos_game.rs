use std::path::PathBuf;

use crate::{
    asset_loader::assets::{
        MaterialAssetsSystem, MeshAssetsSystem, ShaderAssetsSystem, TextureAssetsSystem,
    }, base_components::TransformComponent, ecs::world::{SceneId, World, scene::Scene}, math::{float3, quaternion}
};

pub struct KairosGame {
    main_scene: SceneId,
}

impl KairosGame {
    pub fn new(world: &mut World) -> Self {
        let assets_server = &mut world.assets_server;
        assets_server.push(TextureAssetsSystem::new());
        assets_server.push(ShaderAssetsSystem::new());
        assets_server.push(MaterialAssetsSystem::new());
        assets_server.push(MeshAssetsSystem::new());

        let mesh = assets_server.load::<MeshAssetsSystem>(PathBuf::from("res/models/Suzanne.mesh"));
        let material = assets_server.load::<MaterialAssetsSystem>(PathBuf::from("res/materials/material.mat"));

        let main_scene = world.push_scene(Scene::new(1024, 1024, 1024));

        const NUM_INSTANCES_PER_ROW: i32 = 5;

        for z in -NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW {
            for x in -NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW {
                let position = float3::new(x as f32, 0.0, z as f32);
                let rotation = quaternion::identity();
                let scale = float3::new(1.0, 1.0, 1.0);

                world.create_entity(&main_scene, TransformComponent {
                    position,
                    rotation,
                    scale
                });

                // let local_to_world = float4x4::trs(position, rotation, scale);
                // graphics_command.draw(mesh.clone(), material.clone(), local_to_world);
            }
        }

        Self {
            main_scene
        }
    }

    pub fn update(&mut self, world: &mut World) {

    }
}
