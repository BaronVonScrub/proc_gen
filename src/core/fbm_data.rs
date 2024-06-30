use crate::proc_gen::core::seeded_or_not::SeededOrNot;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FBMData {
    pub seed: SeededOrNot,
    pub scale: f32,
    pub octaves: u8,
    pub frequency: f32,
    pub lacunarity: f32,
    pub persistence: f32,
}