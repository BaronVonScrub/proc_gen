use bevy::prelude::*;
use crate::event_system::spawn_events::*;
use crate::event_system::event_listeners::*;
use crate::event_system::spawnables::structure::structure_spawn_listener;
use crate::event_system::event_listeners::collider_priority_despawn_system;

pub struct EventSystemPlugin;

impl Plugin for EventSystemPlugin {
    fn build(&self, app: &mut App) {
        // Initialize generation states/resources
        app.init_resource::<CollisionResolutionTimer>();
        app.init_resource::<SpawningStability>();
        app.init_resource::<GeneratingFrameCounter>();
        app.init_resource::<SpawnActivity>();
        app.init_resource::<GenerationAdvanceArming>();
        app.init_state::<GenerationState>();
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

        // Registering event handling systems (only needed during Generating)
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
            nest_spawn_listener,
        ).run_if(in_state(GenerationState::Generating)));

        // UI overlay update (always on)
        app.add_systems(Update, update_generation_state_overlay);

        // Generation-time systems only
        app.add_systems(Update, (
            // Tick spawn activity every frame; individual listeners will reset this when they process events
            tick_spawn_activity,
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
            collider_priority_despawn_system,
            // Tick generating frame counter while in Generating state
            tick_generating_counter,
        ).run_if(in_state(GenerationState::Generating)));

        // Single state driver for all GenerationState transitions (always scheduled)
        app.add_systems(Update, generation_state_driver);

        // Deferred processors to run after children spawned in Update have been realized (only during Generating)
        app.add_systems(PostUpdate, (
            selective_replacement_progressor,
            enqueue_generation_only_colliders,
            strip_generation_only_colliders_progressor,
        ).run_if(in_state(GenerationState::Generating)));

        // On entering navmesh build phase, activate queued affectors
        app.add_systems(OnEnter(GenerationState::NavMeshBuilding), activate_navmesh_affectors);
        // On entering Generating, reset counters
        app.add_systems(OnEnter(GenerationState::Generating), reset_generating_phase);
        app.add_systems(Startup, spawn_generation_state_overlay);

    }
}
