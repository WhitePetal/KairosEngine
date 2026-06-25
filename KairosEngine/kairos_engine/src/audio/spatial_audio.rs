use std::{collections::HashMap, time::Duration};

use kira::{
    AudioManager, Decibels, Easing, Mapping, Mix, Tween, Value,
    effect::reverb::{ReverbBuilder, ReverbHandle},
    listener::{ListenerHandle, ListenerId},
    sound::{PlaybackPosition, PlaybackState},
    track::{
        SendTrackBuilder, SendTrackHandle, SendTrackId, SpatialTrackBuilder, SpatialTrackHandle,
    },
};

use crate::{
    asset_loader::assets::{AssetsServer, AudioAssetsSystem},
    audio::spatial_audio::{
        spatial_audio_listener::SpatialAudioListenerComponent,
        spatial_audio_volume::{
            SpatialAudioVolume, SpatialAudioVolumeState, SpatialAudioVolumeTrackKey,
            SpatialAudioVolumeTrackLeaving, SpatialAudioVolumeTrackState, SpatialSoundHandle,
        },
    },
    ecs::world::World,
    math::{Vector, float3, quaternion},
    spatial::Transform,
};

pub mod spatial_audio_listener;
pub mod spatial_audio_reverb;
pub mod spatial_audio_volume;

pub struct SpatialAudioConfig {
    max_track_count: u8,
    max_listener_count: u8,
    cut_off_distance_sq: f32,
    audio_volume_leaving_duration: f32,
    default_reverb_distance_range: f32,
    default_reverb_min_volume: f32,
    default_reverb_max_volume: f32,
}
impl SpatialAudioConfig {
    pub fn new(
        max_track_count: u8,
        max_listener_count: u8,
        cut_off_distance_sq: f32,
        audio_volume_leaving_duration: f32,
        default_reverb_distance_range: f32,
        default_reverb_min_volume: f32,
        default_reverb_max_volume: f32,
    ) -> Self {
        Self {
            max_track_count,
            max_listener_count,
            cut_off_distance_sq,
            audio_volume_leaving_duration,
            default_reverb_distance_range,
            default_reverb_min_volume,
            default_reverb_max_volume,
        }
    }
}

struct Tracks {
    tracks: Vec<Option<SpatialTrackHandle>>,
    free_slots: Vec<u8>,
    used_track_count: u8,
}
impl Tracks {
    pub fn new(per_listener_track_capacity: u8) -> Self {
        Self {
            tracks: Vec::with_capacity(per_listener_track_capacity as usize),
            free_slots: Vec::with_capacity(per_listener_track_capacity as usize),
            used_track_count: 0,
        }
    }
    pub fn free_track(&mut self, index: u8) {
        if self.used_track_count == 0 {
            return;
        }

        if let Some(slot) = self.tracks.get_mut(index as usize) {
            slot.take();
            self.free_slots.push(index);
            self.used_track_count -= 1;
        }
    }

    pub fn use_track(
        &mut self,
        manager: &mut AudioManager,
        listener_id: ListenerId,
        reverb_track: SendTrackId,
        reverb_mapping: Mapping<Decibels>,
    ) -> (u8, &mut Option<SpatialTrackHandle>) {
        let index;
        let handle = manager
            .add_spatial_sub_track(
                listener_id,
                float3::ZERO,
                SpatialTrackBuilder::new()
                    .with_send(reverb_track, Value::FromListenerDistance(reverb_mapping)),
            )
            .ok();
        if self.free_slots.len() == 0 {
            index = self.tracks.len() as u8;
            self.tracks.push(handle);
        } else {
            index = self.free_slots.pop().unwrap();
            self.tracks[index as usize] = handle;
        }

        self.used_track_count += 1;
        (index, &mut self.tracks[index as usize])
    }

    pub fn used_track_count(&self) -> u8 {
        self.used_track_count
    }
}

