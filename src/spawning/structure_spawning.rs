use bevy::prelude::*;
use rand::Rng;
pub(crate) use crate::core::structure::{Structure};
use crate::core::structure_key::StructureKey;
use crate::core::structure_reference::StructureReference;
use crate::core::structure_error::StructureError;
use crate::core::tags::Tags;
use crate::management::structure_management::import_structure;
use crate::spawning::transformation::*;
use crate::core::spread_data::SpreadData;
use crate::spawning::euler_transform::EulerTransform;
use crate::spawning::helpers::{GenRng, jiggle_transform, reflect_point};
use crate::spawning::light_spawning::{spawn_point_light, spawn_spot_light};
use crate::spawning::scene_spawning::spawn_scene_from_path;
use crate::systems::events::{AmbLightEvent, BGMusicEvent, DirLightEvent, FogEvent, SelectiveReplacementEvent, SFXEvent};

pub(crate) fn spawn_structure_by_name(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    structure_name: String,
    parent_transform: Transform,
    struct_stack: &mut Vec<String>,
    gen_rng: &mut ResMut<GenRng>,
    dir_light_writer: &mut EventWriter<DirLightEvent>,
    amb_light_writer: &mut EventWriter<AmbLightEvent>,
    fog_writer: &mut EventWriter<FogEvent>,
    music_writer: &mut EventWriter<BGMusicEvent>,
    sfx_writer: &mut EventWriter<SFXEvent>,
    selective_replacement_writer: &mut EventWriter<SelectiveReplacementEvent>,
    parent: Option<Entity>
) -> Result<Option<Entity>, StructureError> {
    if struct_stack.contains(&structure_name) {
        return Err(StructureError::CycleDetected(structure_name));
    }

    if struct_stack.len() >= 100 {
        return Err(StructureError::Other(format!("Maximum recursion depth exceeded while processing {}", structure_name)));
    }

    struct_stack.push(structure_name.clone());

    let formatted_name = format!("{:?}", structure_name.clone());
    let ent = commands.spawn(PbrBundle { ..default() })
        .insert(Name::new(formatted_name))
        .id();

    if let Some(parent) = parent {
        commands.entity(parent).push_children(&[ent]);
    }

    let result = match import_structure(structure_name.clone()) {
        Ok(structure) => {
            let tags = Tags(structure.tags.clone());
            if tags.len() != 0 {
                commands.entity(ent).insert(tags);
            }

            spawn_structure_by_data(
                commands,
                asset_server,
                &structure,
                parent_transform,
                struct_stack,
                gen_rng,
                dir_light_writer,
                amb_light_writer,
                fog_writer,
                music_writer,
                sfx_writer,
                selective_replacement_writer,
                Some(ent)
            )
        },
        Err(e) => {
            return Err(StructureError::ImportFailed(format!("Error importing {:?}: {:?}", structure_name, e)));
        }
    };

    struct_stack.pop();

    match result {
        Ok(child_maybe) => {
            if let Some(child) = child_maybe {
                commands.entity(ent).push_children(&[child]);
                Ok(Some(child))
            } else {
                Ok(None)
            }
        }
        Err(e) => Err(StructureError::ImportFailed(format!("Error importing {:?}: {:?}", structure_name, e))),
    }
}

