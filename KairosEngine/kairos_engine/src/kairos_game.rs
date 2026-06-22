use std::path::PathBuf;

use kira::listener::ListenerHandle;
use smallvec::smallvec;

use crate::{
    asset_loader::assets::{
        AudioAssetsSystem, MaterialAssetsSystem, MeshAssetsSystem,
    },
    audio::{spatial_audio_volume::SpatialAudioVolumeComponent},
    graphics::{
        graphics_graph::GraphicsCommand, lod_mesh_component::LODMeshComponent,
        material_component::MaterialComponent,
    },
    kairos_editor::Engine,
    math::{self, float3, quaternion},
    spatial::TransformComponent,
};

pub struct KairosGame {
    listener: ListenerHandle,
}

impl KairosGame {
    pub fn new(engine: &mut Engine) -> Self {
        let assets_server = &mut engine.assets_server;

        let mesh = assets_server.load::<MeshAssetsSystem>(
            PathBuf::from("res/models/Suzanne.mesh"),
            // None::<fn(&mut MeshAsset)>,
        );
        let material = assets_server.load::<MaterialAssetsSystem>(
            PathBuf::from("res/materials/material.mat"),
            // None::<fn(&mut MaterialAsset)>,
        );

        let blip_audio =
            assets_server.load::<AudioAssetsSystem>(PathBuf::from("res/audios/blip.audio"));

        let cam_trans = TransformComponent::new(
            float3::new(0.0, 1.0, -2.0),
            quaternion::identity(),
            float3::ONE,
        );
        let listener = engine.audio_engine.add_spatial_listener(cam_trans).unwrap();

        const NUM_INSTANCES_PER_ROW: i32 = 10;
        engine.world.spawn_batch(
            (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW)
                .flat_map(|z| (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW).map(move |x| (x, z)))
                .map(|(x, z)| {
                    let scale = float3::new(0.05, 0.05, 0.05);
                    let position = float3::new(x as f32, 0.0, z as f32) * scale * 2.0;
                    let rotation = quaternion::identity();
                    let transform = TransformComponent::new(position, rotation, scale);
                    let audios = smallvec![blip_audio.clone()];
                    let spatial_audio_volume = SpatialAudioVolumeComponent::new(
                        &mut engine.audio_engine,
                        &listener,
                        transform,
                        audios,
                        true,
                    );

                    (
                        transform,
                        LODMeshComponent::new(mesh.clone()),
                        MaterialComponent::new(material.clone()),
                        spatial_audio_volume,
                    )
                }),
        );

        Self {
            listener
        }
    }

    pub fn update(&mut self, engine: &mut Engine) {
        engine.time.update();
        let total_time = engine.time.total_time().as_secs_f32();

        let transfoms = engine
            .world
            .query_mut::<&mut TransformComponent>()
            .into_iter();
        transfoms.for_each(|trans| {
            let position = &mut trans.position;
            let x = position.x();
            let y = math::sin(x * 4.0 + total_time);
            *position = float3::new(x, y, position.z());
        });

        engine
            .audio_engine
            .update(&mut engine.world, &mut engine.assets_server);
    }

    pub fn render(&self, engine: &mut Engine, graphics_command: &mut GraphicsCommand) {
        let renderers = engine
            .world
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
