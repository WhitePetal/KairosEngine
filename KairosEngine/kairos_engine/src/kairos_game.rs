use std::path::PathBuf;

use crate::{
    asset_loader::assets::{
        AudioAssetHandle, AudioAssetsSystem, MaterialAssetsSystem, MeshAssetsSystem,
    },
    audio::AudioEngine,
    audio::background::BackgroundAudio,
    audio::spatial::{
        spatial_audio_listener::SpatialAudioListenerComponent,
        spatial_audio_reverb::SpatialAudioReverb, spatial_audio_volume::SpatialAudioVolume,
    },
    graphics::{
        camera::Camera, lod_mesh_component::LODMesh, material_component::MaterialComponent,
        mesh::SerializedMeshAsset, view_port::GameView,
    },
    inputs::Input,
    kairos_editor::Engine,
    math::{float3, quaternion},
    physics::{PhysicsEngine, collider::ColliderMaterial},
    spatial::AABB,
};
use kairos_ecs::world::World;
use kairos_transform::LocalTransform;

// ── Minimal audible scene ─────────────────────────────────────────────
//
// The scene exists to make the restored audio path audible and drivable with no
// graphics/physics/input dependency at all: one drifting listener, one reverb
// zone, a ring of spatial volumes and one background track.
//
// The listener's circular drift is **temporary scaffolding** — it is what gives
// the spatial path something to react to while input and the camera are still
// being migrated, and it is deliberately pure local arithmetic so that it can be
// deleted in one piece: when camera/input land, drop `listener_drift_angle`,
// `KairosGame::drift_listener` and the `LISTENER_DRIFT_*` constants, and hang
// the listener component on the camera entity instead.

/// Centre of the listener's circular drift, in world space — the axis the volume
/// ring is built around.
const LISTENER_DRIFT_CENTER: float3 = float3::ZERO;

/// Radius of the listener's circular drift, in metres. Smaller than the volume
/// ring so the drift sweeps the listener across every bearing of it.
const LISTENER_DRIFT_RADIUS: f32 = 6.0;

/// Angular speed of the drift, in radians per second: one lap per
/// `TAU / LISTENER_DRIFT_ANGULAR_SPEED` ≈ 15.7 s, slow enough that a track
/// handover has settled before the next one starts.
const LISTENER_DRIFT_ANGULAR_SPEED: f32 = 0.4;

/// Angle the listener starts at: the -Z bearing, i.e. just *outside* the reverb
/// zone, so that the first lap makes the zone's boundary crossing audible.
const LISTENER_DRIFT_START_ANGLE: f32 = std::f32::consts::PI;

/// Number of [`SpatialAudioVolume`] entities on the scene ring.
///
/// Deliberately larger than the engine's per-listener track capacity
/// (`MAX_SPATIAL_TRACK_COUNT` = 8): only the eight volumes nearest the listener
/// can hold a track at any moment, so four of these twelve are always losing the
/// competition for one — and which four keeps changing as the listener drifts.
/// That competition is the handover the scene exists to make audible.
const AUDIO_VOLUME_COUNT: usize = 12;

/// Distances from the scene origin the ring's volumes sit at, in metres: volume
/// `index` sits on lane `index % 3`, one volume every
/// `TAU / AUDIO_VOLUME_COUNT`, so one ring interleaves three concentric lanes at
/// 16 m, 26 m and 36 m.
///
/// Mixing near and far emitters around the ring, rather than parking them all at
/// one distance, is what makes kira's distance attenuation audible. kira's
/// default spatial track fades from 0 dB at 1 m to -60 dB at 100 m and
/// interpolates *in decibels* (`Tweenable for Decibels` lerps
/// `Decibels::SILENCE` → `IDENTITY`), so it costs roughly 0.6 dB per metre: with
/// the listener drifting on a 6 m-radius orbit, the ring's nearest emitter
/// arrives about 5..7 dB down and its farthest about 24..25 dB down.
///
/// Interleaving the lanes — rather than giving each lane a contiguous arc — is
/// what makes the handover audible: it keeps the emitters on either side of the
/// eight-track boundary within about 1 dB of each other, so giving up a track is
/// heard as a fade rather than as a level jump, and it lets all twelve volumes
/// take a turn holding one as the listener drifts past.
const AUDIO_VOLUME_LANE_RADII: [f32; 3] = [16.0, 26.0, 36.0];

/// Width of the window the ring's looping blips are given random start offsets
/// in, in seconds. The blip loops over ~0.5 s, so a window that is not a whole
/// multiple of that loop length leaves the twelve copies spread out of phase.
const AUDIO_VOLUME_START_OFFSET_WINDOW: f32 = 5.0;

