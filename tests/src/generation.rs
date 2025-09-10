use bevy::prelude::{Commands, Mesh, Vec2};
use bevy_math::primitives::{Circle, Rectangle};
use proc_gen::core::tmaterial::TMaterial;
use proc_gen::spawning::euler_transform::EulerTransform;
use proc_gen::spawn;
use proc_gen::event_system::spawn_events::*;
use bevy::prelude::*;
use proc_gen::event_system::event_listeners::{
    GenerationState,
    CollisionResolutionTimer,
    CurrentPass,
    HighestPassIndex,
    PendingInPass,
};
use proc_gen::spawning::helpers::GenRng;
use std::time::{SystemTime, UNIX_EPOCH};
use proc_gen::management::structure_management::clear_structure_cache;
#[cfg(feature = "debug")]
use proc_gen::event_system::event_listeners::AllPathsDebug;
use bevy_pbr::StandardMaterial;
use proc_gen::materials::path_blend::{PathBlendMaterial, PathBlendParams, falloff_mode, GroundPathMaterial, make_path_blend_material};

#[derive(Component)]
pub(crate) struct GeneratedRoot;

fn send_generation_events(c: &mut Commands, parent: Option<Entity>) {

    // CFG guard: Feature "castle"
    #[cfg(feature = "castle")]
    {
        // Always spawn baseline atmosphere/lighting setup
        spawn!(c, StructureSpawnEvent {
            structure: "atmospheric_setup_castle".to_string(),
            transform: Default::default(),
            parent,
        });

        spawn!(c, StructureSpawnEvent {
            structure: "Castle/castle_with_trees".to_string(),
            transform: Default::default(),
            parent,
        });

        spawn!(c, MeshSpawnEvent {
            mesh: Mesh::from(Circle::new(11.0))
                .with_generated_tangents()
                .unwrap(),
            transform: EulerTransform {
                translation: (0.0, 0.0, 0.0),
                // Circle lies in XY plane by default; rotate -90deg around X to place it on XZ ground
                rotation: (-90.0, 0.0, 0.0),
                scale: (1.0, 1.0, 1.0),
            },
            material: TMaterial::PathBlend {
                material_name: "Grass".to_string(),
                tiling_factor: Vec2::new(6.0, 6.0),
                // Blend toward the Soil albedo near the path
                near_albedo_path: Some("materials/Soil/TCom_Ground_Soil3_2x2_1K_albedo.png".to_string()),
                near_metallic_roughness_path: Some("materials/Soil/TCom_Ground_Soil3_2x2_1K_metallicRoughness.png".to_string()),
                near_ao_path: Some("materials/Soil/TCom_Ground_Soil3_2x2_1K_ao.png".to_string()),
            },
            parent,
        });
    }

    // CFG guard: Feature "tree" (spawn demo branch-with-leaves when castle is not enabled)
    #[cfg(all(feature = "tree", not(feature = "castle")))]
    {
        // Basic atmosphere for tree-only scene (reuse castle setup for now)
        spawn!(c, StructureSpawnEvent {
            structure: "atmospheric_setup_tree".to_string(),
            transform: Default::default(),
            parent,
        });

        // Demo branch with leaves
        spawn!(c, StructureSpawnEvent {
            structure: "Trees/massive_branch".to_string(),
            transform: Default::default(),
            parent,
        });

        spawn!(c, MeshSpawnEvent {
            mesh: Mesh::from(Circle::new(11.0))
                .with_generated_tangents()
                .unwrap(),
            transform: EulerTransform {
                translation: (0.0, 0.0, 0.0),
                // Circle lies in XY plane by default; rotate -90deg around X to place it on XZ ground
                rotation: (-90.0, 0.0, 0.0),
                scale: (0.6, 0.6, 0.6),
            },
            material: TMaterial::TiledMaterial {
                material_name: "Grass".to_string(),
                tiling_factor: Vec2::new(2.0, 2.0),
            },
            parent,
        });
    }

    // CFG guard: Feature "map"
    #[cfg(feature = "map")]
    {
        // Minimal atmosphere/lighting for map scenario
        spawn!(c, StructureSpawnEvent {
            structure: "atmospheric_setup_map".to_string(),
            transform: Default::default(),
            parent,
        });

        // Square ground plane (Rectangle in XY, rotate -90deg around X to lie on XZ)
        spawn!(c, MeshSpawnEvent {
            mesh: Mesh::from(Rectangle::new(24.0, 24.0))
                .with_generated_tangents()
                .unwrap(),
            transform: EulerTransform {
                translation: (0.0, 0.0, 0.0),
                rotation: (-90.0, 0.0, 0.0),
                scale: (1.0, 1.0, 1.0),
            },
            material: TMaterial::PathBlend {
                material_name: "Grass".to_string(),
                tiling_factor: Vec2::new(6.0, 6.0),
                // Blend toward the Soil albedo near the path
                near_albedo_path: Some("materials/Soil/TCom_Ground_Soil3_2x2_1K_albedo.png".to_string()),
                near_metallic_roughness_path: Some("materials/Soil/TCom_Ground_Soil3_2x2_1K_metallicRoughness.png".to_string()),
                near_ao_path: Some("materials/Soil/TCom_Ground_Soil3_2x2_1K_ao.png".to_string()),
            },
            parent,
        });

        // Spawn the path demo structure that places a PathEnd tag and computes a path to it from the opposite corner
        spawn!(c, StructureSpawnEvent {
            structure: "map_paths".to_string(),
            transform: Default::default(),
            parent,
        });
    }
}

