use bevy::app::{App, Plugin};

use crate::spawning::helpers::GenRng;
use crate::core::tags::Tags;
use crate::serialization::caching::MaterialCache;
use crate::core::components::{PathPolyline, PathPolylineList};
use crate::management::material_autoloader::MaterialAutoloader;

pub struct GeneratorPlugin;

impl Plugin for GeneratorPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(GenRng::new(132))
            .insert_resource(MaterialCache::new())
            .add_plugins(MaterialAutoloader)
            .add_plugins(crate::materials::path_blend::PathBlendPlugin)
            .add_plugins(crate::event_system::event_system_plugin::EventSystemPlugin)
            .register_type::<Tags>()
            .register_type::<PathPolyline>()
            .register_type::<PathPolylineList>();
    }
}