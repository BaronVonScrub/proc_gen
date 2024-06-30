use crate::core::fbm_data::FBMData;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SpreadData {
    Regular,
    Gaussian(f32),
    Noise {
        fbm_data: FBMData,
        sample_size: f32,
        exclusivity_radius: f32,
        resolution_modifier: f32,
    },
}
