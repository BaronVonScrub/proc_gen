use bevy::prelude::{Commands, Mesh, Vec2};
use bevy_math::primitives::Circle;
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
    AllPathsDebug,
};
use proc_gen::spawning::helpers::GenRng;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Component)]
pub(crate) struct GeneratedRoot;

fn send_generation_events(c: &mut Commands, parent: Option<Entity>) {
    spawn!(c, StructureSpawnEvent {
        structure: "atmospheric_setup".to_string(),
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
        material: TMaterial::TiledMaterial {
            material_name: "Grass".to_string(),
            tiling_factor: Vec2::new(5.0, 5.0),
        },
        parent,
    });
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
    mut paths_dbg: ResMut<AllPathsDebug>,
) {
    if !keys.just_pressed(KeyCode::Space) { return; }

    // Fresh, non-deterministic seed from system time
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let seed = now.as_nanos() as u64;
    *gen_rng = GenRng::new(seed);

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
    // Clear accumulated path debug lines
    paths_dbg.paths.clear();
    next_state.set(GenerationState::Generating);
}
