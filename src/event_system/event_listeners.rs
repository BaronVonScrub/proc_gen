#[allow(unused_variables)]

use bevy::prelude::*;
use bevy::render::mesh::MeshAabb;
use bevy_rapier3d::prelude::{ActiveEvents, Collider, RigidBody};
use oxidized_navigation::NavMeshAffector;
use crate::event_system::spawn_events::*;
use uuid::Uuid;
use crate::core::tmaterial::TMaterial;
use crate::serialization::caching::MaterialCache;
use std::path::Path;
use bevy::prelude::*;
use bevy_rapier3d::prelude::{ContactForceEventThreshold, Damping, Dominance, LockedAxes, Sleeping};
use crate::spawning::object_logic::{ObjectType, Pathfinder, PathState, Selectable};
use crate::core::structure_key::StructureKey;
use crate::core::collider::ColliderBehaviour;
use crate::spawning::helpers::*;
use crate::spawning::light_spawning::{spawn_point_light, spawn_spot_light};
use crate::core::components::MainCamera;
use crate::core::structure::Structure;
use crate::core::structure_reference::StructureReference;
use crate::core::components::MainDirectionalLight;
use crate::event_system::spawnables::structure::spawn_structure_data;
use crate::core::tags::Tags;
use rand::prelude::IteratorRandom;
use crate::spawning::helpers::GenRng;
use bevy::ecs::world::World;
use crate::spawning::euler_transform::EulerTransform;
use crate::spawning::transformation::{get_looped_position_list, generate_noise_spawn_points};
use bevy_math::cubic_splines::CubicCardinalSpline;
use rand::Rng;
use crate::core::spread_data::SpreadData;
use bevy_kira_audio::{Audio, AudioChannel, AudioControl};
use bevy_kira_audio::AudioSource;
use crate::management::audio_management::SoundEffects;

// Tracks a selective replacement that should be deferred until the subtree has finished spawning.
#[derive(Component)]
pub struct SelectiveReplacementPending {
    pub replacement_reference: StructureReference,
    pub tags: Vec<String>,
    pub replace_count: usize,
    pub last_descendant_count: usize,
    pub last_candidate_count: usize,
    pub stable_frames: u8,
}

pub fn mesh_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<MeshSpawnEvent>,
    material_cache: Res<MaterialCache>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for event in reader.read() {
        let (material_name, adjusted_mesh) = match &event.material {
            TMaterial::BasicMaterial { material_name } => {
                (material_name.clone(), event.mesh.clone()) // No tiling factor adjustment needed
            }
            TMaterial::TiledMaterial { material_name, tiling_factor } => {
                let mut mesh = event.mesh.clone();
                if let Some(bevy::render::mesh::VertexAttributeValues::Float32x2(uvs)) =
                    mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
                {
                    for uv in uvs.iter_mut() {
                        uv[0] *= tiling_factor.x;
                        uv[1] *= tiling_factor.y;
                    }
                }
                (material_name.clone(), mesh)
            }
        };

        let bounding_box = adjusted_mesh.compute_aabb().unwrap();
        let half_extents = bounding_box.half_extents;
        let collider_size = Vec3::new(half_extents.x, half_extents.y, half_extents.z);

        if let Some(material_handle) = material_cache.get(&material_name) {
            let mesh_handle = meshes.add(adjusted_mesh);

            // First, spawn the entity and get its ID
            let entity_id = commands.spawn_empty().id();

            // Then, use commands.entity() to insert components
            commands.entity(entity_id)
                .insert(Mesh3d(mesh_handle))
                .insert(MeshMaterial3d((*material_handle).clone()))
                .insert(Transform::from(event.transform.clone()))
                .insert(Name::new("Mesh"))
                .insert(Collider::cuboid(collider_size.x, collider_size.y, collider_size.z))
                .insert(RigidBody::KinematicPositionBased)
                .insert(ActiveEvents::CONTACT_FORCE_EVENTS)
                .insert(NavMeshAffector)
                .insert(InheritedVisibility::default());

            // Set parent if applicable
            if let Some(parent) = event.parent {
                commands.entity(entity_id).set_parent(parent);
            }
        } else {
            println!("Material not found: {}", material_name);
        }
    }
}

