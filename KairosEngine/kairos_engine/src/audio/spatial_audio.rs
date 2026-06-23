use std::collections::HashSet;

use kira::{AudioManager, Decibels, Mapping, Tween, Value, listener::ListenerId, modulator::tweener::TweenerBuilder, sound::PlaybackState, track::{SpatialTrackBuilder, SpatialTrackHandle}};

use crate::{asset_loader::assets::AssetsServer, audio::spatial_audio::{spatial_audio_listener::SpatialAudioListenerComponent, spatial_audio_volume::{SpatialAudioVolumeComponent, SpatialAudioVolumeState, SpatialSoundHandle}}, ecs::{component_tuple::QueryIter, entity::Entity, world::World}, math::{self, Vector, float3}, spatial::TransformComponent};

pub mod spatial_audio_volume;
pub mod spatial_audio_listener;

pub struct SpatialAudioConfig {
    max_track_count: u8,
    max_listener_count: u8,
    cut_off_distance: f32,
}
impl SpatialAudioConfig {
    pub fn new(max_track_count: u8, max_listener_count: u8, cut_off_distance: f32) -> Self {
        Self {
            max_track_count,
            max_listener_count,
            cut_off_distance,
        }
    }
}

struct ListenerInfo {
    pub tracks: Vec<Option<SpatialTrackHandle>>,
    pub listener_id: ListenerId,
    pub position: float3,
}

pub struct SpatialAudioTracks {
    per_listener_track_count: u8,
    listeners: Vec<ListenerInfo>,
    config: SpatialAudioConfig,
}

impl SpatialAudioTracks {
    pub fn new(config: SpatialAudioConfig) -> Self {
        Self { 
            per_listener_track_count: config.max_listener_count,
            listeners: Vec::with_capacity(config.max_listener_count as usize),
            config,
        }
    }

    pub fn update(&mut self, assets_server: &mut AssetsServer, manager: &mut AudioManager, world: &mut World) {
        let listeners = world.query_mut::<(Entity, &TransformComponent, &SpatialAudioListenerComponent)>().into_iter();
        if listeners.len() == 0 {
            return;
        }
        let mut listener_entities = Vec::with_capacity(listeners.len());
        for (entity, _, listener) in listeners {
            listener_entities.push((entity, listener.priority));
        }
        let mut listener_capacity =  self.listeners.capacity();
        if listener_entities.len() > listener_capacity  {
            listener_entities.select_nth_unstable_by(listener_capacity, |x, y| {
                y.1.cmp(&x.1)
            });
        } else {
            listener_capacity = listener_entities.len()
        }

        self.update_listeners_inner(world, &listener_entities[0..listener_capacity]);

        self.update_audios(assets_server, manager, world);
    }

    fn update_listeners_inner(&mut self, world: &mut World, listener_entities: &[(Entity, u32)]) {
        let len = listener_entities.len();
        let per_listener_track_count = self.config.max_track_count / (len as u8);
        if per_listener_track_count < self.per_listener_track_count {
            for listnere in &mut self.listeners {
                listnere.tracks.truncate(per_listener_track_count as usize);
            }
        }
        self.per_listener_track_count = per_listener_track_count;

        let entities = listener_entities.iter().map(|e| e.0);
        let listeners = entities.clone().map(|entity| {
            unsafe { world.get_unchecked::<&mut SpatialAudioListenerComponent>(entity) }
        });
        let ids = listeners.clone().map(|listener| {
            listener.handle.id()
        });

        let listeners = listeners.zip(entities.map(|entity|{
            unsafe {
                world.get_unchecked::<&TransformComponent>(entity)
            }
        }));

        self.listeners.retain(|info| ids.clone().any(|id| {
            info.listener_id == id
        }));
        for (listener, trans) in listeners {
            listener.handle.set_position(trans.position, Tween::default());
            listener.handle.set_orientation(trans.rotation, Tween::default());
            let id = listener.handle.id();
            match self.listeners.iter_mut().find(|info| info.listener_id == id) {
                Some(info) => {
                    info.position = trans.position;
                },
                None => {
                    let info = ListenerInfo {
                        tracks: Vec::with_capacity(self.per_listener_track_count as usize),
                        listener_id: id,
                        position: trans.position,
                    };
                    self.listeners.push(info);
                },
            }
        };
    }

