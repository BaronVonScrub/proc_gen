use bevy::ecs::reflect::ReflectComponent;
use bevy::app::{App, Plugin};
use bevy::prelude::{Component, Reflect};
use bevy_inspector_egui::InspectorOptions;
use serde::{Deserialize, Serialize};
use bevy_inspector_egui::prelude::ReflectInspectorOptions;

pub(crate) struct ObjectLogicPlugin;
impl Plugin for ObjectLogicPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Ownership>()
            .register_type::<ObjectType>();
    }
}


#[derive(InspectorOptions, Serialize, Deserialize, Component, Debug, Clone, Reflect)]
#[reflect(Component, InspectorOptions)]
pub enum Ownership {
    Team(u8),
    Inherit,
}

#[derive(InspectorOptions, Serialize, Deserialize, Debug, Clone, Component, Reflect)]
#[reflect(Component, InspectorOptions)]
pub enum ObjectType {
    Building,
    Unit,
    Cosmetic,
    Resource,
    Terrain,
    Other,
}