pub fn scene_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<SceneSpawnEvent>,
    asset_server: Res<AssetServer>,
) {
    for event in reader.read() {
        let global_transform = Transform::from(event.transform.clone());

        let parent_entity = commands.spawn_empty()
            .insert(global_transform)
            .insert(InheritedVisibility::default())
            .id();

        if let StructureKey::Object { path, collider, offset, ownership, selectable, object_type } = &event.data {
            let scene_handle: Handle<Scene> = asset_server.load(path);

            commands.entity(parent_entity).with_children(|parent| {
                parent.spawn_empty()
                    .insert(InheritedVisibility::default())
                    .insert(SceneRoot(scene_handle))
                    .insert(Transform::from_translation(*offset));
            });

            let filename = Path::new(path)
                .file_name()
                .and_then(|file_name| file_name.to_str()).unwrap_or("Unnamed Scene");

            commands.entity(parent_entity)
                .insert(Name::new(filename.to_string()))
                .insert(ownership.clone())
                .insert(object_type.clone());

            if *selectable {
                commands.entity(parent_entity).insert(Selectable { is_selected: false });
            }

            if let Some(internal_collider) = collider.clone() {
                if let Some(collider) = create_collider(&internal_collider.collider_type) {
                    let mut entity_commands = commands.entity(parent_entity);
                    entity_commands.insert(collider)
                        .insert(Dominance::group(internal_collider.priority))
                        .insert(Damping { linear_damping: 10.0, angular_damping: 0.0 })
                        .insert(LockedAxes::ROTATION_LOCKED | LockedAxes::TRANSLATION_LOCKED_Y)
                        .insert(ActiveEvents::CONTACT_FORCE_EVENTS)
                        .insert(Sleeping {
                            normalized_linear_threshold: 0.01,
                            angular_threshold: 0.01,
                            sleeping: false,
                        })
                        .insert(ContactForceEventThreshold(0.0));

                    match object_type {
                        ObjectType::Unit => {
                            let start_goal = global_transform.translation;
                            entity_commands.insert(Pathfinder {
                                path: PathState::Ready(start_goal),
                            });
                        }
                        ObjectType::Cosmetic => { /* Do nothing */ }
                        _ => {
                            entity_commands.insert(NavMeshAffector);
                        }
                    }

                    match internal_collider.behaviour {
                        ColliderBehaviour::Dynamic | ColliderBehaviour::GenerationDynamic => {
                            entity_commands.insert(RigidBody::Dynamic);
                        }
                        ColliderBehaviour::Kinematic => {
                            entity_commands.insert(RigidBody::KinematicPositionBased);
                        }
                    }
                }
            }
        }

        // If the event has a parent entity, set it as the parent of this new entity
        if let Some(parent) = event.parent {
            commands.entity(parent_entity).set_parent(parent);
        }
    }
}

pub fn point_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<PointLightSpawnEvent>,
) {
    for event in reader.read() {
        let entity = spawn_point_light(
            &mut commands,
            event.light.clone(),
            Transform::from(event.transform.clone()),
        );
        if let Some(parent) = event.parent {
            commands.entity(entity).set_parent(parent);
        }
    }
}

pub fn spot_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<SpotLightSpawnEvent>,
) {
    for event in reader.read() {
        let entity = spawn_spot_light(
            &mut commands,
            event.light.clone(),
            Transform::from(event.transform.clone()),
        );
        if let Some(parent) = event.parent {
            commands.entity(entity).set_parent(parent);
        }
    }
}

pub fn directional_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<DirectionalLightSpawnEvent>,
    existing: Query<Entity, With<MainDirectionalLight>>,
) {
    for event in reader.read() {
        if let Some(parent) = event.parent {
            // Spawn a new directional light attached to the provided parent
            let entity = commands
                .spawn_empty()
                .insert(event.light.clone())
                .insert(Transform::from(event.transform.clone()))
                .insert(InheritedVisibility::default())
                .insert(Name::new("DirectionalLight"))
                .id();
            commands.entity(entity).set_parent(parent);
        } else {
            // Update existing main directional light if present, otherwise spawn a new one
            if let Some(entity) = existing.iter().next() {
                commands
                    .entity(entity)
                    .insert(event.light.clone())
                    .insert(Transform::from(event.transform.clone()));
            } else {
                commands
                    .spawn_empty()
                    .insert(event.light.clone())
                    .insert(Transform::from(event.transform.clone()))
                    .insert(InheritedVisibility::default())
                    .insert(Name::new("MainDirectionalLight"))
                    .insert(MainDirectionalLight);
            }
        }
    }
}

