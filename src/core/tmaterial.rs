use serde::{Serialize, Deserialize};
use bevy::prelude::Vec2;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TMaterial {
    BasicMaterial {
        material_name: String,
    },
    TiledMaterial {
        material_name: String,
        tiling_factor: Vec2,
    },
}
