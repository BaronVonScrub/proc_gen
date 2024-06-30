use bevy::ecs::reflect::ReflectComponent;
use bevy::app::{App, Plugin};
use bevy::math::Vec3;
use bevy::prelude::{Component, Reflect};
use bevy::tasks::Task;
use bevy_inspector_egui::InspectorOptions;
use serde::{Deserialize, Serialize};
use bevy_inspector_egui::prelude::ReflectInspectorOptions;
use oxidized_navigation::query::FindPathError;

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

#[derive(Component, Serialize, Deserialize, Reflect)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Selectable {
    pub(crate) is_selected: bool,
}

#[derive(Component)]
pub struct Pathfinder {
    pub(crate) path: PathState,
}

pub enum PathState {
    Ready(Vec3),
    Calculating(Task<(Result<Option<Vec<Vec3>>, FindPathError>, Vec3)>),
    Pathing(Vec<Vec3>),
}

impl Default for Pathfinder {
    fn default() -> Self {
        Pathfinder {
            path: PathState::Ready(Vec3::ZERO),
        }
    }
}