pub fn ambient_light_spawn_listener(
    mut reader: EventReader<AmbientLightSpawnEvent>,
    mut ambient: ResMut<AmbientLight>,
) {
    for event in reader.read() {
        *ambient = event.light.clone();
    }
}

pub fn distance_fog_spawn_listener(
    mut reader: EventReader<DistanceFogSpawnEvent>,
    mut query: Query<&mut DistanceFog, With<MainCamera>>,
) {
    for event in reader.read() {
        for mut fog in &mut query {
            *fog = event.fog.clone();
        }
    }
}

pub fn sound_effect_spawn_listener(
    mut reader: EventReader<SoundEffectSpawnEvent>,
    sfx: Res<AudioChannel<SoundEffects>>,
    asset_server: Res<AssetServer>,
) {
    for event in reader.read() {
        let handle: Handle<AudioSource> = asset_server.load(event.file.as_str());
        // Play as a one-shot on the SFX channel
        sfx.play(handle);
    }
}

pub fn background_music_spawn_listener(
    mut reader: EventReader<BackgroundMusicSpawnEvent>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
) {
    for event in reader.read() {
        let handle: Handle<AudioSource> = asset_server.load(event.file.as_str());
        // Stop any currently playing global track and start looping the new one
        audio.stop();
        audio.play(handle).looped();
    }
}

pub fn nest_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<NestSpawnEvent>,
) {
    for event in reader.read() {
        match Structure::try_from(&event.reference) {
            Ok(structure) => {
                // Create a container entity for the nested structure
                let container = commands
                    .spawn_empty()
                    .insert(Transform::from(event.transform.clone()))
                    .insert(InheritedVisibility::default())
                    .insert(Name::new(structure.structure_name.clone()))
                    .id();

                if let Some(parent) = event.parent {
                    commands.entity(container).set_parent(parent);
                }

                // Attach Tags from the structure to the container (if any)
                let tags = Tags(structure.tags.clone());
                println!(
                    "[Spawn] Nest: structure '{}' -> container {:?} | tags = {:?}",
                    structure.structure_name, container, tags.0
                );
                if tags.len() != 0 {
                    commands.entity(container).insert(tags);
                    println!("[Spawn] Nest: tags inserted on {:?}", container);
                }

                let _ = spawn_structure_data(
                    &mut commands,
                    &structure,
                    Transform::IDENTITY,
                    Some(container),
                );
            }
            Err(e) => {
                eprintln!("NestSpawnEvent import error: {:?}", e);
            }
        }
    }
}

pub fn choose_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<ChooseSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
) {
    for event in reader.read() {
        match Structure::try_from(&event.list) {
            Ok(structure_list) => {
                // Pick one
                let sub_structure = structure_list.create_random_substructure(&1usize, gen_rng.rng_mut());
                // Directly spawn children under the provided parent, applying the event transform
                let _ = spawn_structure_data(
                    &mut commands,
                    &sub_structure,
                    Transform::from(event.transform.clone()),
                    event.parent,
                );
            }
            Err(e) => {
                eprintln!("ChooseSpawnEvent import error: {:?}", e);
            }
        }
    }
}

// Temporary no-op stubs to satisfy system registrations
pub fn choose_some_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<ChooseSomeSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
) {
    for event in reader.read() {
        match Structure::try_from(&event.list) {
            Ok(structure_list) => {
                let sub_structure = structure_list.create_random_substructure(&event.count, gen_rng.rng_mut());
                // Directly spawn children under the provided parent, applying the event transform
                let _ = spawn_structure_data(
                    &mut commands,
                    &sub_structure,
                    Transform::from(event.transform.clone()),
                    event.parent,
                );
            }
            Err(e) => {
                eprintln!("ChooseSomeSpawnEvent import error: {:?}", e);
            }
        }
    }
}

