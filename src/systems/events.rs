use crate::proc_gen::spawning::mesh_spawning::spawn_mesh;
use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioChannel, AudioControl};
use bevy_kira_audio::prelude::AudioEmitter;
use crate::audio_manager::SoundEffects;
use crate::generation::GenRng;
use crate::material_autoloader::MaterialCache;
use crate::proc_gen::spawning::structure_spawning::spawn_structure_by_name;
use crate::proc_gen::spawning::scene_spawning::spawn_scene_from_path;
use crate::proc_gen::core::structure_key::StructureKey;
use crate::proc_gen::core::tmaterial::TMaterial;
use crate::proc_gen::spawning::euler_transform::EulerTransform;
use crate::proc_gen::core::components::MainCamera;
use crate::proc_gen::core::components::MainDirectionalLight;

#[derive(Event)]
pub enum ObjectSpawnEvent {
    MeshSpawn {
        mesh: Mesh,
        transform: EulerTransform,
        material: TMaterial,
    },
    SceneSpawn {
        data: StructureKey,
        transform: EulerTransform,
    },
    StructureSpawn {
        structure: String,
        transform: EulerTransform,
    }
}

#[derive(Event)]
pub enum FogEvent {
    SetFog {
        fog: FogSettings
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
    mut sfx_writer: EventWriter<SFXEvent>
) {
    for spawn_event in spawn_reader.read() {
        match spawn_event {
            ObjectSpawnEvent::MeshSpawn { mesh, transform, material } => {
                spawn_mesh(
                    &mut commands,
                    &material_cache,
                    &mut meshes,
                    mesh,
                    Transform::from(transform.clone()),
                    &material
                );
            },
            ObjectSpawnEvent::SceneSpawn { data, transform } => {
                spawn_scene_from_path(
                    &mut commands,
                    &asset_server,
                    data,
                    Transform::from(transform.clone()),
                    Transform::IDENTITY
                );
            },
            ObjectSpawnEvent::StructureSpawn { structure, transform } => {
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
                    None
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
    mut fog_query: Query<&mut FogSettings, With<MainCamera>>,
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
                    AudioEmitter {
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
