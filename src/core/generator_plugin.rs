use bevy::prelude::*;
use bevy::app::{App, Plugin, Update};

use crate::systems::events::{
    ambient_light_updater_system, background_music_updater_system, directional_light_updater_system,
    fog_updater_system, object_spawn_reader_system, sfx_event_listener_system, selective_replacement_reader_system,
};
use crate::spawning::helpers::GenRng;
use crate::core::tags::Tags;
use crate::serialization::caching::MaterialCache;
use crate::management::material_autoloader::MaterialAutoloader;

pub struct GeneratorPlugin;

impl Plugin for GeneratorPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Update,
                (
                    object_spawn_reader_system,
                    ambient_light_updater_system,
                    directional_light_updater_system,
                    fog_updater_system,
                    background_music_updater_system,
                    sfx_event_listener_system,
                    selective_replacement_reader_system,
                )
                    .chain()
            )
            .insert_resource(GenRng::new(132))
            .insert_resource(MaterialCache::new())
            .add_plugins(MaterialAutoloader)
            .register_type::<Tags>();
    }
}