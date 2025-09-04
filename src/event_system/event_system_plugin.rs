use bevy::prelude::*;
use crate::event_system::spawn_events::*;
use crate::event_system::event_listeners::*;
use crate::event_system::spawnables::structure::structure_spawn_listener;

pub struct EventSystemPlugin;

impl Plugin for EventSystemPlugin {
    fn build(&self, app: &mut App) {
        // Registering all events
        app.add_event::<MeshSpawnEvent>()
            .add_event::<SceneSpawnEvent>()
            .add_event::<StructureSpawnEvent>()
            .add_event::<PointLightSpawnEvent>()
            .add_event::<SpotLightSpawnEvent>()
            .add_event::<DirectionalLightSpawnEvent>()
            .add_event::<AmbientLightSpawnEvent>()
            .add_event::<DistanceFogSpawnEvent>()
            .add_event::<SoundEffectSpawnEvent>()
            .add_event::<BackgroundMusicSpawnEvent>()
            .add_event::<NestSpawnEvent>()
            .add_event::<ChooseSpawnEvent>()
            .add_event::<ChooseSomeSpawnEvent>()
            .add_event::<RandSpawnEvent>()
            .add_event::<ProbabilitySpawnEvent>()
            .add_event::<LoopSpawnEvent>()
            .add_event::<NestingLoopSpawnEvent>()
            .add_event::<NoiseSpawnEvent>()
            .add_event::<PathSpawnEvent>()
            .add_event::<ReflectionSpawnEvent>()
            .add_event::<SelectiveReplacementSpawnEvent>();

        // Registering event handling systems
        app.add_systems(Update, (
            mesh_spawn_listener,
            scene_spawn_listener,
            structure_spawn_listener,
            point_light_spawn_listener,
            spot_light_spawn_listener,
            directional_light_spawn_listener,
            ambient_light_spawn_listener,
            distance_fog_spawn_listener,
            sound_effect_spawn_listener,
            background_music_spawn_listener,
            nest_spawn_listener
        ));

        app.add_systems(Update, (
            choose_spawn_listener,
            choose_some_spawn_listener,
            rand_spawn_listener,
            probability_spawn_listener,
            loop_spawn_listener,
            nesting_loop_spawn_listener,
            noise_spawn_listener,
            path_spawn_listener,
            reflection_spawn_listener,
            selective_replacement_spawn_listener,
        ));

        // Deferred processor to run after children spawned in Update have been realized
        app.add_systems(PostUpdate, (
            selective_replacement_progressor,
        ));

    }
}