pub(crate) fn generate_map(mut c: Commands) {
    // Create a root entity for all procedurally generated content so we can clear it on reset.
    let root = c
        .spawn_empty()
        .insert(Name::new("GeneratedRoot"))
        .insert(Transform::default())
        .insert(Visibility::default())
        .insert(GeneratedRoot)
        .id();

    send_generation_events(&mut c, Some(root));
}

pub(crate) fn reset_on_space(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut gen_rng: ResMut<GenRng>,
    roots: Query<Entity, With<GeneratedRoot>>,
    mut next_state: ResMut<NextState<GenerationState>>,
    mut collision_timer: ResMut<CollisionResolutionTimer>,
    // Generation pass resetting
    mut cur_pass: ResMut<CurrentPass>,
    mut highest_pass: ResMut<HighestPassIndex>,
    mut pending_inpass: ResMut<PendingInPass>,
    #[cfg(feature = "debug")] mut all_paths_debug: Option<ResMut<AllPathsDebug>>,
) {
    if !keys.just_pressed(KeyCode::Space) { return; }

    // Immediately clear debug overlay paths so the next frame draws nothing
    #[cfg(feature = "debug")]
    if let Some(mut dbg) = all_paths_debug.as_mut() { dbg.paths.clear(); }

    // Fresh, non-deterministic seed from system time
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let seed = now.as_nanos() as u64;
    *gen_rng = GenRng::new(seed);

    // Clear cached .arch structures so subsequent imports re-read from disk
    clear_structure_cache();

    // Despawn previous generation root(s)
    for root in roots.iter() {
        commands.entity(root).despawn_recursive();
    }

    // Spawn a new root and re-dispatch generation events
    let new_root = commands
        .spawn_empty()
        .insert(Name::new("GeneratedRoot"))
        .insert(Transform::default())
        .insert(Visibility::default())
        .insert(GeneratedRoot)
        .id();

    send_generation_events(&mut commands, Some(new_root));

    // Reset generation pipeline state so it runs again
    collision_timer.frames = 0;
    // Reset pass state and pending deferrals; HighestPassIndex will be raised by authored InPass keys
    cur_pass.0 = 0;
    highest_pass.0 = 0;
    pending_inpass.0.clear();
    next_state.set(GenerationState::Generating);
}
