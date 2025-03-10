use crate::spawning::mesh_spawning::spawn_mesh;
use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioChannel, AudioControl};
use bevy_kira_audio::SpatialAudioEmitter;
use rand::prelude::IteratorRandom;
use crate::spawning::structure_spawning::spawn_structure_by_name;
use crate::spawning::scene_spawning::spawn_scene_from_path;
use crate::core::structure_key::StructureKey;
use crate::core::tmaterial::TMaterial;
use crate::spawning::euler_transform::EulerTransform;
use crate::core::components::MainCamera;
use crate::core::components::MainDirectionalLight;
use crate::management::audio_management::SoundEffects;
use crate::serialization::caching::{MaterialCache};
use crate::spawning::helpers::GenRng;
use crate::core::structure_reference::StructureReference;
use crate::core::tags::Tags;

#[derive(Event)]
pub enum ObjectSpawnEvent {
    MeshSpawn {
        mesh: Mesh,
        transform: EulerTransform,
        material: TMaterial,
        parent: Option<Entity>,
    },
    SceneSpawn {
        data: StructureKey,
        transform: EulerTransform,
        parent: Option<Entity>,
    },
    StructureSpawn {
        structure: String,
        transform: EulerTransform,
        parent: Option<Entity>,
    }
}

#[derive(Event)]
pub enum FogEvent {
    SetFog {
        fog: DistanceFog
    }
}

#[derive(Event)]
pub enum DirLightEvent {
    SetDirLight {
        light: DirectionalLight,
        transform: Transform,
    }
}

#[derive(Event)]
pub enum AmbLightEvent {
    SetAmbLight {
        light: AmbientLight
    }
}

#[derive(Event)]
pub enum BGMusicEvent {
    SetBGMusic {
        filepath: String
    }
}

#[derive(Event)]
pub enum SFXEvent {
    CreateAudioEmitter {
        filepath: String,
        entity: Entity
    }
}

#[derive(Event)]
pub enum SelectiveReplacementEvent {
    Replace {
        entity: Entity,
        replacement_reference: StructureReference,
        tags: Vec<String>,
        replace_count: usize,
    }
}

pub fn object_spawn_reader_system(
    mut spawn_reader: EventReader<ObjectSpawnEvent>,
    asset_server: Res<AssetServer>,
    material_cache: Res<MaterialCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gen_rng: ResMut<GenRng>,
    mut commands: Commands,
    mut dir_light_writer: EventWriter<DirLightEvent>,
    mut amb_light_writer: EventWriter<AmbLightEvent>,
    mut fog_writer: EventWriter<FogEvent>,
    mut music_writer: EventWriter<BGMusicEvent>,
    mut sfx_writer: EventWriter<SFXEvent>,
    mut selective_replacement_writer: EventWriter<SelectiveReplacementEvent>
) {
    for spawn_event in spawn_reader.read() {
        match spawn_event {
            ObjectSpawnEvent::MeshSpawn { mesh, transform, material, .. } => {
                spawn_mesh(
                    &mut commands,
                    &material_cache,
                    &mut meshes,
                    mesh,
                    Transform::from(transform.clone()),
                    &material
                );
            },
            ObjectSpawnEvent::SceneSpawn { data, transform, .. } => {
                spawn_scene_from_path(
                    &mut commands,
                    &asset_server,
                    data,
                    Transform::from(transform.clone()),
                    Transform::IDENTITY
                );
            },
            ObjectSpawnEvent::StructureSpawn { structure, transform, parent } => {
                match spawn_structure_by_name(
                    &mut commands,
                    &asset_server,
                    structure.clone(),
                    Transform::from(transform.clone()),
                    &mut vec![],
                    &mut gen_rng,
                    &mut dir_light_writer,
                    &mut amb_light_writer,
                    &mut fog_writer,
                    &mut music_writer,
                    &mut sfx_writer,
                    &mut selective_replacement_writer,
                    *parent
                ) {
                    Ok(_) => {},
                    Err(error) => {
                        eprintln!("Failed to spawn structure: {} because {:?}", structure, error);
                    }
                }
            }
        }
    }
}