pub fn rand_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<RandSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
) {
    for event in reader.read() {
        let jiggled = jiggle_transform(&mut gen_rng, event.rand.clone(), event.transform.clone());
        let reference = event.reference.clone();
        let parent = event.parent;
        commands.queue(move |world: &mut World| {
            world.send_event(NestSpawnEvent { reference, transform: jiggled, parent });
        });
    }
}

pub fn probability_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<ProbabilitySpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
) {
    for event in reader.read() {
        if gen_rng.rng_mut().gen::<f32>() < event.probability {
            let reference = event.reference.clone();
            let transform = event.transform.clone();
            let parent = event.parent;
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform, parent });
            });
        } // else skip spawn
    }
}

pub fn loop_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<LoopSpawnEvent>,
) {
    for event in reader.read() {
        // Container for grouping loop spawns
        let container = commands
            .spawn_empty()
            .insert(Transform::from(event.transform.clone()))
            .insert(InheritedVisibility::default())
            .insert(Name::new("Loop"))
            .id();

        if let Some(parent) = event.parent {
            commands.entity(container).set_parent(parent);
        }

        let positions = get_looped_position_list(
            Transform::from(event.transform.clone()).translation,
            event.shift_transform.clone(),
            event.count,
        );

        let child_transforms: Vec<EulerTransform> = (0..event.count)
            .map(|n| EulerTransform {
                translation: (
                    event.child_transform.translation.0 * n as f32,
                    event.child_transform.translation.1 * n as f32,
                    event.child_transform.translation.2 * n as f32,
                ),
                rotation: (
                    event.child_transform.rotation.0 * n as f32,
                    event.child_transform.rotation.1 * n as f32,
                    event.child_transform.rotation.2 * n as f32,
                ),
                scale: (
                    1.0 + event.child_transform.scale.0 * n as f32,
                    1.0 + event.child_transform.scale.1 * n as f32,
                    1.0 + event.child_transform.scale.2 * n as f32,
                ),
            })
            .collect();

        for (pos, offset) in positions.into_iter().zip(child_transforms.into_iter()) {
            let euler = EulerTransform {
                translation: (
                    pos.x + offset.translation.0,
                    pos.y + offset.translation.1,
                    pos.z + offset.translation.2,
                ),
                rotation: offset.rotation,
                scale: offset.scale,
            };

            let reference = event.reference.clone();
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform: euler, parent: Some(container) });
            });
        }
    }
}

pub fn nesting_loop_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<NestingLoopSpawnEvent>,
) {
    for event in reader.read() {
        let base = Transform::from(event.transform.clone());
        let step = Transform::from(event.repeated_transform.clone());

        for i in 0..event.count {
            let mut current = base;
            for _ in 0..i {
                current = current * step;
            }

            let reference = event.reference.clone();
            let parent = event.parent;
            let euler = EulerTransform::from(current);
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform: euler, parent });
            });
        }
    }
}

pub fn noise_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<NoiseSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
) {
    for event in reader.read() {
        // Container for grouping
        let container = commands
            .spawn_empty()
            .insert(Transform::from(event.transform.clone()))
            .insert(InheritedVisibility::default())
            .insert(Name::new("Noise Spawn"))
            .id();

        if let Some(parent) = event.parent {
            commands.entity(container).set_parent(parent);
        }

        // Build a temporary key to reuse generator helper
        let temp_key = StructureKey::NoiseSpawn {
            reference: event.reference.clone(),
            fbm: event.fbm.clone(),
            sample_size: event.sample_size.clone(),
            count: event.count,
            exclusivity_radius: event.exclusivity_radius,
            resolution_modifier: event.resolution_modifier,
        };

        let points = generate_noise_spawn_points(&temp_key, &mut gen_rng);

        for (x, y, z) in points.into_iter() {
            let base = event.transform.clone();
            let new_translation = Vec3::new(
                base.translation.0 + base.scale.0 * x,
                base.translation.1 + base.scale.1 * z,
                base.translation.2 + base.scale.2 * y,
            );

            let euler = EulerTransform {
                translation: (new_translation.x, new_translation.y, new_translation.z),
                rotation: (0.0, 0.0, 0.0),
                scale: (1.0, 1.0, 1.0),
            };

            let reference = event.reference.clone();
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform: euler, parent: Some(container) });
            });
        }
    }
}