/// Bounds of the reverb zone: the +Z half of the listener's orbit.
///
/// Zones are tested against the *listener*, not against emitters: a zone whose
/// bounds contain the listener decides the listener's reverb send, so the bounds
/// are sized to the listener's 6 m orbit rather than to the 16..36 m volume ring.
/// A zone covering `z >= 0` therefore puts the listener inside it for half of
/// every lap and outside it for the other half: it crosses the boundary twice
/// per 15.7 s lap.
const REVERB_ZONE_MIN: float3 = float3::new(-10.0, -4.0, 0.0);
const REVERB_ZONE_MAX: float3 = float3::new(10.0, 4.0, 10.0);

/// Reverb-zone settings, in the units `SpatialAudioReverb` takes them — the
/// pre-fork demo's values, unchanged.
///
/// KNOWN ISSUE (pre-existing in the engine, not introduced by this scene):
/// `distance_range`, `min_volume` and `max_volume` have **no audible effect**.
/// `SpatialAudioTracks::update_reverbs` feeds them to
/// `SendTrackHandle::set_volume(Value::FromListenerDistance(..))`, but kira
/// 0.12.1 processes send tracks with an `Info` whose `spatial_track_info` is
/// `None` (`Mixer::process`), so `Info::listener_distance()` never resolves,
/// `Parameter::calculate_new_raw_value` returns `None`, and the send volume
/// stays at the `SendTrackBuilder` default of `Decibels::IDENTITY`. Only
/// `feed_back`, `damping` and `mix` reach the effect.
///
/// KNOWN ISSUE (also pre-existing): those three are written only while a zone
/// contains the listener, and `update_reverbs` never resets them when none does,
/// so they latch. Entering the zone is audible — the listener starts outside it,
/// where the reverb still has the settings `update_listeners_inner` built it
/// with (kira's default feedback 0.9, the builder's `damping(0.5)`, `Mix::WET`),
/// and the zone replaces them with the shorter, brighter, 60 %-wet values above
/// — but leaving the zone is not: nothing puts the reverb back.
const REVERB_DISTANCE_RANGE: f32 = 20.0;
const REVERB_MIN_VOLUME: f32 = -12.0;
const REVERB_MAX_VOLUME: f32 = 24.0;
const REVERB_FEEDBACK: f32 = 0.2;
const REVERB_DAMPING: f32 = 0.2;
const REVERB_MIX: f32 = 0.6;

/// A point on a horizontal circle: `radius` metres from `center`, at `angle`
/// radians. `angle = 0` is the +Z bearing, and `angle` increases towards +X.
///
/// Both the listener's drift orbit and the volume ring are circles of this shape
/// around [`LISTENER_DRIFT_CENTER`], so they share this one step from polar to
/// world coordinates.
fn point_on_circle(center: float3, radius: f32, angle: f32) -> float3 {
    center + float3::new(radius * angle.sin(), 0.0, radius * angle.cos())
}

/// Where the listener sits on its drift orbit at `angle` radians.
fn listener_drift_position(angle: f32) -> float3 {
    point_on_circle(LISTENER_DRIFT_CENTER, LISTENER_DRIFT_RADIUS, angle)
}

/// The listener's placement at `angle`: on the drift orbit, facing its centre.
///
/// Facing the centre keeps every bearing of the ring in front of the listener,
/// so the drift is heard as the ring sweeping past rather than as the listener
/// turning away from it.
fn listener_drift_transform(angle: f32) -> LocalTransform {
    LocalTransform::look_at(
        listener_drift_position(angle),
        LISTENER_DRIFT_CENTER,
        float3::UP,
    )
}

// ── KairosGame ────────────────────────────────────────────────────────

