use std::{path::PathBuf, time::Duration};

use kira::{clock::{ClockHandle, ClockId, ClockTime}, modulator::lfo::LfoHandle, sound::static_sound::{StaticSoundData, StaticSoundHandle}};

use crate::{
    asset_loader::assets::{
        AudioAssetsSystem, MaterialAssetsSystem, MeshAssetsSystem, ShaderAssetsSystem,
        TextureAssetsSystem,
    },
    spatial::TransformComponent,
    ecs::world::World,
    graphics::{
        graphics_graph::GraphicsCommand, lod_mesh_component::LODMeshComponent,
        material_component::MaterialComponent,
    },
    math::{self, float3, quaternion},
};

pub struct KairosGame {
    audio_tweener: Option<kira::modulator::tweener::TweenerHandle>,
    audio_change_time: f32,
    audio_tweener_target: f64,
    lfo_handle: Option<LfoHandle>,
    audio_clock: Option<ClockHandle>,
    audio_clock_time: Option<ClockTime>,
    score_auido: Option<StaticSoundHandle>,
}

impl KairosGame {
    pub fn new(world: &mut World) -> Self {
        let assets_server = world.assets_server_mut();
        assets_server.push(TextureAssetsSystem::new());
        assets_server.push(ShaderAssetsSystem::new());
        assets_server.push(MaterialAssetsSystem::new());
        assets_server.push(MeshAssetsSystem::new());
        assets_server.push(AudioAssetsSystem::new());

        let mesh = assets_server
            .load::<MeshAssetsSystem>(
                PathBuf::from("res/models/Suzanne.mesh"),
                // None::<fn(&mut MeshAsset)>,
            )
            .unwrap();
        let material = assets_server
            .load::<MaterialAssetsSystem>(
                PathBuf::from("res/materials/material.mat"),
                // None::<fn(&mut MaterialAsset)>,
            )
            .unwrap();

        const NUM_INSTANCES_PER_ROW: i32 = 10;
        world.spawn_batch(
            (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW)
                .flat_map(|z| (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW).map(move |x| (x, z)))
                .map(|(x, z)| {
                    let scale = float3::new(0.05, 0.05, 0.05);
                    let position = float3::new(x as f32, 0.0, z as f32) * scale * 2.0;
                    let rotation = quaternion::identity();
                    (
                        TransformComponent::new(position, rotation, scale),
                        LODMeshComponent::new(mesh.clone()),
                        MaterialComponent::new(material.clone()),
                    )
                }),
        );

        // Self {
        //     audio_tweener: world
        //         .audio_engine_mut()
        //         .as_mut()
        //         .and_then(|x| x.dynamic_music().ok()),
        //     audio_change_time: 0.0,
        //     audio_tweener_target: 1.0,
        // }

        // Self {
        //     audio_tweener: None,
        //     audio_change_time: 0.0,
        //     audio_tweener_target: 1.0,
        //     lfo_handle: world.audio_engine_mut().as_mut().and_then(|x| x.ghost_noise().ok())
        // }

        // let audio_clock = world.audio_engine_mut().as_mut().and_then(|x| x.metronome().ok());
        // let audio_clock_time = audio_clock.as_ref().map(|x| {
        //     ClockTime {
        //         clock: x.id(),
        //         ticks: 0,
        //         fraction: 0.0
        //     }
        // });
        // Self {
        //     audio_tweener: None,
        //     audio_change_time: 0.0,
        //     audio_tweener_target: 1.0,
        //     lfo_handle: None,
        //     audio_clock,
        //     audio_clock_time,
        // }

        // Self {
        //     audio_tweener: None,
        //     audio_change_time: 0.0,
        //     audio_tweener_target: 1.0,
        //     lfo_handle: None,
        //     audio_clock: None,
        //     audio_clock_time: None,
        //     score_auido:  world.audio_engine_mut().as_mut().and_then(|x| x.score_counter().ok())
        // }

        if let Some(audio) = world.audio_engine_mut().as_mut() {
            audio.seamless_loop_with_intro();
        }
        Self {
            audio_tweener: None,
            audio_change_time: 0.0,
            audio_tweener_target: 1.0,
            lfo_handle: None,
            audio_clock: None,
            audio_clock_time: None,
            score_auido: None,
        }
    }

    pub fn update(&mut self, world: &mut World) {
        world.time.update();
        let total_time = world.time.total_time().as_secs_f32();

        // if let Some(audio_clock) = self.audio_clock.as_ref() {
        //     let pre_audio_clock_time = self.audio_clock_time.as_mut().unwrap();
        //     if let Some(audio) = world.audio_engine_mut().as_mut() {
        //         audio.metronome_update(pre_audio_clock_time, audio_clock);
        //     }
        // }
        if let Some(audio) = world.audio_engine_mut().as_mut() {
            if let Some(sound) = &mut self.score_auido {
                audio.score_counter_update(sound, total_time);
            }
        }

        if (total_time - self.audio_change_time) > 10.0 {
            if let Some(audio_tweener) = &mut self.audio_tweener {
                audio_tweener.set(
                    self.audio_tweener_target,
                    kira::Tween {
                        duration: Duration::from_secs(10),
                        ..Default::default()
                    },
                );
                self.audio_tweener_target = 1.0 - self.audio_tweener_target;
                println!("change audio tweener target: {}", self.audio_tweener_target)
            }
            self.audio_change_time = total_time
        }

        let transfoms = world.query_mut::<&mut TransformComponent>().into_iter();
        transfoms.for_each(|trans| {
            let position = &mut trans.position;
            let x = position.x();
            let y = math::sin(x * 4.0 + total_time);
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
