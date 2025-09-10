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
        app.init_resource::<NavMeshPriorityThreshold>();
        app.init_resource::<PathResolveTimer>();
        #[cfg(feature = "debug")]
        {
            app.init_resource::<AllPathsDebug>();
        }
        app.init_resource::<CurrentPass>();
        app.init_resource::<HighestPassIndex>();
        app.init_resource::<PendingInPass>();
        app.init_resource::<PendingPathEvents>();
        app.init_resource::<ResolvedPathSpawns>();
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
            .add_event::<AtmosphereNishitaSpawnEvent>()
            .add_event::<MainDirectionalLightSpawnEvent>()
            .add_event::<SoundEffectSpawnEvent>()
            .add_event::<BackgroundMusicSpawnEvent>()
            .add_event::<NestSpawnEvent>()
            .add_event::<ChooseSpawnEvent>()
            .add_event::<ChooseSomeSpawnEvent>()
            .add_event::<RandSpawnEvent>()
            .add_event::<RandDistDirSpawnEvent>()
            .add_event::<ProbabilitySpawnEvent>()
            .add_event::<LoopParamSpawnEvent>()
            .add_event::<LoopSpawnEvent>()
            .add_event::<NestingLoopSpawnEvent>()
            .add_event::<NoiseSpawnEvent>()
            .add_event::<PathSpawnEvent>()
            .add_event::<PathToTagSpawnEvent>()
            .add_event::<PathToAllTagsSpawnEvent>()
            .add_event::<PathWorldPointsEvent>()
            .add_event::<ReflectionSpawnEvent>()
            .add_event::<SelectiveReplacementSpawnEvent>();
        app.add_event::<InPassSpawnEvent>();

        // Registering non-path event handling systems (only needed during Generating)
        app.add_systems(Update, (
            mesh_spawn_listener,
            scene_spawn_listener,
            structure_spawn_listener,
            point_light_spawn_listener,
            spot_light_spawn_listener,
            directional_light_spawn_listener,
            main_directional_light_spawn_listener,
            ambient_light_spawn_listener,
            distance_fog_spawn_listener,
            sound_effect_spawn_listener,
            background_music_spawn_listener,
            // Materialize path-driven spawns during Generating (not PathResolve)
            path_spawn_listener,
            nest_spawn_listener,
        ).run_if(in_state(GenerationState::Generating)));

        // Atmosphere events are only processed when the 'atmosphere' feature is enabled
        #[cfg(feature = "atmosphere")]
        {
            app.add_systems(Update, atmosphere_nishita_spawn_listener.run_if(in_state(GenerationState::Generating)));
        }

        // UI overlay update (always on)
        app.add_systems(Update, update_generation_state_overlay);
        // Draw accumulated path debug gizmos when enabled
        #[cfg(feature = "debug")]
        {
            app.add_systems(Update, draw_all_paths_debug);
        }

        // Generation-time systems only (excluding pathfinding, which is buffered)
        app.add_systems(Update, (
            // Tick spawn activity every frame; individual listeners will reset this when they process events
            tick_spawn_activity,
            buffer_path_events,
            choose_spawn_listener,
            choose_some_spawn_listener,
            in_pass_spawn_listener,
            process_pending_inpass,
            rand_spawn_listener,
            rand_dist_dir_spawn_listener,
            probability_spawn_listener,
            loop_param_spawn_listener,
            loop_spawn_listener,
            nesting_loop_spawn_listener,
            noise_spawn_listener,
            reflection_spawn_listener,
            selective_replacement_spawn_listener,
            collider_priority_despawn_system,
            // Tick generating frame counter while in Generating state
            tick_generating_counter,
        ).run_if(in_state(GenerationState::Generating)));

        // Path resolution window: run all pathfinding and material application here
        app.add_systems(OnEnter(GenerationState::PathResolve), (reset_path_resolve_phase, flush_path_events_on_enter));
        app.add_systems(Update, (
            path_to_tag_spawn_listener,
            path_to_all_tags_spawn_listener,
            apply_stored_polylines_to_tagged_pathblend,
            apply_world_path_points_to_material,
        ).run_if(in_state(GenerationState::PathResolve)));
        app.add_systems(Update, advance_from_path_resolve);

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
        // On entering Generating, reset counters and flush any resolved PathSpawnEvent from PathResolve
        app.add_systems(OnEnter(GenerationState::Generating), (reset_generating_phase, flush_resolved_paths_on_enter_generating));
        // On entering Completed, advance pass if more passes exist
        app.add_systems(OnEnter(GenerationState::Completed), advance_pass_or_finish);
        app.add_systems(Startup, spawn_generation_state_overlay);

    }
}