pub fn path_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<PathSpawnEvent>,
) {
    for event in reader.read() {
        // Container for grouping
        let container = commands
            .spawn_empty()
            .insert(Transform::from(event.transform.clone()))
            .insert(InheritedVisibility::default())
            .insert(Name::new("Path Spawn"))
            .id();

        if let Some(parent) = event.parent {
            commands.entity(container).set_parent(parent);
        }

        let curve = CubicCardinalSpline::new(event.tension, event.points.clone()).to_curve();
        let positions: Vec<Vec3> = match event.spread {
            SpreadData::Regular => {
                curve.unwrap().iter_positions(event.count as usize).collect()
            }
            _ => {
                panic!("This spread type not supported yet!");
            }
        };

        for point in positions.into_iter() {
            let euler = EulerTransform {
                translation: (point.x, point.y, point.z),
                rotation: (0.0, 0.0, 0.0),
                scale: (1.0, 1.0, 1.0),
            };

            let reference = event.reference.clone();
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform: euler, parent: Some(container) });
            });
        }
    }
}

pub fn reflection_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<ReflectionSpawnEvent>,
) {
    for event in reader.read() {
        if event.reflect_child {
            // Reflect children individually: spawn original children and their reflected counterparts
            match Structure::try_from(&event.reference) {
                Ok(structure) => {
                    // Container anchored at the provided transform
                    let container = commands
                        .spawn_empty()
                        .insert(Transform::from(event.transform.clone()))
                        .insert(InheritedVisibility::default())
                        .insert(Name::new(format!("{} (Child Reflection)", structure.structure_name)))
                        .id();

                    if let Some(parent) = event.parent {
                        commands.entity(container).set_parent(parent);
                    }

                    // Attach structure tags to container if any
                    let tags = Tags(structure.tags.clone());
                    println!(
                        "[Spawn] Reflection(child): '{}' -> container {:?} | tags = {:?}",
                        structure.structure_name, container, tags.0
                    );
                    if tags.len() != 0 {
                        commands.entity(container).insert(tags);
                        println!("[Spawn] Reflection(child): tags inserted on {:?}", container);
                    }

                    // Build a composite structure with original and reflected children
                    let mut combined_data: Vec<(StructureKey, EulerTransform)> = Vec::with_capacity(structure.data.len() * 2);

                    // World position of the container for local<->world conversion
                    let base = Transform::from(event.transform.clone()).translation;

                    for (key, child_euler) in structure.data.iter() {
                        // Original child (unchanged)
                        combined_data.push((key.clone(), child_euler.clone()));

                        // Compute reflected child's translation in world space, then convert back to local
                        let child_local = Vec3::new(child_euler.translation.0, child_euler.translation.1, child_euler.translation.2);
                        let child_world = base + child_local;
                        let reflected_world = reflect_point(child_world, event.reflection_plane, event.reflection_point);
                        let reflected_local = reflected_world - base;

                        let mut reflected_child = child_euler.clone();
                        reflected_child.translation = (reflected_local.x, reflected_local.y, reflected_local.z);

                        combined_data.push((key.clone(), reflected_child));
                    }

                    let composite = Structure {
                        structure_name: format!("{} (+Reflected)", structure.structure_name),
                        tags: vec![],
                        data: combined_data,
                    };

                    let _ = spawn_structure_data(
                        &mut commands,
                        &composite,
                        Transform::IDENTITY,
                        Some(container),
                    );
                }
                Err(e) => {
                    eprintln!("ReflectionSpawnEvent import error: {:?}", e);
                }
            }
            continue;
        }

        // Grouping container (identity like old implementation)
        let container = commands
            .spawn_empty()
            .insert(Transform::IDENTITY)
            .insert(InheritedVisibility::default())
            .insert(Name::new("Reflection"))
            .id();

        if let Some(parent) = event.parent {
            commands.entity(container).set_parent(parent);
        }

        let original = event.transform.clone();
        let local_pos = Vec3::new(original.translation.0, original.translation.1, original.translation.2);
        let reflected_location = reflect_point(local_pos, event.reflection_plane, event.reflection_point);
        let mut reflected = original.clone();
        reflected.translation = (reflected_location.x, reflected_location.y, reflected_location.z);

        let reference_a = event.reference.clone();
        let euler_a = original.clone();
        commands.queue(move |world: &mut World| {
            world.send_event(NestSpawnEvent { reference: reference_a, transform: euler_a, parent: Some(container) });
        });

        let reference_b = event.reference.clone();
        let euler_b = reflected.clone();
        commands.queue(move |world: &mut World| {
            world.send_event(NestSpawnEvent { reference: reference_b, transform: euler_b, parent: Some(container) });
        });
    }
}

