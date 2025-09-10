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
    // Request a PathBlendMaterial to be created and applied directly.
    // The base material is looked up by name (same cache as other variants),
    // UVs will be scaled by tiling_factor on the mesh like TiledMaterial,
    // and optional params can override the default PathBlendParams.
    PathBlend {
        material_name: String,
        tiling_factor: Vec2,
        // Optional secondary albedo texture path (e.g., "materials/Path/your_albedo.png")
        near_albedo_path: Option<String>,
        // Optional secondary metallic-roughness texture path
        near_metallic_roughness_path: Option<String>,
        // Optional secondary ambient occlusion texture path
        near_ao_path: Option<String>,
    },
}
