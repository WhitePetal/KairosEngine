use serde::{Deserialize, Serialize};

use super::Color32;

impl Serialize for Color32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex)
            .map_err(|e| D::Error::custom(format!("Deserialize Color32 Failed: {}", e)))
    }
}
