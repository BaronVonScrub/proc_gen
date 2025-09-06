use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WobbleParams {
    pub amplitude: f32,           // lateral offset magnitude in meters
    pub wavelength: f32,          // world-space distance between crests
    pub phase: f32,               // phase offset in radians
    pub checkpoint_spacing: f32,  // distance between successive wobble checkpoints along the base path
}
