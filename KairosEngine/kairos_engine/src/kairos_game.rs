use std::path::PathBuf;

use smallvec::smallvec;

use crate::{
    asset_loader::assets::{AudioAssetsSystem, MaterialAssetsSystem, MeshAssetsSystem},
    audio::spatial_audio::{
        spatial_audio_listener::SpatialAudioListenerComponent,
        spatial_audio_volume::SpatialAudioVolumeComponent,
    },
    graphics::{
        graphics_graph::GraphicsCommand, lod_mesh_component::LODMeshComponent,
        material_component::MaterialComponent,
    },
    kairos_editor::Engine,
    math::{self, float3, quaternion},
    spatial::TransformComponent,
};

pub struct KairosGame {}

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

        // 与 scene_window 相机一致: pos (0,1,-2), target (0,0,0)
        // 1. 从 identity forward (0,0,-1) 旋转到 cam_forward 的四元数
        let identity_forward = float3::new(0.0, 0.0, -1.0);
        let target_forward = math::normalize(float3::new(0.0, -1.0, 2.0));
        let dot = math::dot(&identity_forward, &target_forward);
        let (cam_forward, dot_abs) = if dot < 0.0 {
            (
                float3::new(
                    -target_forward.x(),
                    -target_forward.y(),
                    -target_forward.z(),
                ),
                -dot,
            )
        } else {
            (target_forward, dot)
        };
        let q_forward = if dot_abs > 0.9999 {
            quaternion::identity()
        } else {
            let axis = math::cross(identity_forward, cam_forward);
            let w = 1.0 + dot_abs;
            let len =
                math::sqrt(axis.x() * axis.x() + axis.y() * axis.y() + axis.z() * axis.z() + w * w);
            quaternion::new(axis.x() / len, axis.y() / len, axis.z() / len, w / len)
        };
        // 2. 绕局部 forward 滚转 180°，把右耳从 +X 翻到 -X，对齐相机的 cross(forward,up)
        let q_roll = quaternion::new(0.0, 0.0, 1.0, 0.0); // 180° around local Z
        let cam_rotation = q_forward * q_roll;
        let cam_trans =
            TransformComponent::new(float3::new(0.0, 1.0, -2.0), cam_rotation, float3::ONE);

        if let Some(listener_id) = engine.audio_engine.create_listener() {
            engine.world.spawn((
                cam_trans,
                SpatialAudioListenerComponent {
                    listener_id,
                    priority: 100,
                },
            ));
        }

        const NUM_INSTANCES_PER_ROW: i32 = 1;
        engine.world.spawn_batch(
            (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW)
                .flat_map(|z| (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW).map(move |y| (y, z)))
                .map(|(y, z)| {
                    let scale = float3::new(0.05, 0.05, 0.05);
                    let position = float3::new(0.0, y as f32, z as f32) * scale * 2.0;
                    let rotation = quaternion::identity();
                    let transform = TransformComponent::new(position, rotation, scale);
                    let audios = smallvec![blip_audio.clone()];
                    let spatial_audio_volume = SpatialAudioVolumeComponent::new(audios, true);

                    (
                        transform,
                        LODMeshComponent::new(mesh.clone()),
                        MaterialComponent::new(material.clone()),
                        spatial_audio_volume,
                    )
                }),
        );

        Self {}
    }

    pub fn update(&mut self, engine: &mut Engine) {
        engine.time.update();
        let total_time = engine.time.total_time().as_secs_f32();
        let delta_time = engine.time.delta_time().as_secs_f32();

        let transfoms = engine
            .world
            .query_mut::<&mut TransformComponent>()
            .into_iter();
        transfoms.for_each(|trans| {
            let position = &mut trans.position;
            let y = position.y();
            let x = math::sin(y * 4.0 + total_time);
            *position = float3::new(x, y, position.z());
        });

        engine
            .audio_engine
            .update(&mut engine.assets_server, &mut engine.world, delta_time);
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
