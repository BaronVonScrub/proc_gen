use bevy::prelude::*;
use crate::core::fbm_data::FBMData;
use crate::core::rand_data::RandData;
use crate::core::sample_size::SampleSize;
use crate::core::spread_data::SpreadData;
use crate::core::structure_reference::StructureReference;
use crate::spawning::euler_transform::EulerTransform;
use crate::core::tmaterial::TMaterial;
use crate::core::wobble::WobbleParams;
use crate::core::structure_key::StructureKey;

#[derive(Debug, Clone, Event)]
pub struct MeshSpawnEvent {
    pub mesh: Mesh,
    pub transform: EulerTransform,
    pub material: TMaterial,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct PathWorldPointsEvent {
    pub points: Vec<Vec3>,
}

#[derive(Debug, Clone, Event)]
pub struct RandDistDirSpawnEvent {
    pub reference: StructureReference,
    pub dist_min: f32,
    pub dist_max: f32,
    pub angle_min_deg: f32,
    pub angle_max_deg: f32,
    pub y: f32,
    pub transform: EulerTransform,
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
pub struct MainDirectionalLightSpawnEvent {
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
pub struct AtmosphereNishitaSpawnEvent {
    pub sun_position: Vec3,
    pub rayleigh_multiplier: Vec3,
    pub mie_multiplier: f32,
    pub mie_direction: f32,
    pub align_to_main_light: bool,
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
pub struct LoopParamSpawnEvent {
    pub reference: StructureReference,
    // Per-index placement parameters
    pub origin: Vec3,        // relative to the loop container (parent)
    pub rotation: Vec3,      // degrees per index (XYZ applied as EulerRot::XYZ)
    pub distance: f32,       // distance from origin along +X rotated by rotation*index
    // Per-index child modifiers (applied index times)
    pub child_position: Vec3, // additive per index
    pub child_rotation: Vec3, // additive degrees per index
    pub child_scale: Vec3,    // multiplicative factor per index (component-wise)
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
pub struct PathToTagSpawnEvent {
    pub reference: StructureReference,
    pub start: Vec3,
    pub manual_points: Option<Vec<Vec3>>,
    pub tag: String,
    pub tension: f32,
    pub spread: SpreadData,
    pub count: u32,
    pub wobble: Option<WobbleParams>,
    // Optional: if provided, the computed world polyline will be stored on an entity
    // carrying this label in its Tags. If such an entity doesn't exist, one will be created.
    pub store_as: Option<String>,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct PathToAllTagsSpawnEvent {
    pub reference: StructureReference,
    pub start: Vec3,
    pub manual_points: Option<Vec<Vec3>>,
    pub tag: String,
    pub tension: f32,
    pub spread: SpreadData,
    pub count: u32,
    pub wobble: Option<WobbleParams>,
    // Optional: same behavior as PathToTagSpawnEvent
    pub store_as: Option<String>,
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

#[derive(Debug, Clone, Event)]
pub struct InPassSpawnEvent {
    pub index: u8,
    pub reference: StructureReference,
    pub transform: EulerTransform,
    pub parent: Option<Entity>,
}