pub fn selective_replacement_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<SelectiveReplacementSpawnEvent>,
    // The actual replacement is deferred and handled by selective_replacement_progressor
) {
    for event in reader.read() {
        // 1) Spawn the initial structure under the provided parent/transform
        let initial_structure = match Structure::try_from(&event.initial_reference) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("SelectiveReplacement initial import error: {:?}", e);
                continue;
            }
        };

        let container = commands
            .spawn_empty()
            .insert(Transform::from(event.transform.clone()))
            .insert(InheritedVisibility::default())
            .insert(Name::new(initial_structure.structure_name.clone()))
            .id();

        if let Some(parent) = event.parent {
            commands.entity(container).set_parent(parent);
        }

        println!(
            "[SelectiveReplacement] Start: initial '{}' -> container {:?}",
            initial_structure.structure_name, container
        );

        // Attach Tags on the container if the structure has them
        let container_tags = Tags(initial_structure.tags.clone());
        println!(
            "[SelectiveReplacement] Container tags = {:?}",
            container_tags.0
        );
        if container_tags.len() != 0 {
            commands.entity(container).insert(container_tags);
            println!("[SelectiveReplacement] Tags inserted on container {:?}", container);
        }

        let _ = spawn_structure_data(
            &mut commands,
            &initial_structure,
            Transform::IDENTITY,
            Some(container),
        );
        println!(
            "[SelectiveReplacement] Finished enqueueing initial '{}' children under {:?}",
            initial_structure.structure_name, container
        );

        // 2) Defer the replacement: attach a pending component to the container.
        commands.entity(container).insert(SelectiveReplacementPending {
            replacement_reference: event.replacement_reference.clone(),
            tags: event.tags.clone(),
            replace_count: event.replace_count,
            last_descendant_count: 0,
            last_candidate_count: 0,
            stable_frames: 0,
        });
    }
}

