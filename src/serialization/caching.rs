use bevy::prelude::Resource;
use bevy_asset_loader::prelude::AssetCollection;

#[derive(AssetCollection, Resource)]
pub(crate) struct StructureModels {
    //#[asset(path = "models", collection)]
    //pub(crate) models: Vec<UntypedHandle>
}