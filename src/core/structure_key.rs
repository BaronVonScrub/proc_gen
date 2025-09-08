use crate::serialization::serialization::SerializableDistanceFog;
use crate::serialization::serialization::SerializableAmbientLight;
use crate::serialization::serialization::SerializableDirectionalLight;
use crate::serialization::serialization::SerializableSpotLight;
use crate::serialization::serialization::SerializablePointLight;
use serde::{Serialize, Deserialize};
use bevy::prelude::*;
use crate::core::collider::ColliderInfo;
use crate::core::fbm_data::FBMData;
use crate::core::rand_data::RandData;
use crate::core::sample_size::SampleSize;
use crate::core::spread_data::SpreadData;
use crate::core::structure_reference::StructureReference;
use crate::core::wobble::WobbleParams;
use crate::event_system::spawn_events::*;
use crate::management::structure_management::import_structure;
use crate::spawning::euler_transform::EulerTransform;
use crate::spawning::object_logic::{ObjectType, Ownership};
use bevy::ecs::world::World;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StructureKey {
    Object {
        path: String,
        collider: Option<ColliderInfo>,
        offset: Vec3,
        ownership: Ownership,
        selectable: bool,
        object_type: ObjectType,
    },
    #[serde(with = "SerializablePointLight")]
    PointLight(PointLight),
    #[serde(with = "SerializableSpotLight")]
    SpotLight(SpotLight),
    SoundEffect(String),
    #[serde(with = "SerializableDirectionalLight")]
    DirectionalLight(DirectionalLight),
    #[serde(with = "SerializableDirectionalLight")]
    MainDirectionalLight(DirectionalLight),
    #[serde(with = "SerializableAmbientLight")]
    AmbientLight(AmbientLight),
    #[serde(with = "SerializableDistanceFog")]
    DistanceFog(DistanceFog),
    BackgroundMusic(String),
    AtmosphereNishita {
        sun_position: Vec3,
        rayleigh_multiplier: Vec3,
        mie_multiplier: f32,
        mie_direction: f32,
        align_to_main_light: bool,
    },
    Nest(StructureReference),
    Choose {
        list: StructureReference,
    },
    ChooseSome {
        list: StructureReference,
        count: usize,
    },
    Rand {
        reference: StructureReference,
        rand: RandData,
    },
    ProbabilitySpawn {
        reference: StructureReference,
        probability: f32,
    },
    InPass {
        index: u8,
        reference: StructureReference,
    },
    Loop {
        reference: StructureReference,
        shift_transform: EulerTransform,
        child_transform: EulerTransform,
        count: usize,
    },
    LoopParam {
        reference: StructureReference,
        origin: Vec3,
        rotation: Vec3,
        distance: f32,
        child_position: Vec3,
        child_rotation: Vec3,
        child_scale: Vec3,
        count: usize,
    },
    NestingLoop {
        reference: StructureReference,
        repeated_transform: EulerTransform,
        count: usize,
    },
    NoiseSpawn {
        reference: StructureReference,
        fbm: FBMData,
        sample_size: SampleSize,
        count: u32,
        exclusivity_radius: f32,
        resolution_modifier: f32,
    },
    PathSpawn {
        reference: StructureReference,
        points: Vec<Vec3>,
        tension: f32,
        spread: SpreadData,
        count: u32,
    },
    PathToTag {
        reference: StructureReference,
        start: Vec3,
        manual_points: Option<Vec<Vec3>>, // optional local-space control points
        tag: String,
        tension: f32,
        spread: SpreadData,
        count: u32,
        wobble: Option<WobbleParams>,
    },
    RandDistDir {
        reference: StructureReference,
        dist_min: f32,
        dist_max: f32,
        angle_min_deg: f32,
        angle_max_deg: f32,
        y: f32,
    },
    Reflection {
        reference: StructureReference,
        reflection_plane: Plane3d,
        reflection_point: Vec3,
        reflect_child: bool,
    },
    SelectiveReplacement {
        initial_reference: StructureReference,
        replacement_reference: StructureReference,
        tags: Vec<String>,
        replace_count: usize,
    },
}