pub(crate) fn spawn_structure_by_data(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    structure: &Structure,
    parent_transform: Transform,
    struct_stack: &mut Vec<String>,
    gen_rng: &mut ResMut<GenRng>,
    dir_light_writer: &mut EventWriter<DirLightEvent>,
    amb_light_writer: &mut EventWriter<AmbLightEvent>,
    fog_writer: &mut EventWriter<FogEvent>,
    music_writer: &mut EventWriter<BGMusicEvent>,
    sfx_writer: &mut EventWriter<SFXEvent>,
    selective_replacement_writer: &mut EventWriter<SelectiveReplacementEvent>,
    parent: Option<Entity>
) -> Result<Option<Entity>, StructureError> {
    let entity = commands.spawn(PbrBundle { ..default() })
        .insert(Name::new("Data"))
        .id();

    if let Some(parent) = parent {
        commands.entity(parent).push_children(&[entity]);
    }

    for (key, local_transform) in &structure.data {
        let combined_transform = parent_transform * Transform::from(local_transform.clone());

        let child_entity = match key {
            StructureKey::Object {..} => {
                Some(spawn_scene_from_path(commands, asset_server, key, combined_transform, Transform::IDENTITY))
            },
            StructureKey::Nest(reference) => {
                let struc = Structure::try_from(reference)?;
                spawn_structure_by_data(
                    commands,
                    asset_server,
                    &struc,
                    combined_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?
            }
            StructureKey::PointLight(light) => {
                Some(spawn_point_light(commands, *light, combined_transform))
            },
            StructureKey::ProbabilitySpawn { reference, probability } => {
                if gen_rng.rng_mut().gen::<f32>() < *probability {
                    let struc = Structure::try_from(reference)?;
                    spawn_structure_by_data(
                        commands,
                        asset_server,
                        &struc,
                        combined_transform,
                        struct_stack,
                        gen_rng,
                        dir_light_writer,
                        amb_light_writer,
                        fog_writer,
                        music_writer,
                        sfx_writer,
                        selective_replacement_writer,
                        parent
                    )?
                } else {
                    Some(commands.spawn(()).insert(Name::new("Probabalistically Rejected")).id())
                }
            }
            StructureKey::Choose { list } => {
                let struc = Structure::try_from(list)?;
                let sub_struc = struc.create_random_substructure(&(1usize), gen_rng.rng_mut());
                spawn_structure_by_data(
                    commands,
                    asset_server,
                    &sub_struc,
                    combined_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?
            },
            StructureKey::ChooseSome { list, count: num } => {
                let struc = Structure::try_from(list)?;
                let sub_struc = struc.create_random_substructure(num, gen_rng.rng_mut());
                spawn_structure_by_data(
                    commands,
                    asset_server,
                    &sub_struc,
                    combined_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?
            },
            StructureKey::Loop { reference, shift_transform, child_transform, count } => {
                let positions = get_looped_position_list(combined_transform.translation, shift_transform.clone().into(), *count);

                let child_transforms: Vec<EulerTransform> = (0..*count).map(|n| {
                    EulerTransform {
                        translation: (child_transform.translation.0 * n as f32, child_transform.translation.1 * n as f32, child_transform.translation.2 * n as f32),
                        rotation: (child_transform.rotation.0 * n as f32, child_transform.rotation.1 * n as f32, child_transform.rotation.2 * n as f32),
                        scale: (1.0 + child_transform.scale.0 * n as f32, 1.0 + child_transform.scale.1 * n as f32, 1.0 + child_transform.scale.2 * n as f32)
                    }
                }).collect();

                let structure = Structure {
                    structure_name: "Loop".to_string(),
                    tags: vec![],
                    data: positions.into_iter().zip(child_transforms.iter())
                        .map(|(pos, transform)| {
                            let euler_transform = EulerTransform {
                                translation: (pos.x + transform.translation.0, pos.y + transform.translation.1, pos.z + transform.translation.2),
                                rotation: (transform.rotation.0, transform.rotation.1, transform.rotation.2),
                                scale: (transform.scale.0, transform.scale.1, transform.scale.2)
                            };
                            (StructureKey::Nest(reference.clone()), euler_transform)
                        })
                        .collect()
                };

                spawn_structure_by_data(commands,
                                        asset_server,
                                        &structure,
                                        combined_transform,
                                        struct_stack,
                                        gen_rng,
                                        dir_light_writer,
                                        amb_light_writer,
                                        fog_writer,
                                        music_writer,
                                        sfx_writer,
                                        selective_replacement_writer,
                                        parent)?
            }
            StructureKey::DirectionalLight(light) => {
                dir_light_writer.send(DirLightEvent::SetDirLight {
                    light: light.clone(),
                    transform: combined_transform,
                });
                None
            }
            StructureKey::AmbientLight(light) => {
                amb_light_writer.send(AmbLightEvent::SetAmbLight {
                    light: light.clone(),
                });
                None
            }
            StructureKey::FogSettings(fog) => {
                fog_writer.send(FogEvent::SetFog {
                    fog: fog.clone(),
                });
                None
            }
            StructureKey::BackgroundMusic(filepath) => {
                music_writer.send(BGMusicEvent::SetBGMusic {
                    filepath: filepath.clone()
                });
                None
            }
            StructureKey::SoundEffect(filepath) => {
                let ent = commands.spawn((
                    TransformBundle {
                        local: combined_transform,
                        global: Default::default(),
                    },
                    Name::new(format!("AudioEmitter: {:?}", filepath)),
                )).id();

                sfx_writer.send(SFXEvent::CreateAudioEmitter {
                    filepath: filepath.clone(),
                    entity: ent
                });
                Some(ent)
            }
            StructureKey::SpotLight(light) => {
                Some(spawn_spot_light(commands, *light, combined_transform))
            },
            StructureKey::Rand { reference, rand } => {
                let jiggled = jiggle_transform(gen_rng, rand.clone(), local_transform.clone());

                let structure = Structure {
                    structure_name: "Rand".to_string(),
                    tags: vec![],
                    data: vec![(StructureKey::Nest(reference.clone()), jiggled)]
                };

                spawn_structure_by_data(
                    commands,
                    asset_server,
                    &structure,
                    parent_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?
            }
            StructureKey::NoiseSpawn { reference, .. } => {
                let points = generate_noise_spawn_points(key, gen_rng);

                let struc_data: Vec<(StructureKey, EulerTransform)> = points.iter().map(|(x, y, z)| {
                    let new_translation = Vec3::new(
                        local_transform.translation.0 + local_transform.scale.0 * *x,
                        local_transform.translation.1 + local_transform.scale.1 * *z,
                        local_transform.translation.2 + local_transform.scale.2 * *y,
                    );

                    let new_transform = EulerTransform {
                        translation: (new_translation.x, new_translation.y, new_translation.z),
                        rotation: (0.0, 0.0, 0.0),
                        scale: (1.0, 1.0, 1.0),
                    };

                    (StructureKey::Nest(reference.clone()), new_transform)
                }).collect();

                let structure_unwrapped = Structure {
                    structure_name: "Noise Spawn".to_string(),
                    tags: vec![],
                    data: struc_data
                };

                let raw_nesting = StructureReference::Raw {
                    structure: Box::new(structure_unwrapped),
                    ownership: match reference {
                        StructureReference::Raw { ownership, .. } => ownership.clone(),
                        StructureReference::Ref { ownership, .. } => ownership.clone(),
                    },
                };

                let structure = Structure {
                    structure_name: "Nested Noise Spawn".to_string(),
                    tags: vec![],
                    data: vec![
                        (StructureKey::Nest(raw_nesting), parent_transform.into())
                    ],
                };

                spawn_structure_by_data(
                    commands,
                    asset_server,
                    &structure,
                    parent_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?
            }
            StructureKey::PathSpawn { reference, points, tension, spread, count } => {
                let curve = CubicCardinalSpline::new(*tension, points.clone()).to_curve();
                let positions: Vec<Vec3> = match spread {
                    SpreadData::Regular => {
                        curve.iter_positions(*count as usize).collect()
                    },
                    _ => {
                        panic!("This spread type not supported yet!");
                    },
                };

                let struc_data: Vec<(StructureKey, EulerTransform)> = positions.iter().map(|point| {
                    let euler_transform = EulerTransform {
                        translation: (point.x, point.y, point.z),
                        rotation: (0.0, 0.0, 0.0),
                        scale: (1.0, 1.0, 1.0),
                    };

                    (StructureKey::Nest(reference.clone()), euler_transform)
                }).collect();

                let structure = Structure {
                    structure_name: "Path Spawn".to_string(),
                    tags: vec![],
                    data: struc_data
                };

                spawn_structure_by_data(
                    commands,
                    asset_server,
                    &structure,
                    parent_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?
            }
            StructureKey::Reflection { reference, reflection_plane, reflection_point, reflect_child } => {
                let child = commands.spawn(PbrBundle { ..default() }).id();

                if *reflect_child {
                    return Err("Child reflection not implemented!".into());
                }

                let reflected_location = reflect_point(
                    Vec3::new(local_transform.translation.0, local_transform.translation.1, local_transform.translation.2),
                    *reflection_plane,
                    *reflection_point
                );

                let mut reflected_transform = local_transform.clone();
                reflected_transform.translation = (reflected_location.x, reflected_location.y, reflected_location.z);

                let reflected_combined_transform = parent_transform * Transform::from(reflected_transform);

                let structure_internal = match Structure::try_from(reference) {
                    Ok(structure) => structure,
                    Err(error) => {
                        return Err(error);
                    }
                };

                let reflect_a = spawn_structure_by_data(
                    commands,
                    asset_server,
                    &structure_internal,
                    combined_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?;

                let reflect_b = spawn_structure_by_data(
                    commands,
                    asset_server,
                    &structure_internal,
                    reflected_combined_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?;

                commands.entity(child).push_children(&[reflect_a.unwrap(), reflect_b.unwrap()]);

                Some(child)
            }
            StructureKey::NestingLoop { reference, repeated_transform, count } => {
                let mut structure_data = Vec::new();

                for i in 0..*count {
                    let mut current_transform = combined_transform;
                    for _ in 0..i {
                        current_transform = current_transform * Transform::from(repeated_transform.clone());
                    }
                    structure_data.push((StructureKey::Nest(reference.clone()), current_transform.into()));
                }

                let structure = Structure {
                    structure_name: "Nesting Loop".to_string(),
                    tags: vec![],
                    data: structure_data,
                };

                spawn_structure_by_data(
                    commands,
                    asset_server,
                    &structure,
                    combined_transform,
                    struct_stack,
                    gen_rng,
                    dir_light_writer,
                    amb_light_writer,
                    fog_writer,
                    music_writer,
                    sfx_writer,
                    selective_replacement_writer,
                    parent
                )?
            }
            StructureKey::SelectiveReplacement { initial_reference, replacement_reference, tags, replace_count } => {
                // Convert initial structure reference to a structure
                if let Ok(initial_structure) = Structure::try_from(initial_reference) {
                    // Spawn the initial structure
                    let child = spawn_structure_by_data(
                        commands,
                        asset_server,
                        &initial_structure,
                        combined_transform,
                        struct_stack,
                        gen_rng,
                        dir_light_writer,
                        amb_light_writer,
                        fog_writer,
                        music_writer,
                        sfx_writer,
                        selective_replacement_writer,
                        Some(entity),
                    )?;

                    if let Some(child) = child {
                        // Send the SelectiveReplacementEvent
                        selective_replacement_writer.send(SelectiveReplacementEvent::Replace {
                            entity: child,
                            replacement_reference: replacement_reference.clone(),
                            tags: tags.0.clone(),
                            replace_count: *replace_count,
                        });
                    }

                    Some(entity)
                } else {
                    None
                }
            }
        };

        if let Some(child) = child_entity {
            commands.entity(child).insert(Name::new(format!("{:?}", key.variant_name())));

            if let Some(tags) = key.get_tags() {
                commands.entity(child).insert(tags);
            }

            commands.entity(entity).push_children(&[child]);
        }
    }

    Ok(Some(entity))
}