pub struct KairosGame {
    /// Current angle, in radians, of the listener's circular drift — see the
    /// scene notes above for why this is temporary.
    listener_drift_angle: f32,
}

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

        let material = assets_server.load::<MaterialAssetsSystem>(
            &PathBuf::from("res/materials/material.mat"),
        );

        let background_audio =
            assets_server.load::<AudioAssetsSystem>(&PathBuf::from("res/audios/pad.audio"));

        let blip_audio =
            assets_server.load::<AudioAssetsSystem>(&PathBuf::from("res/audios/blip.audio"));

        // ── Game camera (wayfinder map #158) ──────────────────────────
        //
        // The game camera is game content, so it is spawned once here — not by
        // a tab-opening handler like the editor camera. The world is therefore
        // complete before any tab opens. Its view projection is derived each
        // frame by the extract stage once `GameView.size` is known.
        // The pose frames the whole vertical extent the demo occupies — the
        // ground top at y = -0.9 through the ball top at y = 10.5 — inside the
        // 45-degree vertical fov at the ~17 m framing distance. The target sits
        // near the middle of that span, and the resulting downward tilt keeps
        // the 200x200 ground a receding plane rather than an edge-on line.
        let cam_pos = float3::new(0.0, 8.0, -18.0);
        let cam_target = float3::new(0.0, 0.0, 0.0);
        let game_camera_transform = LocalTransform::look_at(cam_pos, cam_target, float3::UP);
        // Intrinsics are unchanged from the pre-fork values.
        let game_camera = Camera::new(45.0, 0.3, 1000.0);
        let game_camera_entity = engine
            .world
            .spawn((game_camera_transform, game_camera))
            .id();
        engine.world.resource_mut::<GameView>().camera = Some(game_camera_entity);

        Self::spawn_audio_scene(
            &mut engine.world,
            &mut engine.audio_engine,
            background_audio,
            blip_audio,
        );

        // ── Physics bodies (wayfinder map #150) ───────────────────────
        //
        // Plane / ball poses plus their physics bodies. The transforms are
        // shared with the render region below, which returns the entity ids the
        // physics components are appended onto — `Collider` for the immovable
        // plane, `RigidBody` + `Collider` for the movable ball — never as
        // separate parallel entities.
        let plane_transform = LocalTransform::new(
            float3::new(0.0, -1.0, 0.0),
            quaternion::IDENTITY,
            float3::new(10.0, 10.0, 10.0),
        );

        let ball_transform = LocalTransform::new(
            float3::new(0.0, 10.0, 0.0),
            quaternion::IDENTITY,
            float3::ONE * 2.0,
        );

        // Colliders take world-space sizes, so the meshes' local bounds are
        // folded with the scale above: the plane mesh is 200 x 0.2 x 200 (half
        // (100, 0.1, 100)) at scale 10, and the ball mesh is a unit sphere
        // (radius 0.5) at scale 2. The physics side never reads
        // `LocalTransform.scale`, which has no rapier counterpart.
        let plane_half_extents = float3::new(1000.0, 1.0, 1000.0);
        let ball_radius = 1.0;

        let mut physics = engine.world.resource_mut::<PhysicsEngine>();
        let plane_collider = physics.insert_immovable_box(plane_half_extents, plane_transform);
        let (ball_rigid_body, ball_collider) = physics.insert_movable_sphere(
            ball_radius,
            ColliderMaterial { restitution: 0.8 },
            ball_transform,
        );

        SerializedMeshAsset::save_from_glb_file(PathBuf::from("res/models/Ball.glb"));

        let plan_mesh_asset =
            assets_server.load::<MeshAssetsSystem>(&PathBuf::from("res/models/Plane.mesh"));
        let ball_mesh_asset =
            assets_server.load::<MeshAssetsSystem>(&PathBuf::from("res/models/Ball.mesh"));
        let plane_mesh = LODMesh::new(plan_mesh_asset);
        let ball_mesh = LODMesh::new(ball_mesh_asset);

        // ── Render demo (wayfinder map #158) ──────────────────────────
        //
        // Mesh side only: `LocalTransform` + `LODMesh` + `MaterialComponent`.
        // The ids they return are the hosts for the physics components below,
        // which reuse the plane / ball transforms above.
        let plane_entity = engine
            .world
            .spawn((
                plane_transform,
                plane_mesh,
                MaterialComponent::new(material.clone()),
            ))
            .id();
        let ball_entity = engine
            .world
            .spawn((
                ball_transform,
                ball_mesh,
                MaterialComponent::new(material.clone()),
            ))
            .id();

        // ── Physics append (wayfinder map #150) ───────────────────────
        //
        // The components land on the same entities that carry the meshes, so
        // the world holds one plane entity and one ball entity — no parallel
        // physics-only copies.
        engine.world.entity_mut(plane_entity).insert(plane_collider);
        engine
            .world
            .entity_mut(ball_entity)
            .insert((ball_rigid_body, ball_collider));

        Self {
            listener_drift_angle: LISTENER_DRIFT_START_ANGLE,
        }
    }

    /// Spawns the minimal audible scene — the listener, the reverb zone, the
    /// volume ring and the background track — into `world`.
    ///
    /// Nothing here touches graphics, physics or input: every entity is built
    /// from `kairos_transform` types plus the audio components, so the scene
    /// stays runnable while those subsystems are still being migrated.
    ///
    /// The listener entity is not returned or kept: [`Self::drift_listener`]
    /// finds it through its component, so the scene needs no entity bookkeeping
    /// in [`KairosGame`] and disappears cleanly when the drift does.
    fn spawn_audio_scene(
        world: &mut World,
        audio_engine: &mut AudioEngine,
        background_audio: AudioAssetHandle,
        blip_audio: AudioAssetHandle,
    ) {
        // The kira listener itself is owned by the audio engine's spatial track
        // table; the entity only carries its id (kira's handles are not `Clone`,
        // so id-as-bookkeeping/handle-as-owner is the split the engine uses).
        if let Some(listener_id) = audio_engine.create_listener() {
            world.spawn((
                listener_drift_transform(LISTENER_DRIFT_START_ANGLE),
                SpatialAudioListenerComponent {
                    listener_id,
                    priority: 100,
                },
            ));
        }

        // The reverb zone: a room covering the +Z half of the listener's orbit.
        let (reverb, bound) = SpatialAudioReverb::new(
            REVERB_DISTANCE_RANGE,
            REVERB_MIN_VOLUME,
            REVERB_MAX_VOLUME,
            REVERB_FEEDBACK,
            REVERB_DAMPING,
            REVERB_MIX,
            AABB {
                min: REVERB_ZONE_MIN,
                max: REVERB_ZONE_MAX,
            },
        );
        world.spawn((reverb, bound));

        // The volume ring. Spawned one at a time rather than through
        // `spawn_batch`, whose entity iterator is lazy and would silently spawn
        // nothing if dropped; twelve entities do not need the batching.
        let angle_step = std::f32::consts::TAU / AUDIO_VOLUME_COUNT as f32;
        for index in 0..AUDIO_VOLUME_COUNT {
            let angle = index as f32 * angle_step;
            let radius = AUDIO_VOLUME_LANE_RADII[index % AUDIO_VOLUME_LANE_RADII.len()];
            let transform = LocalTransform::new(
                point_on_circle(LISTENER_DRIFT_CENTER, radius, angle),
                quaternion::IDENTITY,
                float3::ONE,
            );
            // `auto_play` starts each blip as soon as its asset resolves; the
            // random start offset keeps the twelve looping copies out of phase,
            // so the ring is heard as a texture whose individual voices fade in
            // and out rather than as one unison hit.
            let volume = SpatialAudioVolume::new(
                smallvec::smallvec![blip_audio.clone()],
                true,
                rand::random_range(0.0..AUDIO_VOLUME_START_OFFSET_WINDOW),
            );
            world.spawn((transform, volume));
        }

        // The background track plays on the main (non-spatial) track, so it is
        // audible from the first frame its asset resolves, wherever the listener
        // happens to be.
        world.spawn(BackgroundAudio::new(background_audio, true));
    }

    /// Advances the listener's purely-local circular drift by one frame.
    ///
    /// The listener orbits [`LISTENER_DRIFT_CENTER`] at a constant radius and
    /// angular speed, always facing the centre. No input, camera or physics is
    /// consulted — see the scene notes above `KairosGame` for why, and for how
    /// to delete this.
    fn drift_listener(&mut self, world: &mut World, delta_time: f32) {
        self.listener_drift_angle = (self.listener_drift_angle
            + LISTENER_DRIFT_ANGULAR_SPEED * delta_time)
            .rem_euclid(std::f32::consts::TAU);

        let transform = listener_drift_transform(self.listener_drift_angle);

        // A no-op while the scene holds no listener entity — the query is simply
        // empty, so a failed `create_listener` degrades to a silent spatial path
        // rather than a panic.
        let mut listeners = world.query::<(&mut LocalTransform, &SpatialAudioListenerComponent)>();
        for (mut local_transform, _) in listeners.iter_mut(&mut *world) {
            *local_transform = transform;
        }
    }

    pub fn update(&mut self, engine: &mut Engine) {
        // Time is advanced exactly once per frame by `time_system` at the
        // `First` stage (frame start); game-side code only reads it here, to
        // feed subsystem update parameters.
        let _total_time = engine.time().total_time().as_secs_f32();
        let delta_time = engine.time().delta_time().as_secs_f32();

        // Drift the listener first, so this frame's audio is spatialised against
        // this frame's listener position rather than the previous one's.
        self.drift_listener(&mut engine.world, delta_time);

        // ── Audio System ──────────────────────────────────────────────
        //
        // Driven by hand: #156 fixed the driver shape to a manual per-frame
        // call from here (`AudioEngine` stays an `Engine` field, `AssetsServer`
        // is still passed by reference, `dt` comes from the `Time` resource the
        // `First` stage advanced).
        engine
            .audio_engine
            .update(&mut engine.assets_server, &mut engine.world, delta_time);
    }
}