impl StructureKey {
    pub fn variant_name(&self) -> String {
        match self {
            StructureKey::Object { path, .. } => path.clone(),
            StructureKey::Nest(reference) => match reference {
                StructureReference::Raw { structure, .. } => structure.structure_name.clone(),
                StructureReference::Ref { structure, .. } => structure.clone(),
            },
            StructureKey::PointLight { .. } => "PointLight".to_string(),
            StructureKey::ProbabilitySpawn { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("Prob {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Prob {:?}", structure.clone()),
            },
            StructureKey::Choose { list, .. } => match list {
                StructureReference::Raw { structure, .. } => format!("Choose {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Choose {:?}", structure.clone()),
            },
            StructureKey::ChooseSome { list, .. } => match list {
                StructureReference::Raw { structure, .. } => format!("Some {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Some {:?}", structure.clone()),
            },
            StructureKey::Loop { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("Loop {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Loop {:?}", structure.clone()),
            },
            StructureKey::LoopParam { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("LoopParam {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("LoopParam {:?}", structure.clone()),
            },
            StructureKey::DirectionalLight { .. } => "DirectionalLight".to_string(),
            StructureKey::MainDirectionalLight { .. } => "MainDirectionalLight".to_string(),
            StructureKey::AmbientLight { .. } => "AmbientLight".to_string(),
            StructureKey::DistanceFog { .. } => "FogSettings".to_string(),
            StructureKey::AtmosphereNishita { .. } => "AtmosphereNishita".to_string(),
            StructureKey::BackgroundMusic { .. } => "BackgroundMusic".to_string(),
            StructureKey::SoundEffect { .. } => "SoundEffect".to_string(),
            StructureKey::SpotLight { .. } => "SpotLight".to_string(),
            StructureKey::Rand { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("Rand {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Rand {:?}", structure.clone()),
            },
            StructureKey::NoiseSpawn { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("Noise {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Noise {:?}", structure.clone()),
            },
            StructureKey::PathSpawn { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("Path {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Path {:?}", structure.clone()),
            },
            StructureKey::PathToTag { reference, tag, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("PathToTag {} -> {:?}", tag, structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("PathToTag {} -> {:?}", tag, structure.clone()),
            },
            StructureKey::Reflection { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("Reflect {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Reflect {:?}", structure.clone()),
            },
            StructureKey::RandDistDir { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("RandDistDir {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("RandDistDir {:?}", structure.clone()),
            },
            StructureKey::NestingLoop { reference, .. } => match reference {
                StructureReference::Raw { structure, .. } => format!("NLoop {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("NLoop {:?}", structure.clone()),
            },
            StructureKey::SelectiveReplacement { initial_reference, .. } => match initial_reference {
                StructureReference::Raw { structure, .. } => format!("SelectiveReplacement {:?}", structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("SelectiveReplacement {:?}", structure.clone()),
            },
            StructureKey::InPass { index, reference } => match reference {
                StructureReference::Raw { structure, .. } => format!("Pass{} {:?}", index, structure.structure_name.clone()),
                StructureReference::Ref { structure, .. } => format!("Pass{} {:?}", index, structure.clone()),
            },
        }
    }

    pub fn get_tags(&self) -> Option<Vec<String>> {
        let tags = match self {
            StructureKey::Nest(reference) => Self::extract_tags(reference),
            StructureKey::Choose { list } => Self::extract_tags(list),
            StructureKey::ChooseSome { list, .. } => Self::extract_tags(list),
            StructureKey::Rand { reference, .. } => Self::extract_tags(reference),
            StructureKey::ProbabilitySpawn { reference, .. } => Self::extract_tags(reference),
            StructureKey::InPass { reference, .. } => Self::extract_tags(reference),
            StructureKey::Loop { reference, .. } => Self::extract_tags(reference),
            StructureKey::LoopParam { reference, .. } => Self::extract_tags(reference),
            StructureKey::NestingLoop { reference, .. } => Self::extract_tags(reference),
            StructureKey::NoiseSpawn { reference, .. } => Self::extract_tags(reference),
            StructureKey::PathSpawn { reference, .. } => Self::extract_tags(reference),
            StructureKey::PathToTag { reference, .. } => Self::extract_tags(reference),
            StructureKey::Reflection { reference, .. } => Self::extract_tags(reference),
            StructureKey::RandDistDir { reference, .. } => Self::extract_tags(reference),
            _ => Vec::new(), // Other variants do not contain a StructureReference
        };

        if tags.len() == 0 {
            None
        } else {
            Some(tags)
        }
    }

    fn extract_tags(reference: &StructureReference) -> Vec<String> {
        match reference {
            StructureReference::Raw { structure, .. } => structure.tags.clone(),
            StructureReference::Ref { structure, .. } => {
                match import_structure(structure.clone()) {
                    Ok(imported_structure) => imported_structure.tags,
                    Err(_) => vec!["Error".to_string()], // Insert "Error" tag if import fails
                }
            }
        }
    }
}

impl StructureKey {
    pub fn dispatch_event(&self, transform: EulerTransform, parent: Option<Entity>, commands: &mut Commands) {
        let event_key = self.clone(); // Clone self to move into the closure

        commands.queue(move |world: &mut World| {
            match event_key {
                StructureKey::Object { .. } => {
                    world.send_event(SceneSpawnEvent {
                        data: event_key,
                        transform,
                        parent
                    });
                }
                StructureKey::PointLight(light) => {
                    world.send_event(PointLightSpawnEvent { light, transform, parent });
                }
                StructureKey::SpotLight(light) => {
                    world.send_event(SpotLightSpawnEvent { light, transform, parent });
                }
                StructureKey::DirectionalLight(light) => {
                    world.send_event(DirectionalLightSpawnEvent { light, transform, parent });
                }
                StructureKey::MainDirectionalLight(light) => {
                    world.send_event(MainDirectionalLightSpawnEvent { light, transform, parent });
                }
                StructureKey::AmbientLight(light) => {
                    world.send_event(AmbientLightSpawnEvent { light, transform, parent });
                }
                StructureKey::DistanceFog(fog) => {
                    world.send_event(DistanceFogSpawnEvent { fog });
                }
                StructureKey::AtmosphereNishita { sun_position, rayleigh_multiplier, mie_multiplier, mie_direction, align_to_main_light } => {
                    world.send_event(AtmosphereNishitaSpawnEvent {
                        sun_position,
                        rayleigh_multiplier,
                        mie_multiplier,
                        mie_direction,
                        align_to_main_light,
                    });
                }
                StructureKey::SoundEffect(file) => {
                    world.send_event(SoundEffectSpawnEvent { file });
                }
                StructureKey::BackgroundMusic(file) => {
                    world.send_event(BackgroundMusicSpawnEvent { file });
                }
                StructureKey::Nest(reference) => {
                    world.send_event(NestSpawnEvent { reference, transform, parent });
                }
                StructureKey::Choose { list } => {
                    world.send_event(ChooseSpawnEvent { list, transform, parent });
                }
                StructureKey::ChooseSome { list, count } => {
                    world.send_event(ChooseSomeSpawnEvent { list, count, transform, parent });
                }
                StructureKey::Rand { reference, rand } => {
                    world.send_event(RandSpawnEvent { reference, rand, transform, parent });
                }
                StructureKey::ProbabilitySpawn { reference, probability } => {
                    world.send_event(ProbabilitySpawnEvent { reference, probability, transform, parent });
                }
                StructureKey::Loop { reference, shift_transform, child_transform, count } => {
                    world.send_event(LoopSpawnEvent {
                        reference,
                        shift_transform,
                        child_transform,
                        count,
                        transform,
                        parent,
                    });
                }
                StructureKey::LoopParam { reference, origin, rotation, distance, child_position, child_rotation, child_scale, count } => {
                    world.send_event(LoopParamSpawnEvent {
                        reference,
                        origin,
                        rotation,
                        distance,
                        child_position,
                        child_rotation,
                        child_scale,
                        count,
                        transform,
                        parent,
                    });
                }
                StructureKey::NestingLoop { reference, repeated_transform, count } => {
                    world.send_event(NestingLoopSpawnEvent {
                        reference,
                        repeated_transform,
                        count,
                        transform,
                        parent
                    });
                }
                StructureKey::NoiseSpawn { reference, fbm, sample_size, count, exclusivity_radius, resolution_modifier } => {
                    world.send_event(NoiseSpawnEvent {
                        reference,
                        fbm,
                        sample_size,
                        count,
                        exclusivity_radius,
                        resolution_modifier,
                        transform,
                        parent,
                    });
                }
                StructureKey::PathSpawn { reference, points, tension, spread, count } => {
                    world.send_event(PathSpawnEvent {
                        reference,
                        points,
                        tension,
                        spread,
                        count,
                        transform,
                        parent,
                    });
                }
                StructureKey::PathToTag { reference, start, manual_points, tag, tension, spread, count, wobble } => {
                    world.send_event(PathToTagSpawnEvent {
                        reference,
                        start,
                        manual_points,
                        tag,
                        tension,
                        spread,
                        count,
                        wobble,
                        transform,
                        parent,
                    });
                }
                StructureKey::Reflection { reference, reflection_plane, reflection_point, reflect_child } => {
                    world.send_event(ReflectionSpawnEvent {
                        reference,
                        reflection_plane,
                        reflection_point,
                        reflect_child,
                        transform,
                        parent,
                    });
                }
                StructureKey::InPass { index, reference } => {
                    world.send_event(InPassSpawnEvent { index, reference, transform, parent });
                }
                StructureKey::RandDistDir { reference, dist_min, dist_max, angle_min_deg, angle_max_deg, y } => {
                    #[cfg(feature = "debug")]
                    println!(
                        "[StructureKey::dispatch_event] Emitting RandDistDir: dist=[{:.2},{:.2}] angle=[{:.1},{:.1}] y={:.2}",
                        dist_min, dist_max, angle_min_deg, angle_max_deg, y
                    );
                    world.send_event(RandDistDirSpawnEvent {
                        reference,
                        dist_min,
                        dist_max,
                        angle_min_deg,
                        angle_max_deg,
                        y,
                        transform,
                        parent,
                    });
                }
                StructureKey::SelectiveReplacement { initial_reference, replacement_reference, tags, replace_count } => {
                    world.send_event(SelectiveReplacementSpawnEvent {
                        initial_reference,
                        replacement_reference,
                        tags,
                        replace_count,
                        transform,
                        parent,
                    });
                }
            }
        });
    }
}