pub fn fog_updater_system(
    mut update_reader: EventReader<FogEvent>,
    mut fog_query: Query<&mut DistanceFog, With<MainCamera>>,
) {
    for event in update_reader.read() {
        match event {
            FogEvent::SetFog { fog } => {
                for mut fog_settings in fog_query.iter_mut() {
                    *fog_settings = fog.clone();
                }
            }
        }
    }
}

pub fn directional_light_updater_system(
    mut update_reader: EventReader<DirLightEvent>,
    mut light_query: Query<(&mut DirectionalLight, &mut Transform), With<MainDirectionalLight>>,
) {
    for event in update_reader.read() {
        match event {
            DirLightEvent::SetDirLight { light: new_light, transform: new_transform } => {
                for (mut light, mut transform) in light_query.iter_mut() {
                    *light = new_light.clone();
                    *transform = new_transform.clone();
                }
            }
        }
    }
}

pub fn sfx_event_listener_system(
    mut update_reader: EventReader<SFXEvent>,
    sfx: Res<AudioChannel<SoundEffects>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for event in update_reader.read() {
        match event {
            SFXEvent::CreateAudioEmitter { filepath, entity } => {
                commands.entity(*entity).insert(
                    SpatialAudioEmitter {
                        instances: vec![
                            sfx.play(asset_server.load(filepath))
                                .looped()
                                .handle()
                        ]
                    }
                );
            }
        }
    }
}

pub fn ambient_light_updater_system(
    mut update_reader: EventReader<AmbLightEvent>,
    mut ambient_light: ResMut<AmbientLight>,
) {
    for event in update_reader.read() {
        match event {
            AmbLightEvent::SetAmbLight { light } => {
                *ambient_light = light.clone();
            }
        }
    }
}

pub fn background_music_updater_system(
    mut update_reader: EventReader<BGMusicEvent>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>
) {
    for event in update_reader.read() {
        match event {
            BGMusicEvent::SetBGMusic { filepath } => {
                audio.stop();
                audio.play(asset_server.load(filepath)).looped();
            }
        }
    }
}

pub fn selective_replacement_reader_system(
    mut commands: Commands,
    mut replacement_reader: EventReader<SelectiveReplacementEvent>,
    mut spawn_writer: EventWriter<ObjectSpawnEvent>,
    parent_query: Query<&Parent>,
    mut query: Query<(Entity, &Tags, &Transform)>,
    mut gen_rng: ResMut<GenRng>,
) {
    for event in replacement_reader.read() {
        match event {
            SelectiveReplacementEvent::Replace {
                entity,
                replacement_reference,
                tags,
                replace_count,
            } => {
                // Ensure only StructureReference::Ref is accepted
                if let StructureReference::Ref { structure,.. } = replacement_reference {
                    // Find entities with matching tags
                    let matching_entities: Vec<(Entity, &Transform)> = query
                        .iter_mut()
                        .filter_map(|(entity, entity_tags, transform)| {
                            if entity_tags.0.iter().any(|tag| tags.contains(tag)) {
                                Some((entity, transform))
                            } else {
                                None
                            }
                        })
                        .collect();

                    // Filter out all those that are NOT descendants of the entity listed in the Replace enum
                    let descendant_entities: Vec<(Entity, &Transform)> = matching_entities
                        .into_iter()
                        .filter(|&(e, _)| is_descendant(entity, e, &parent_query))
                        .collect();


                    let random_entities = descendant_entities.into_iter().choose_multiple(&mut gen_rng.rng_mut(), *replace_count);

                    // Process the filtered descendant entities
                    for (descendant, transform) in random_entities.iter().take(*replace_count) {
                        // Get the parent entity
                        let parent_entity = parent_query.get(*descendant).ok().map(|parent| parent.get());

                        // Despawn the entity and send a spawn event
                        commands.entity(*descendant).despawn_recursive();

                        spawn_writer.send(
                            ObjectSpawnEvent::StructureSpawn {
                                structure: structure.clone(),
                                transform: (*(*transform)).into(),
                                parent: parent_entity,
                            }
                        );
                    }
                }
            }
        }
    }
}


fn is_descendant(ancestor: &Entity, child: Entity, parent_query: &Query<&Parent>) -> bool {
    let mut current_entity = child;

    while let Ok(parent) = parent_query.get(current_entity) {
        if parent.get() == *ancestor {
            return true;
        }
        current_entity = parent.get();
    }

    false
}