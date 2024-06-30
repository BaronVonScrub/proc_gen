use std::collections::HashMap;
use bevy::asset::Handle;
use bevy::prelude::Resource;
use bevy_asset_loader::prelude::AssetCollection;
use bevy_pbr::StandardMaterial;

#[derive(AssetCollection, Resource)]
pub(crate) struct StructureModels {
    //#[asset(path = "models", collection)]
    //pub(crate) models: Vec<UntypedHandle>
}

#[derive(Resource)]
pub(crate) struct MaterialCache {
    map: HashMap<String, Handle<StandardMaterial>>,
}

impl MaterialCache {
    pub fn new() -> Self {
        MaterialCache {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, handle: Handle<StandardMaterial>) {
        self.map.insert(name, handle);
    }

    pub fn get(&self, name: &str) -> Option<&Handle<StandardMaterial>> {
        self.map.get(name)
    }
}