struct ListenerInfo {
    pub tracks: Tracks,
    pub listener_id: ListenerId,
    pub position: float3,
}

pub struct SpatialAudioTracks {
    per_listener_track_capacity: u8,

    all_listeners: HashMap<ListenerId, (ListenerHandle, bool)>,

    listener_infos: Vec<ListenerInfo>,

    _reverb_handle: ReverbHandle,
    reverb_distance_mapping: Mapping<Decibels>,
    reverb_send_track: SendTrackHandle,

    config: SpatialAudioConfig,
}

impl SpatialAudioTracks {
    pub fn new(
        manager: &mut AudioManager,
        config: SpatialAudioConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reverb_builder = SendTrackBuilder::new();
        let reverb_handle =
            reverb_builder.add_effect(ReverbBuilder::new().mix(Mix::WET).damping(0.5));
        let reverb_send_track = manager.add_send_track(reverb_builder)?;
        let reverb_distance_mapping = Mapping {
            input_range: (0.0, config.default_reverb_distance_range as f64),
            output_range: (
                Decibels(config.default_reverb_min_volume),
                Decibels(config.default_reverb_max_volume),
            ),
            easing: Easing::Linear,
        };

        Ok(Self {
            per_listener_track_capacity: config.max_listener_count,
            all_listeners: HashMap::with_capacity((config.max_listener_count as usize) << 1),
            listener_infos: Vec::with_capacity(config.max_listener_count as usize),
            reverb_handle,
            reverb_distance_mapping,
            reverb_send_track,
            config,
        })
    }

    pub fn create_listener(&mut self, manager: &mut AudioManager) -> Option<ListenerId> {
        let handle = manager
            .add_listener(float3::ZERO, quaternion::IDENTITY)
            .ok();
        if let Some(handle) = handle {
            let id = handle.id();
            self.all_listeners.insert(id, (handle, false));
            Some(id)
        } else {
            None
        }
    }

    pub fn update(
        &mut self,
        assets_server: &mut AssetsServer,
        manager: &mut AudioManager,
        world: &mut World,
        delta_time: f32,
    ) {
        let listeners_iter = world
            .query_mut::<(&Transform, &mut SpatialAudioListenerComponent)>()
            .into_iter();
        if listeners_iter.len() == 0 {
            return;
        }

        let mut listeners = listeners_iter
            .map(|(trans, listener)| (*trans, *listener))
            .collect::<Box<_>>();
        let mut listener_capacity = self.listener_infos.capacity();
        if listeners.len() > listener_capacity {
            listeners
                .select_nth_unstable_by(listener_capacity, |x, y| y.1.priority.cmp(&x.1.priority));
        } else {
            listener_capacity = listeners.len()
        }

        self.update_listeners_inner(&mut listeners[0..listener_capacity]);

        self.update_audios(assets_server, manager, world, delta_time);

        // let volumes = world.query_mut::<(Entity, &TransformComponent, &SpatialAudioVolumeComponent)>().into_iter();
        // volumes.enumerate().for_each(|(i, (entity, _, volume))| {
        //     println!("audio volume {:?} => entity: {:?}, state: {:?}, tracks_state: {:?}", i, entity, volume.state, volume.track_states);
        // });
    }

