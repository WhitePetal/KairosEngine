use std::fmt::Debug;

use kira::{
    AudioManager, AudioManagerSettings, Capacities, DefaultBackend, Tween,
    listener::ListenerHandle,
    sound::PlaybackState,
    track::{MainTrackBuilder, TrackHandle},
};

use crate::{
    asset_loader::assets::AssetsServer,
    audio::spatial_audio_volume::{
        SpatialAudioVolumeComponent, SpatialAudioVolumeState, SpatialSoundHandle,
    },
    ecs::world::World,
    spatial::TransformComponent,
};

pub mod audio;
pub mod spatial_audio_volume;

pub struct AudioEngine {
    manager: AudioManager,
    lead_track: Option<TrackHandle>,
}

impl Debug for AudioEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEngine").finish_non_exhaustive()
    }
}

impl AudioEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings {
            capacities: Capacities {
                sub_track_capacity: 1024,
                ..Default::default()
            },
            main_track_builder: MainTrackBuilder::new().sound_capacity(1024),
            ..Default::default()
        })?;
        Ok(Self {
            manager,
            lead_track: None,
        })
    }

    pub fn add_spatial_listener(
        &mut self,
        transform: TransformComponent,
    ) -> Option<ListenerHandle> {
        self.manager
            .add_listener(transform.position, transform.rotation)
            .ok()
    }

    pub fn update(&mut self, world: &mut World, assets_server: &mut AssetsServer) {
        // spatial audio volumes system
        let spatial_audio_volumes =
            world.query_mut::<(&TransformComponent, &mut SpatialAudioVolumeComponent)>();
        for (trans, volume) in spatial_audio_volumes.into_iter() {
            let Some(track) = &mut volume.track else {
                continue;
            };
            match volume.state {
                SpatialAudioVolumeState::Created => {
                    if volume.auto_play {
                        volume.state = SpatialAudioVolumeState::WaitLoading;
                    }
                }
                SpatialAudioVolumeState::WaitLoading => {
                    let mut loaded = true;
                    let audios = &volume.audios;
                    let audio_handles = &mut volume.audio_handles;
                    for i in 0..audios.len() {
                        let handle = &mut audio_handles[i];
                        match handle {
                            SpatialSoundHandle::None => {
                                let audio = assets_server.get(&audios[i].clone());
                                match audio {
                                    Some(audio) => {
                                        let play =
                                            track.play(audio.sound_data.clone().loop_region(..));
                                        match play {
                                            Ok(mut play) => {
                                                play.pause(Tween::default());
                                                *handle = SpatialSoundHandle::Some(play);
                                            }
                                            Err(err) => {
                                                println!("play sound error: {:?}", err);
                                                *handle = SpatialSoundHandle::Err
                                            }
                                        }
                                    }
                                    None => {
                                        loaded = false;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    if loaded {
                        for handle in audio_handles {
                            match handle {
                                SpatialSoundHandle::Some(handle) => {
                                    handle.resume(Tween::default());
                                }
                                _ => {}
                            }
                        }
                        volume.state = SpatialAudioVolumeState::Playing;
                    }
                }
                SpatialAudioVolumeState::Playing => {
                    let mut completed = true;
                    let audio_handles = &mut volume.audio_handles;
                    for handle in audio_handles {
                        match handle {
                            SpatialSoundHandle::Some(handle) => {
                                if handle.state() != PlaybackState::Stopped {
                                    completed = false;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if completed {
                        volume.state = SpatialAudioVolumeState::Completed;
                    } else {
                        track.set_position(trans.position, Tween::default());
                    }
                }
                SpatialAudioVolumeState::Paused => {
                    todo!()
                }
                SpatialAudioVolumeState::Completed => {
                    // no do nothing
                    // TODO: if auto_destroy, do destroy now
                }
            }
        }
    }
}
