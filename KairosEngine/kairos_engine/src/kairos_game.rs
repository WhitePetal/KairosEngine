use std::path::PathBuf;

use crate::{
    asset_loader::assets::{AudioAssetsSystem, MaterialAssetsSystem, MeshAssetsSystem},
    audio::{
        background::BackgroundAudio,
        spatial::{
            spatial_audio_listener::SpatialAudioListenerComponent,
            spatial_audio_reverb::SpatialAudioReverb,
        },
    },
    graphics::{
        camera::Camera, graphics_graph::GraphicsCommand, lod_mesh_component::LODMesh,
        material_component::MaterialComponent, mesh::SerializedMeshAsset,
    },
    inputs::Input,
    kairos_editor::Engine,
    math::{self, float3, quaternion},
    physics::{
        collider::{Collider, ColliderMaterial},
        rigid_body::RigidBody,
    },
    spatial::{AABB, Transform},
};

pub struct KairosGame {}

impl KairosGame {
    pub fn new(engine: &mut Engine) -> Self {
        engine.input_engine.registe_input(
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyW),
            Input::W,
        );
        engine.input_engine.registe_input(
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyA),
            Input::A,
        );
        engine.input_engine.registe_input(
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyS),
            Input::S,
        );
        engine.input_engine.registe_input(
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyD),
            Input::D,
        );

        let assets_server = &mut engine.assets_server;

        SerializedMeshAsset::save_from_glb_file(PathBuf::from("res/models/Suzanne.glb"));

        let mesh = assets_server.load::<MeshAssetsSystem>(
            PathBuf::from("res/models/Suzanne.mesh"),
            // None::<fn(&mut MeshAsset)>,
        );
        let material = assets_server.load::<MaterialAssetsSystem>(
            PathBuf::from("res/materials/material.mat"),
            // None::<fn(&mut MaterialAsset)>,
        );

        // let pad_audio = StaticSoundData::from_file("res/audios/pad.ogg").unwrap().loop_region(..);

        // let pad = SerializedAudioAsset {
        //     source_path: PathBuf::from("res/audios/pad.ogg"),
        //     audio_asset_settings: SerializedAudioAssetSettings::from_static_sound_data(&pad_audio)
        // };
        // let _ = pad.save_to_file();

        let background_audio =
            assets_server.load::<AudioAssetsSystem>(PathBuf::from("res/audios/pad.audio"));

        let blip_audio =
            assets_server.load::<AudioAssetsSystem>(PathBuf::from("res/audios/blip.audio"));

        let cam_pos = float3::new(0.0, 1.0, -2.0);
        let cam_target = float3::new(0.0, 0.0, 0.0);
        let cam_trans = Transform::look_at(cam_pos, cam_target, float3::UP);
        let camera = Camera::new(45.0, 16.0 / 9.0, 0.3, 100.);
        engine.world.spawn((cam_trans, camera));

        if let Some(listener_id) = engine.audio_engine.create_listener() {
            engine.world.spawn((
                cam_trans,
                SpatialAudioListenerComponent {
                    listener_id,
                    priority: 100,
                },
            ));
        }

        let background_audio = BackgroundAudio::new(background_audio, true);
        engine.world.spawn((background_audio,));

        let spatial_audio_reverb = SpatialAudioReverb::new(
            20.0,
            -12.0,
            24.0,
            0.2,
            0.2,
            0.6,
            AABB {
                min: float3::new(-20.0, -20.0, -20.0),
                max: float3::new(20.0, 20.0, 20.0),
            },
        );
        engine.world.spawn(spatial_audio_reverb);

        const NUM_INSTANCES_PER_ROW: i32 = 20;
        // engine.world.spawn_batch(
        //     (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW)
        //         .flat_map(|z| (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW).map(move |x| (x, z)))
        //         .map(|(x, z)| {
        //             let scale = float3::new(0.05, 0.05, 0.05);
        //             let position = float3::new(x as f32, 0.0, z as f32) * scale * 2.0;
        //             let rotation = quaternion::identity();
        //             let transform = Transform::new(position, rotation, scale);
        //             let audios = smallvec![blip_audio.clone()];
        //             let spatial_audio_volume =
        //                 SpatialAudioVolume::new(audios, true, rand::random_range(0.0..5.0));

        //             (
        //                 transform,
        //                 LODMesh::new(mesh.clone()),
        //                 Material::new(material.clone()),
        //                 spatial_audio_volume,
        //             )
        //         }),
        // );

        let plane_transform = Transform::new(
            float3::new(0.0, -1.0, 0.0),
            quaternion::IDENTITY,
            float3::ONE,
        );
        let plane_collider = Collider::box_collider(&mut engine.physics_engine, 100.0, 0.1, 100.0);
        plane_collider.set_position(&mut engine.physics_engine, plane_transform.position);

        let ball_transform = Transform::new(
            float3::new(0.0, 10.0, 0.0),
            quaternion::IDENTITY,
            float3::ONE,
        );
        let ball_rigid_body = RigidBody::with_sphere_collider_with_material(
            &mut engine.physics_engine,
            0.5,
            ColliderMaterial { restitution: 0.8 },
        );
        ball_rigid_body.set_position(&mut engine.physics_engine, ball_transform.position);

        SerializedMeshAsset::save_from_glb_file(PathBuf::from("res/models/Ball.glb"));

        let plan_mesh_asset =
            assets_server.load::<MeshAssetsSystem>(PathBuf::from("res/models/Plane.mesh"));
        let ball_mesh_asset =
            assets_server.load::<MeshAssetsSystem>(PathBuf::from("res/models/Ball.mesh"));
        let plane_mesh = LODMesh::new(plan_mesh_asset);
        let ball_mesh = LODMesh::new(ball_mesh_asset);
        engine.world.spawn((
            plane_transform,
            plane_collider,
            plane_mesh,
            MaterialComponent::new(material.clone()),
        ));
        engine.world.spawn((
            ball_transform,
            ball_rigid_body,
            ball_mesh,
            MaterialComponent::new(material.clone()),
        ));

        Self {}
    }

    pub fn update(&mut self, engine: &mut Engine) {
        engine.time.update();
        let total_time = engine.time.total_time().as_secs_f32();
        let delta_time = engine.time.delta_time().as_secs_f32();

        // let transfoms = engine.world.query_mut::<&mut Transform>().into_iter();
        // transfoms.for_each(|trans| {
        //     let position = &mut trans.position;
        //     let x = position.x();
        //     let y = math::sin(x + total_time * 0.5);
        //     *position = float3::new(x, y, position.z());
        // });

        engine
            .audio_engine
            .update(&mut engine.assets_server, &mut engine.world, delta_time);

        engine.physics_engine.update(&mut engine.world, delta_time);

        engine.input_engine.update(delta_time);
    }

    pub fn render(&self, engine: &mut Engine, graphics_command: &mut GraphicsCommand) {
        let renderers = engine
            .world
            .query_mut::<(&Transform, &LODMesh, &MaterialComponent)>()
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