    fn update_listeners_inner(
        &mut self,
        listeners: &mut [(Transform, SpatialAudioListenerComponent)],
    ) {
        let len = listeners.len();
        let per_listener_track_count = self.config.max_track_count / (len as u8);
        // 如果有新增的listener，那么每个 listener 可使用的track数会变少
        if per_listener_track_count < self.per_listener_track_capacity {
            for listnere in &mut self.listener_infos {
                listnere
                    .tracks
                    .tracks
                    .truncate(per_listener_track_count as usize);
            }
        }
        self.per_listener_track_capacity = per_listener_track_count;

        {
            let ids = listeners.iter().map(|(_, listener)| listener.listener_id);
            self.listener_infos
                .retain(|info| ids.clone().any(|id| info.listener_id == id));
        }

        let listeners = listeners.iter_mut();

        for (_, be_ref) in &mut self.all_listeners.values_mut() {
            *be_ref = false
        }

        for (trans, listener) in listeners {
            let id = listener.listener_id;
            let Some((handle, be_ref)) = self.all_listeners.get_mut(&id) else {
                continue;
            };
            *be_ref = true;
            handle.set_position(trans.position, Tween::default());
            handle.set_orientation(trans.rotation, Tween::default());
            match self
                .listener_infos
                .iter_mut()
                .find(|info| info.listener_id == id)
            {
                Some(info) => {
                    info.position = trans.position;
                }
                None => {
                    let info = ListenerInfo {
                        tracks: Tracks::new(self.per_listener_track_capacity),
                        listener_id: id,
                        position: trans.position,
                    };
                    self.listener_infos.push(info);
                }
            }
        }

        self.all_listeners.retain(|_, (_, be_ref)| *be_ref);
    }

    fn update_audios(
        &mut self,
        assets_server: &mut AssetsServer,
        manager: &mut AudioManager,
        world: &mut World,
        delta_time: f32,
    ) {
        // 先更新 kairos engine 端的 audio volume 数据
        let volumes = world
            .query_mut::<(&Transform, &mut SpatialAudioVolume)>()
            .into_iter()
            .map(|(_, volume)| volume);
        for volume in volumes {
            Self::update_audio_volume_state(
                assets_server,
                delta_time,
                self.config.audio_volume_leaving_duration,
                volume,
            );
        }

        // 再对每个 listener 更新 volumes
        // 分配 track、播放...
        for listener in &mut self.listener_infos {
            Self::update_listener_audios(
                assets_server,
                world,
                self.config.cut_off_distance_sq,
                self.config.audio_volume_leaving_duration,
                self.per_listener_track_capacity,
                manager,
                listener,
                self.reverb_send_track.id(),
                self.reverb_distance_mapping,
            )
        }
    }

    fn update_listener_audios(
        assets_server: &mut AssetsServer,
        world: &mut World,
        cut_off_dst_sq: f32,
        fade_time: f32,
        per_listener_track_count: u8,
        manager: &mut AudioManager,
        listener: &mut ListenerInfo,
        reverb_track: SendTrackId,
        reverb_mapping: Mapping<Decibels>,
    ) {
        // 首先，如果有 volume play completed 或者 leaved track
        // 那么先让它们free掉持有的track
        let volumes = world
            .query_mut::<(&Transform, &mut SpatialAudioVolume)>()
            .into_iter()
            .map(|(_, volume)| volume);
        for volume in volumes {
            Self::free_audio_volume_track(listener, volume);
        }

        // 找到前 k 个 距离 listener 最近的 可播放的 volumes
        let mut volumes = world
            .query_mut::<(&Transform, &mut SpatialAudioVolume)>()
            .into_iter()
            .filter(|(_, volume)| match volume.state {
                SpatialAudioVolumeState::Created => false,
                SpatialAudioVolumeState::WaitLoading => false,
                SpatialAudioVolumeState::Playing => true,
                SpatialAudioVolumeState::Paused => true,
                SpatialAudioVolumeState::Completed => false,
            })
            .map(|(trans, volume)| {
                let dst_sq = float3::distance_sq(listener.position, trans.position);
                (dst_sq, trans, volume)
            })
            .filter(|(dst, _, _)| *dst < cut_off_dst_sq)
            .collect::<Vec<_>>();

        let track_count;
        let volumes_len = volumes.len();
        if volumes_len > per_listener_track_count as usize {
            volumes.select_nth_unstable_by(per_listener_track_count as usize, |x, y| {
                x.0.total_cmp(&y.0)
            });
            track_count = per_listener_track_count;
        } else {
            track_count = volumes.len() as u8;
        }

        // 在 track 上 播放/更新 前k个 volumes
        // 由于可能在k之外有的volume之前持有着track
        // 因此这里 播放/更新的 volumes 数量可能少于k
        for (_, trans, volume) in &mut volumes[0..track_count as usize] {
            if !Self::play_audio_volume_in_track(
                assets_server,
                manager,
                listener,
                trans,
                volume,
                per_listener_track_count,
                reverb_track,
                reverb_mapping,
            ) {
                Self::leaving_audio_volume_in_track(fade_time, listener, trans, volume);
            }
        }

        // 剩下的 volume，如果持有 track，则进入 leaving 状态
        for (_, trans, volume) in &mut volumes[track_count as usize..volumes_len] {
            Self::leaving_audio_volume_in_track(fade_time, listener, trans, volume);
        }
    }

