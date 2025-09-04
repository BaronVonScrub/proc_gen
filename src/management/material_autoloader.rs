use bevy::{
    prelude::*,
    reflect::Reflect,
    pbr::StandardMaterial,
    utils::hashbrown::HashMap,
};
use bevy::asset::processor::LoadTransformAndSave;
use bevy::asset::transformer::IdentityAssetTransformer;
use bevy::image::ImageLoader;
use bevy::image::CompressedImageSaver;
use bevy_asset_loader::prelude::*;
use crate::serialization::caching::MaterialCache;

pub struct MaterialAutoloader;

#[derive(AssetCollection, Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct MaterialTextures {
    #[asset(path = "materials", collection(typed, mapped))]
    pub textures: HashMap<String, Handle<Image>>,
}

// 🚀 Step 1: Define Game States
#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
}

impl Plugin for MaterialAutoloader {
    fn build(&self, app: &mut App) {

        app.register_asset_processor::<LoadTransformAndSave<ImageLoader, IdentityAssetTransformer<_>, CompressedImageSaver>>
            (LoadTransformAndSave::from(CompressedImageSaver));

        app.set_default_asset_processor::<LoadTransformAndSave<ImageLoader, IdentityAssetTransformer<_>, CompressedImageSaver>>(
            ".png",
        );

        app.init_state::<GameState>() // ✅ Initialize game state
            .add_loading_state(
                LoadingState::new(GameState::Loading) // ✅ Load assets in this state
                    .continue_to_state(GameState::Playing) // ✅ Move to Playing when done
                    .load_collection::<MaterialTextures>(), // ✅ Load textures!
            )
            .add_systems(OnEnter(GameState::Playing), preload_materials_system) // ✅ Convert textures into materials
            //.add_systems(OnEnter(GameState::Playing), check_l oaded_assets)
            ; // ✅ Debug check
    }
}

// 🚀 Step 2: Convert Loaded Textures into Materials
fn preload_materials_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    material_textures: Res<MaterialTextures>,
) {
    let mut material_cache = MaterialCache::new();
    let mut material_sets: HashMap<String, (Option<String>, Option<String>, Option<String>, Option<String>)> = HashMap::new();

    for file_path in material_textures.textures.keys() {
        debug!("Loading material: {}", file_path);

        let (mat_name, tex_type) = extract_tex_data(file_path);
        if tex_type == "unknown" {
            continue;
        }

        let entry = material_sets.entry(mat_name.clone()).or_insert((None, None, None, None));

        match tex_type.as_str() {
            "albedo" => entry.0 = Some(file_path.to_string()),
            "ao" => entry.1 = Some(file_path.to_string()),
            "normal" => entry.2 = Some(file_path.to_string()),
            "met_roughness" | "metallicRoughness" => entry.3 = Some(file_path.to_string()),
            _ => {}
        }
    }

    for (material_name, textures) in material_sets.iter() {
        let base_tex = textures.0.as_ref().map(|path| asset_server.load(path));
        let ao_tex = textures.1.as_ref().map(|path| asset_server.load(path));
        let normal_tex = textures.2.as_ref().map(|path| asset_server.load(path));
        let met_rough_tex = textures.3.as_ref().map(|path| asset_server.load(path));

        let material_handle = materials.add(StandardMaterial {
            base_color_texture: base_tex,
            occlusion_texture: ao_tex,
            normal_map_texture: normal_tex,
            metallic_roughness_texture: met_rough_tex,
            metallic: 0.1,
            perceptual_roughness: 0.9,
            // Many texture libraries author normal maps for DirectX (-Y). Flip to match Bevy's expected +Y.
            flip_normal_map_y: true,
            ..Default::default()
        });

        material_cache.insert(material_name.clone(), material_handle);
    }

    commands.insert_resource(material_cache);
}

// 🚀 Utility: Extract Material Data
fn extract_tex_data(tex_name: &str) -> (String, String) {
    // Support multiple common conventions for texture suffixes
    let texture_types = ["albedo", "ao", "normal", "met_roughness", "metallicRoughness"];
    let parts: Vec<&str> = tex_name.split('/').collect();
    let materials_index = parts.iter().position(|&r| r == "materials").unwrap_or(0);
    let material_name = parts.get(materials_index + 1).unwrap_or(&"").to_string();

    let texture_type = texture_types.iter()
        .find_map(|&t| {
            if tex_name.contains(&format!("_{}", t)) {
                Some(t.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    (material_name, texture_type)
}