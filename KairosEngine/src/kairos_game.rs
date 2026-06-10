use std::path::PathBuf;

use crate::{
    asset_loader::assets::{
        MaterialAssetsSystem, MeshAssetsSystem, ShaderAssetsSystem, TextureAssetsSystem,
    },
    base_components::TransformComponent,
    ecs::world::{SceneId, World, scene::Scene},
    graphics::{
        graphics_graph::GraphicsCommand, lod_mesh_component::LODMeshComponent,
        material_component::MaterialComponent,
    },
    math::{self, float3, float4, quaternion},
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
        let material =
            assets_server.load::<MaterialAssetsSystem>(PathBuf::from("res/materials/material.mat"));

        let main_scene = world.push_scene(Scene {});

        const NUM_INSTANCES_PER_ROW: i32 = 5;

        for z in -NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW {
            for x in -NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW {
                let position = float3::new(x as f32, 0.0, z as f32);
                let rotation = quaternion::identity();
                let scale = float3::new(1.0, 1.0, 1.0);

                world.create_entity(
                    &main_scene,
                    (
                        TransformComponent::new(position, rotation, scale),
                        LODMeshComponent::new(mesh.clone()),
                        MaterialComponent::new(material.clone()),
                    ),
                );
            }
        }

        Self { main_scene }
    }

    pub fn update(&self, world: &mut World) {
        let total_time = world.time.total_time().as_secs_f32();

        world.query_mut::<TransformComponent, _>(&self.main_scene, |trans| {
            let position = &mut trans.position;
            let x = position.x();
            let y = (x + total_time).sin();
            *position = float3::new(x, y, position.z());
        });
    }

    pub fn render(&self, world: &mut World, graphics_command: &mut GraphicsCommand) {
        world.query::<(TransformComponent, LODMeshComponent, MaterialComponent), _>(
            &self.main_scene,
            |(trans, lod, mat)| {
                graphics_command.draw(
                    lod.lod0.clone(),
                    mat.material.clone(),
                    trans.get_local_to_world(),
                );
            },
        );
    }
}