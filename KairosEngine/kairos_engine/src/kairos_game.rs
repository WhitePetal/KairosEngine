use std::path::PathBuf;

use crate::{
    asset_loader::assets::{
        MaterialAssetsSystem, MeshAssetsSystem, ShaderAssetsSystem, TextureAssetsSystem,
    },
    base_components::TransformComponent,
    ecs::world::World,
    graphics::{
        graphics_graph::GraphicsCommand, lod_mesh_component::LODMeshComponent,
        material_component::MaterialComponent,
    },
    math::{float3, quaternion},
};

pub struct KairosGame {}

impl KairosGame {
    pub fn new(world: &mut World) -> Self {
        let assets_server = world.assets_server_mut();
        assets_server.push(TextureAssetsSystem::new());
        assets_server.push(ShaderAssetsSystem::new());
        assets_server.push(MaterialAssetsSystem::new());
        assets_server.push(MeshAssetsSystem::new());

        let mesh = assets_server.load::<MeshAssetsSystem>(PathBuf::from("res/models/Suzanne.mesh"));
        let material =
            assets_server.load::<MaterialAssetsSystem>(PathBuf::from("res/materials/material.mat"));

        const NUM_INSTANCES_PER_ROW: i32 = 5;
        world.spawn_batch(
            (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW)
                .flat_map(|z| (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW).map(move |x| (x, z)))
                .map(|(x, z)| {
                    let position = float3::new(x as f32, 0.0, z as f32);
                    let rotation = quaternion::identity();
                    let scale = float3::new(1.0, 1.0, 1.0);
                    (
                        TransformComponent::new(position, rotation, scale),
                        LODMeshComponent::new(mesh.clone()),
                        MaterialComponent::new(material.clone()),
                    )
                }),
        );

        Self {}
    }

    pub fn update(&self, world: &mut World) {
        world.time.update();
        let total_time = world.time.total_time().as_secs_f32();

        let transfoms = world.query_mut::<&mut TransformComponent>().into_iter();
        transfoms.for_each(|trans| {
            let position = &mut trans.position;
            let x = position.x();
            let y = (x + total_time).sin();
            *position = float3::new(x, y, position.z());
        });
    }

    pub fn render(&self, world: &mut World, graphics_command: &mut GraphicsCommand) {
        let renderers = world
            .query_mut::<(&TransformComponent, &LODMeshComponent, &MaterialComponent)>()
            .into_iter();
        renderers.for_each(|(trans, lod, mat)| {
            graphics_command.draw(
                lod.lod0.clone(),
                mat.material.clone(),
                trans.get_local_to_world(),
            );
        });
    }
}