    fn update_audios(&mut self, assets_server: &mut AssetsServer, manager: &mut AudioManager, world: &mut World) {
        let per_listener_track_count = self.per_listener_track_count;
        for listener in &mut self.listeners {
            let mut volumes = world.query_mut::<(&TransformComponent, &mut SpatialAudioVolumeComponent)>()
                .into_iter()
                .map(|(trans, audio)| {
                    (float3::distance(listener.position, trans.position), trans.position, audio)
                })
                .collect::<Vec<_>>();
            Self::update_listener_info(assets_server, manager, listener, &mut volumes, per_listener_track_count, self.config.cut_off_distance)
        }
    }

    fn update_listener_info(assets_server: &mut AssetsServer, manager: &mut AudioManager, listener: &mut ListenerInfo, volumes: &mut [(f32, float3, &mut SpatialAudioVolumeComponent)], per_listener_track_count: u8, cut_off: f32) {
        let track_count;
        if volumes.len() > per_listener_track_count as usize {
            volumes.select_nth_unstable_by(per_listener_track_count as usize, |x, y| {
                x.0.total_cmp(&y.0)
            });
            track_count = per_listener_track_count;
        } else {
            track_count = volumes.len() as u8;
        }

        // create tracks
        let reused_count = track_count.min(listener.tracks.len() as u8);
        let need_push_count = track_count.saturating_sub(listener.tracks.len() as u8);

        for i in 0..reused_count {
            let i = i as usize;
            let pos = volumes[i].1;
            match &mut listener.tracks[i] {
                Some(track) => {
                    track.set_position(pos, Tween::default());
                },
                None => {
                    match manager.add_spatial_sub_track(listener.listener_id, listener.position, SpatialTrackBuilder::new()) {
                        Ok(track) => {
                            listener.tracks[i] = Some(track)
                        },
                        Err(err) => {
                            println!("create spatial sub track failed: {:?}", err);
                        },
                    }
                }
            }
        }
        for i in reused_count .. reused_count + need_push_count {
            match manager.add_spatial_sub_track(listener.listener_id, listener.position, SpatialTrackBuilder::new()) {
                Ok(mut track) => {
                    let pos: float3 = volumes[i as usize].1;
                    track.set_position(pos, Tween::default());
                    listener.tracks.push(Some(track));
                },
                Err(err) => {
                    listener.tracks.push(None);
                    println!("create spatial sub track failed: {:?}", err);
                },
            }
        }

        let mut volumes = volumes.iter_mut();
        listener.tracks[0..track_count as usize]
            .iter_mut()
            .zip(&mut volumes)
            .for_each(|(track, (_, _, volume))| {
                Self::play_in_track_audio(assets_server, track, volume);
            });


        for (dst, pos, volume) in volumes {
            let dst = *dst;
            if dst > cut_off {
                continue;
            }

            let mut atten = 1.0 - dst / cut_off;
            atten = atten * atten;
            Self::play_in_background_audio(assets_server, manager, volume, atten);
        }
    }

    fn play_in_track_audio(assets_server: &mut AssetsServer, track: &mut Option<SpatialTrackHandle>, volume: &mut SpatialAudioVolumeComponent) {
        let Some(track) = track else {
            return;
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
                                        let play = track.play(audio.sound_data.clone());
                                        match play {
                                            Ok(mut play) => {
                                                play.pause(Tween::default());
                                                *handle = SpatialSoundHandle::Some(play);
                                            }
                                            Err(err) => {
                                                eprintln!("play sound error: {:?}, {:?}", err, "track play failed");
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

    fn play_in_background_audio(assets_server: &mut AssetsServer, manager: &mut AudioManager, volume: &mut SpatialAudioVolumeComponent, atten: f32) {
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
                                        let db = Decibels(-60.0 * (1.0 - atten) - 60.0 * atten);
                                        let play = manager.play(audio.sound_data.volume(Value::Fixed(db)));
                                        match play {
                                            Ok(mut play) => {
                                                play.pause(Tween::default());
                                                *handle = SpatialSoundHandle::Some(play);
                                            }
                                            Err(err) => {
                                                println!("play sound error: {:?}, {:?}", err, "manager paly failed");
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
