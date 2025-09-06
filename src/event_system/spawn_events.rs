use bevy::prelude::*;
use crate::core::fbm_data::FBMData;
use crate::core::rand_data::RandData;
use crate::core::sample_size::SampleSize;
use crate::core::spread_data::SpreadData;
use crate::core::structure_reference::StructureReference;
use crate::spawning::euler_transform::EulerTransform;
use crate::core::tmaterial::TMaterial;
use crate::core::structure_key::StructureKey;

#[derive(Debug, Clone, Event)]
pub struct MeshSpawnEvent {
    pub mesh: Mesh,
    pub transform: EulerTransform,
    pub material: TMaterial,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct SceneSpawnEvent {
    pub data: StructureKey,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct StructureSpawnEvent {
    pub structure: String,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct PointLightSpawnEvent {
    pub light: PointLight,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct SpotLightSpawnEvent {
    pub light: SpotLight,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct DirectionalLightSpawnEvent {
    pub light: DirectionalLight,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct AmbientLightSpawnEvent {
    pub light: AmbientLight,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct DistanceFogSpawnEvent {
    pub fog: DistanceFog,
}

#[derive(Debug, Clone, Event)]
pub struct SoundEffectSpawnEvent {
    pub file: String,
}

#[derive(Debug, Clone, Event)]
pub struct BackgroundMusicSpawnEvent {
    pub file: String,
}

#[derive(Debug, Clone, Event)]
pub struct NestSpawnEvent {
    pub reference: StructureReference,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct ChooseSpawnEvent {
    pub list: StructureReference,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct ChooseSomeSpawnEvent {
    pub list: StructureReference,
    pub count: usize,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct RandSpawnEvent {
    pub reference: StructureReference,
    pub rand: RandData,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct ProbabilitySpawnEvent {
    pub reference: StructureReference,
    pub probability: f32,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct LoopSpawnEvent {
    pub reference: StructureReference,
    pub shift_transform: EulerTransform,
    pub child_transform: EulerTransform,
    pub count: usize,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct NestingLoopSpawnEvent {
    pub reference: StructureReference,
    pub repeated_transform: EulerTransform,
    pub count: usize,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct NoiseSpawnEvent {
    pub reference: StructureReference,
    pub fbm: FBMData,
    pub sample_size: SampleSize,
    pub count: u32,
    pub exclusivity_radius: f32,
    pub resolution_modifier: f32,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct PathSpawnEvent {
    pub reference: StructureReference,
    pub points: Vec<Vec3>,
    pub tension: f32,
    pub spread: SpreadData,
    pub count: u32,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct ReflectionSpawnEvent {
    pub reference: StructureReference,
    pub reflection_plane: Plane3d,
    pub reflection_point: Vec3,
    pub reflect_child: bool,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct SelectiveReplacementSpawnEvent {
    pub initial_reference: StructureReference,
    pub replacement_reference: StructureReference,
    pub tags: Vec<String>,
    pub replace_count: usize,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct SelectiveReplacementFinalizeEvent {
    pub container: Entity,
    pub replacement_reference: StructureReference,
    pub tags: Vec<String>,
    pub replace_count: usize,
}