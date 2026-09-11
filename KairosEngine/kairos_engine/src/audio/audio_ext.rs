//! The `AudioExt` asset: the editor's runtime composite for an `.audio` file.
//!
//! An `.audio` descriptor names its source audio file. The audio inspector needs
//! both the runtime [`AudioAsset`] (decoded by kira, for playback) and the
//! [`PcmData`] (raw samples, for the waveform and spectrum views), so
//! [`AudioExt`] bundles the two handles. It loads through the next-generation
//! core and declares both through [`LoadContext::load`], so a live composite
//! keeps them loaded.

use kairos_asset::next::{
    Asset, AssetLoader, AssetWorldExt, Handle, LoadContext, Reader, UntypedAssetId,
    VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::audio::audio::{AudioAsset, SerializedAudioAsset};

pub mod pcm;
pub use pcm::PcmData;

/// Editor runtime composite for one `.audio`: handles to its runtime sound data
/// and its decoded PCM.
#[derive(Debug, Default)]
pub struct AudioExt {
    pub audio: Option<Handle<AudioAsset>>,
    pub pcm: Option<Handle<PcmData>>,
}

impl Asset for AudioExt {}
impl VisitAssetDependencies for AudioExt {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.audio.visit_dependencies(visit);
        self.pcm.visit_dependencies(visit);
    }
}

/// How many composite slots [`Assets<AudioExt>`](kairos_asset::next::Assets)
/// preallocates. Carried over from the legacy stack's
/// `AUDIO_EXT_ASSETS_CAPACITY`.
pub const AUDIO_EXT_ASSETS_CAPACITY: usize = 8;

/// Reads a `.audio` descriptor and declares its runtime [`AudioAsset`] and
/// [`PcmData`] dependencies.
#[derive(Debug)]
pub struct AudioExtLoader;

impl AssetLoader for AudioExtLoader {
    type Asset = AudioExt;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<AudioExt, KairosError>> {
        async move {
            let mut toml_bytes = Vec::new();
            reader.read_to_end(&mut toml_bytes).await?;
            let serialized: SerializedAudioAsset = toml::from_slice(&toml_bytes)?;

            // The runtime sound data is this same `.audio` path loaded as an
            // `AudioAsset`; the PCM is the source file the descriptor names.
            let audio_path = load_context.path().path().to_path_buf();
            let audio = load_context.load::<AudioAsset>(audio_path);
            let pcm = load_context.load::<PcmData>(serialized.source_path);

            Ok(AudioExt {
                audio: Some(audio),
                pcm: Some(pcm),
            })
        }
    }

    /// This loader is resolved by asset type, never by extension: the `.audio`
    /// extension is claimed by
    /// [`AudioAssetLoader`](crate::audio::audio::AudioAssetLoader), and both are
    /// always loaded with an explicit asset type.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// Registers the [`AudioExt`] asset and its [`AudioExtLoader`] with the core.
///
/// Must run after [`kairos_asset::next::install`] and after the `AudioAsset` and
/// `PcmData` stores its loader declares dependencies on.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<AudioExt>(AUDIO_EXT_ASSETS_CAPACITY);
    world.register_asset_loader(AudioExtLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use kairos_asset::next::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{AudioExt, PcmData, install as install_audio_ext};
    use crate::audio::audio::{
        AudioAsset, SerializedAudioAsset, SerializedAudioAssetSettings, install as install_audio,
    };
    use crate::audio::audio_ext::pcm::wav_bytes;

    /// The two ad-hoc stages the asset drivers are installed into.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Tracking;

    impl ScheduleLabel for Tracking {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Events;

    impl ScheduleLabel for Events {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    /// An `.audio` load through the core lands the composite *and* pulls both its
    /// `AudioAsset` and `PcmData` dependencies into their stores.
    #[test]
    fn audio_ext_loads_with_its_dependencies() {
        // The default source is rooted at the cwd, so every path — including the
        // descriptor's `source_path`, which is loaded as an asset — is relative.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let cwd = std::env::current_dir().expect("the cwd");

        let source_path = dir.path().join("Probe.wav");
        std::fs::write(&source_path, wav_bytes(&[0.0, 0.5, -0.5, 0.0], 44100))
            .expect("write the source audio");
        let rel_source = source_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let descriptor_path = dir.path().join("Probe.audio");
        let descriptor = SerializedAudioAsset {
            source_path: rel_source,
            audio_asset_settings: SerializedAudioAssetSettings::default(),
        };
        std::fs::write(
            &descriptor_path,
            toml::to_string(&descriptor).expect("serialize the audio descriptor"),
        )
        .expect("write the audio descriptor");
        let rel_descriptor = descriptor_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_audio(&mut world);
        crate::audio::audio_ext::pcm::install(&mut world);
        install_audio_ext(&mut world);

        let handle = world.resource::<AssetServer>().load::<AudioExt>(rel_descriptor);

        // The composite's dependencies resolve on their own tasks, so pump until
        // all three values are in their stores.
        let mut loaded = None;
        for _ in 0..400 {
            world.run_schedule(Tracking);
            if let Some(ext) = world.resource::<Assets<AudioExt>>().get(handle.id()) {
                let audio = ext
                    .audio
                    .as_ref()
                    .and_then(|audio| world.resource::<Assets<AudioAsset>>().get(audio.id()));
                let pcm = ext
                    .pcm
                    .as_ref()
                    .and_then(|pcm| world.resource::<Assets<PcmData>>().get(pcm.id()));
                if let (Some(audio), Some(pcm)) = (audio, pcm) {
                    loaded = Some((audio.sound_data.sample_rate, pcm.num_samples));
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(loaded, Some((44100, 4)));
    }
}