    fn update_audio_volume_state(
        assets_server: &mut AssetsServer,
        delta_time: f32,
        audio_volume_leaving_duration: f32,
        volume: &mut SpatialAudioVolume,
    ) {
        match volume.state {
            SpatialAudioVolumeState::Created => {
                if volume.auto_play {
                    volume.state = SpatialAudioVolumeState::WaitLoading;
                }
            }
            SpatialAudioVolumeState::WaitLoading => {
                let mut loaded = true;
                let audios = &volume.audios;
                for i in 0..audios.len() {
                    let audio = assets_server.get(&audios[i].clone());
                    match audio {
                        Some(_) => {}
                        None => {
                            loaded = false;
                        }
                    }
                }

                if loaded {
                    volume.state = SpatialAudioVolumeState::Playing;
                }
            }
            SpatialAudioVolumeState::Playing => {
                volume.playing_time = volume.playing_time + delta_time;

                let mut completed = volume.audio_handles.len() == volume.audios.len();
                for track_state in &mut volume.track_states {
                    Self::update_audio_track_state(
                        audio_volume_leaving_duration,
                        delta_time,
                        track_state,
                    );
                }

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
                volume.playing_time = 0.0;
                for track_state in &mut volume.track_states {
                    match track_state {
                        SpatialAudioVolumeTrackState::Playing(key) => {
                            *track_state = SpatialAudioVolumeTrackState::Leaved(*key)
                        }
                        SpatialAudioVolumeTrackState::Leaving(leaving) => {
                            *track_state = SpatialAudioVolumeTrackState::Leaved(leaving.track_key)
                        }
                        SpatialAudioVolumeTrackState::Leaved(_) => {}
                    }
                }
                // TODO: if auto_destroy, do destroy now
            }
        }
    }

    fn update_audio_track_state(
        audio_volume_leaving_duration: f32,
        delta_time: f32,
        track_state: &mut SpatialAudioVolumeTrackState,
    ) {
        match track_state {
            SpatialAudioVolumeTrackState::Playing(_) => {}
            SpatialAudioVolumeTrackState::Leaving(leaving) => {
                leaving.timer = leaving.timer + delta_time;
                if leaving.timer > audio_volume_leaving_duration {
                    *track_state = SpatialAudioVolumeTrackState::Leaved(leaving.track_key);
                }
            }
            SpatialAudioVolumeTrackState::Leaved(_) => {}
        }
    }

    fn free_audio_volume_track(listener: &mut ListenerInfo, volume: &mut SpatialAudioVolume) {
        for track_state in &volume.track_states {
            let SpatialAudioVolumeTrackState::Leaved(key) = track_state else {
                continue;
            };
            if key.listener_id != listener.listener_id {
                continue;
            }

            listener.tracks.free_track(key.track_index);
        }

        volume.track_states.retain(|state| {
            if let SpatialAudioVolumeTrackState::Leaved(_) = state {
                false
            } else {
                true
            }
        });
        volume.audio_handles.clear();
    }

