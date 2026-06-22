use std::path::{PathBuf};

use anyhow::Error;

use crate::audio::audio::{SerializedAudioAsset, SerializedAudioAssetSettings};

impl SerializedAudioAsset {
    /// Convert a raw audio source file into an AudioAsset.
    /// This reads the audio file, decodes it into sound data,
    /// and creates an AudioAsset with the source path as metadata.
    pub fn convert_audio_to_asset(path: &PathBuf) -> Result<SerializedAudioAsset, Error> {
        let bytes = std::fs::read(path)?;
        let sound_data =
            kira::sound::static_sound::StaticSoundData::from_cursor(std::io::Cursor::new(bytes))?;
        let source_path = path.to_path_buf();
        let asset = SerializedAudioAsset {
            source_path,
            audio_asset_settings: SerializedAudioAssetSettings::from_static_sound_data(&sound_data),
        };
        Ok(asset)
    }

    /// Save the AudioAsset to a `.audio` TOML file.
    /// The `sound_data` is skipped during serialization (it will be
    /// reconstructed from `meta.source_path` when loaded).
    /// Only the metadata and settings are persisted to the TOML file.
    pub fn save_to_file(&self) -> Result<(), Error> {
        let toml_content = toml::to_string(self)?;
        let path = self.source_path.with_extension("audio");
        std::fs::write(&path, toml_content)?;
        Ok(())
    }
}
