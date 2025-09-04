use bevy::prelude::{Commands, Mesh, Vec2};
use bevy_math::prelude::Plane3d;
use bevy_utils::default;
use proc_gen::core::tmaterial::TMaterial;
use proc_gen::spawning::euler_transform::EulerTransform;
use proc_gen::spawn;
use proc_gen::event_system::spawn_events::*;
use bevy::prelude::*;
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
        structure: "Castle/damaged_castle".to_string(),
        transform: Default::default(),
        parent,
    });

    spawn!(c, StructureSpawnEvent {
        structure: "test_unit".to_string(),
        transform: EulerTransform {
            translation: (10.0, 0.0, -10.0),
            rotation: Default::default(),
            scale: (1.0, 1.0, 1.0),
        },
        parent,
    });

    spawn!(c, MeshSpawnEvent {
        mesh: Mesh::from(Plane3d { ..default() })
            .with_generated_tangents()
            .unwrap(),
        transform: EulerTransform {
            translation: (0.0, 0.0, 0.0),
            rotation: (0.0, 0.0, 0.0),
            scale: (20.0, 20.0, 20.0),
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
}
