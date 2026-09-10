use anyhow::Error;

use crate::material::SerializedMaterial;

impl SerializedMaterial {
    /// Write the material back to its `.mat` TOML file.
    pub fn save_to_file(&self) -> Result<(), Error> {
        let toml_content = toml::to_string(self)?;
        std::fs::write(&self.source_path, toml_content)?;
        Ok(())
    }
}