    fn play_audio_volume_in_track(
        assets_server: &mut AssetsServer,
        manager: &mut AudioManager,
        listener: &mut ListenerInfo,
        trans: &Transform,
        volume: &mut SpatialAudioVolume,
        per_listener_track_count: u8,
        reverb_track: SendTrackId,
        reverb_mapping: Mapping<Decibels>,
    ) -> bool {
        let mut using_track_index = None;
        for track_state in &mut volume.track_states {
            let using = match track_state {
                SpatialAudioVolumeTrackState::Playing(key) => {
                    if key.listener_id == listener.listener_id {
                        Some(key.track_index)
                    } else {
                        None
                    }
                }
                SpatialAudioVolumeTrackState::Leaving(leaving) => {
                    if leaving.track_key.listener_id == listener.listener_id {
                        Some(leaving.track_key.track_index)
                    } else {
                        None
                    }
                }
                SpatialAudioVolumeTrackState::Leaved(_) => None,
            };
            if let Some(track) = using {
                using_track_index = Some(track);
                break;
            }
        }
        if let Some(track_index) = using_track_index {
            let track = &mut listener.tracks.tracks[track_index as usize];
            if let Some(track) = track {
                track.set_position(trans.position, Tween::default());
            }
            return true;
        }

        if listener.tracks.used_track_count() >= per_listener_track_count {
            return false;
        }

        let (track_index, track) =
            listener
                .tracks
                .use_track(manager, listener.listener_id, reverb_track, reverb_mapping);
        volume
            .track_states
            .push(SpatialAudioVolumeTrackState::Playing(
                SpatialAudioVolumeTrackKey {
                    listener_id: listener.listener_id,
                    track_index: track_index,
                },
            ));

        if let SpatialAudioVolumeState::Playing = volume.state
            && let Some(track) = track
        {
            for i in 0..volume.audios.len() {
                let audio = assets_server
                    .get::<AudioAssetsSystem>(&volume.audios[i])
                    .unwrap();
                track.set_position(trans.position, Tween::default());
                match track.play(
                    audio
                        .sound_data
                        .start_position(PlaybackPosition::Seconds(volume.playing_time as f64)),
                ) {
                    Ok(handle) => {
                        volume.audio_handles.push(SpatialSoundHandle::Some(handle));
                    }
                    Err(err) => {
                        println!(
                            "play spatial audio failed, play_audio_volume_in_track: {:?}",
                            err
                        );
                        volume.audio_handles.push(SpatialSoundHandle::Err);
                    }
                }
            }
        }
        true
    }

    fn leaving_audio_volume_in_track(
        fade_time: f32,
        listener: &mut ListenerInfo,
        trans: &Transform,
        volume: &mut SpatialAudioVolume,
    ) {
        for track_state in &mut volume.track_states {
            match track_state {
                SpatialAudioVolumeTrackState::Playing(key) => {
                    if key.listener_id != listener.listener_id {
                        continue;
                    }
                    if let Some(track) = &mut listener.tracks.tracks[key.track_index as usize] {
                        track.pause(Tween {
                            duration: Duration::from_secs_f32(fade_time),
                            ..Default::default()
                        });
                        track.set_position(trans.position, Tween::default());
                    }
                    *track_state =
                        SpatialAudioVolumeTrackState::Leaving(SpatialAudioVolumeTrackLeaving {
                            track_key: *key,
                            timer: 0.0,
                        });
                    break;
                }
                SpatialAudioVolumeTrackState::Leaving(leaving) => {
                    if leaving.track_key.listener_id != listener.listener_id {
                        continue;
                    }
                    if let Some(track) =
                        &mut listener.tracks.tracks[leaving.track_key.track_index as usize]
                    {
                        track.set_position(trans.position, Tween::default());
                    }
                    break;
                }
                SpatialAudioVolumeTrackState::Leaved(_) => {}
            }
        }
    }
}
