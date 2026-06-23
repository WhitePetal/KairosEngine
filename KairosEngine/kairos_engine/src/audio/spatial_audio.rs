use std::collections::HashSet;

use kira::{AudioManager, Tween, listener::ListenerId, track::{SpatialTrackBuilder, SpatialTrackHandle}};

use crate::{audio::spatial_audio::{spatial_audio_listener::SpatialAudioListenerComponent, spatial_audio_volume::SpatialAudioVolumeComponent}, ecs::{component_tuple::QueryIter, entity::Entity, world::World}, math::{self, Vector, float3}, spatial::TransformComponent};

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

    pub fn update(&mut self, manager: &mut AudioManager, world: &mut World) {
        let listeners = world.query_mut::<(Entity, &TransformComponent, &SpatialAudioListenerComponent)>().into_iter();
        let mut listener_entities = Vec::with_capacity(listeners.len());
        for (entity, _, listener) in listeners {
            listener_entities.push((entity, listener.priority));
        }
        let mut listener_capacity =  self.listeners.capacity();
        if listener_capacity < listener_entities.len() {
            listener_entities.select_nth_unstable_by(listener_capacity, |x, y| {
                y.1.cmp(&x.1)
            });
            listener_capacity = listener_entities.len()
        }

        self.update_listeners_inner(world, &listener_entities[0..listener_capacity]);

        self.update_audios(manager, world);
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

    fn update_audios(&mut self, manager: &mut AudioManager, world: &mut World) {
        let mut audios = world.query_mut::<(&TransformComponent, &mut SpatialAudioVolumeComponent)>().into_iter().collect::<Vec<_>>();
        let per_listener_track_count = self.per_listener_track_count;
        for listener in &mut self.listeners {
            Self::update_listener_info(manager, per_listener_track_count, listener, &mut audios)
        }
    }

    fn update_listener_info(manager: &mut AudioManager, per_listener_track_count: u8, listener: &mut ListenerInfo, audios: &mut [(&TransformComponent, &mut SpatialAudioVolumeComponent)]) {
        let mut track_count;
        if audios.len() > per_listener_track_count as usize {
            audios.select_nth_unstable_by(per_listener_track_count as usize, |x, y| {
                let dstX = float3::distance(x.0.position, listener.position);
                let dstY = float3::distance(y.0.position, listener.position);
                dstX.total_cmp(&dstY)
            });
            track_count = per_listener_track_count;
        } else {
            track_count = audios.len() as u8;
        }

        // create tracks
        let reused_count = track_count.min(listener.tracks.len() as u8);
        let need_push_count = track_count.saturating_sub(listener.tracks.len() as u8);

        for i in 0..reused_count {
            let i = i as usize;
            let pos = audios[i].0.position;
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
                    let pos = audios[i as usize].0.position;
                    track.set_position(pos, Tween::default());
                    listener.tracks.push(Some(track));
                },
                Err(err) => {
                    listener.tracks.push(None);
                    println!("create spatial sub track failed: {:?}", err);
                },
            }
        }

        listener.tracks[0..track_count as usize]
            .iter()
            .zip(audios.iter_mut().map(|x| &mut x.1))
            .for_each(|(track, audio)| {
                Self::play_audio(track, audio);
            });
    }

    fn play_audio(track: &Option<SpatialTrackHandle>, audio: &mut SpatialAudioVolumeComponent) {

    }
}