// Runs each frame to check if the subtree under containers with SelectiveReplacementPending has stabilized.
pub fn selective_replacement_progressor(
    mut commands: Commands,
    mut pending_query: Query<(Entity, &mut SelectiveReplacementPending)>,
    parent_query: Query<&Parent>,
    transform_query: Query<&Transform>,
    tag_query: Query<(Entity, &Tags, Option<&Name>)>,
    any_entity_query: Query<Entity>,
    mut gen_rng: ResMut<GenRng>,
) {
    for (container, mut pending) in pending_query.iter_mut() {
        // Count all descendants (regardless of tags). This stabilizes only when the subtree finished expanding.
        let mut total_descendants = 0usize;
        for entity in any_entity_query.iter() {
            if entity != container && is_descendant(container, entity, &parent_query) {
                total_descendants += 1;
            }
        }

        if total_descendants == 0 {
            // Nothing spawned yet under this container — keep waiting.
            if pending.last_descendant_count != 0 {
                println!(
                    "[SelectiveReplacement][Deferred] descendants changed: {} -> {} (waiting)",
                    pending.last_descendant_count, total_descendants
                );
                pending.last_descendant_count = 0;
            }
            pending.stable_frames = 0;
            continue;
        }

        if total_descendants != pending.last_descendant_count {
            println!(
                "[SelectiveReplacement][Deferred] descendants changed: {} -> {} (waiting)",
                pending.last_descendant_count, total_descendants
            );
            pending.last_descendant_count = total_descendants;
            pending.stable_frames = 0;
            // Wait for tag candidates as well, but since descendants changed this frame, defer immediately
            continue;
        }

        // Count current candidates under this container
        let mut candidates: Vec<(Entity, Option<String>)> = Vec::new();
        let mut count = 0usize;
        for (entity, entity_tags, name_opt) in tag_query.iter() {
            if entity_tags.0.iter().any(|t| pending.tags.contains(t)) && is_descendant(container, entity, &parent_query) {
                count += 1;
                candidates.push((entity, name_opt.map(|n| n.as_str().to_string())));
            }
        }

        // If there are still zero candidates, do not proceed. Keep waiting.
        if count == 0 {
            if pending.last_candidate_count != 0 {
                println!(
                    "[SelectiveReplacement][Deferred] candidates changed: {} -> {} (waiting)",
                    pending.last_candidate_count, count
                );
                pending.last_candidate_count = 0;
            } else {
                println!("[SelectiveReplacement][Deferred] still 0 candidates; waiting");
            }
            pending.stable_frames = 0;
            continue;
        }

        if count != pending.last_candidate_count {
            println!(
                "[SelectiveReplacement][Deferred] candidates changed: {} -> {} (waiting)",
                pending.last_candidate_count, count
            );
            pending.last_candidate_count = count;
            pending.stable_frames = 0;
            continue;
        } else {
            pending.stable_frames = pending.stable_frames.saturating_add(1);
        }

        // Wait until candidates are stable for at least 2 frames to ensure child spawns have completed
        if pending.stable_frames < 2 {
            continue;
        }

        // Perform replacement now
        println!(
            "[SelectiveReplacement][Deferred] stabilized with {} candidates. Proceeding to replace {}",
            count, pending.replace_count
        );

        // Resolve the replacement structure
        let replacement_structure = match Structure::try_from(&pending.replacement_reference) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("SelectiveReplacement replacement import error: {:?}", e);
                // Remove the pending to avoid infinite retry
                commands.entity(container).remove::<SelectiveReplacementPending>();
                continue;
            }
        };

        // Choose targets
        let chosen: Vec<(Entity, Option<String>)> = candidates
            .into_iter()
            .choose_multiple(gen_rng.rng_mut(), pending.replace_count);
        println!(
            "[SelectiveReplacement] Chosen {} entities to replace (replace_count = {})",
            chosen.len(), pending.replace_count
        );

        for (target, name_opt) in chosen {
            println!(
                "[SelectiveReplacement] Replacing entity {:?} name {:?}",
                target, name_opt
            );
            // Get parent of the target and its transform
            let parent_of_target = parent_query.get(target).ok().map(|p| p.get());
            let Ok(target_transform) = transform_query.get(target) else { continue; };

            // Despawn target
            commands.entity(target).despawn_recursive();

            // Spawn replacement container
            let repl_container = commands
                .spawn_empty()
                .insert(Transform::from(*target_transform))
                .insert(InheritedVisibility::default())
                .insert(Name::new(replacement_structure.structure_name.clone()))
                .id();

            if let Some(parent_ent) = parent_of_target {
                commands.entity(repl_container).set_parent(parent_ent);
            }

            // Attach tags from replacement structure
            let repl_tags = Tags(replacement_structure.tags.clone());
            println!(
                "[SelectiveReplacement] Replacement '{}' -> container {:?} | tags = {:?}",
                replacement_structure.structure_name, repl_container, repl_tags.0
            );
            if repl_tags.len() != 0 {
                commands.entity(repl_container).insert(repl_tags);
                println!("[SelectiveReplacement] Tags inserted on replacement container {:?}", repl_container);
            }

            let _ = spawn_structure_data(
                &mut commands,
                &replacement_structure,
                Transform::IDENTITY,
                Some(repl_container),
            );
        }

        // Done for this container
        commands.entity(container).remove::<SelectiveReplacementPending>();
    }
}

fn is_descendant(ancestor: Entity, child: Entity, parent_query: &Query<&Parent>) -> bool {
    let mut current = child;
    while let Ok(parent) = parent_query.get(current) {
        if parent.get() == ancestor {
            return true;
        }
        current = parent.get();
    }
    false
}
