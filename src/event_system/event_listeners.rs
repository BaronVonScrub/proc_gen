#[allow(unused_variables)]

use bevy::prelude::*;
use bevy::render::mesh::MeshAabb;
use bevy_rapier3d::prelude::{ActiveCollisionTypes, ActiveEvents, CollisionEvent, Collider, ContactForceEvent, ContactForceEventThreshold, Damping, Dominance, LockedAxes, RigidBody, Sleeping};
use oxidized_navigation::NavMeshAffector;
use crate::event_system::spawn_events::*;
use crate::core::tmaterial::TMaterial;
use crate::serialization::caching::MaterialCache;
use std::path::Path;
use crate::spawning::object_logic::{ObjectType, Pathfinder, PathState, Selectable};
use crate::core::structure_key::StructureKey;
use crate::core::collider::{ColliderBehaviour, ColliderPriority};
use crate::spawning::helpers::*;
use crate::spawning::light_spawning::{spawn_point_light, spawn_spot_light};
use crate::core::components::MainCamera;
use crate::core::structure::Structure;
use crate::core::structure_reference::StructureReference;
use crate::core::components::MainDirectionalLight;
use crate::event_system::spawnables::structure::spawn_structure_data;
use crate::core::tags::Tags;

// Marker to indicate we've already stripped colliders for a GenerationOnlyCollider subtree
#[derive(Component, Default)]
pub struct GenerationOnlyCollidersStripped;
// Tracks stabilization for GenerationOnlyCollider subtrees before stripping
#[derive(Component, Default)]
pub struct GenerationOnlyColliderPending {
    pub last_descendant_count: usize,
    pub last_collider_count: usize,
    pub stable_frames: u8,
}

// A single driver that manages all GenerationState transitions
pub fn generation_state_driver(
    state: Res<State<GenerationState>>,
    mut next: ResMut<NextState<GenerationState>>,
    // For CollisionResolution phase
    mut timer: ResMut<CollisionResolutionTimer>,
    // For Generating readiness gating
    gen_only_pending: Query<Entity, With<GenerationOnlyColliderPending>>,
    selective_pending: Query<Entity, With<SelectiveReplacementPending>>,
    mut stability: ResMut<SpawningStability>,
    generating_frames: Res<GeneratingFrameCounter>,
    activity: Res<SpawnActivity>,
    mut arming: ResMut<GenerationAdvanceArming>,
    // For NavMeshBuilding completion check
    active_tasks: Option<Res<oxidized_navigation::ActiveGenerationTasks>>,
) {
    match *state.get() {
        GenerationState::Generating => {
            // Pending work check updates stability
            let any_pending = !gen_only_pending.is_empty() || !selective_pending.is_empty();
            if any_pending {
                stability.no_pending_stable_frames = 0;
                arming.spawn_done_frames = 0;
                return;
            } else {
                stability.no_pending_stable_frames = stability.no_pending_stable_frames.saturating_add(1);
            }

            // Readiness thresholds
            const MIN_GENERATING_FRAMES: u16 = 30;
            const REQUIRED_STABLE_FRAMES: u8 = 20;
            const REQUIRED_IDLE_FRAMES: u8 = 5;
            let ready = generating_frames.frames_in_generating >= MIN_GENERATING_FRAMES
                && stability.no_pending_stable_frames >= REQUIRED_STABLE_FRAMES
                && activity.idle_frames >= REQUIRED_IDLE_FRAMES;

            if ready {
                // Debounce before advancing
                arming.spawn_done_frames = arming.spawn_done_frames.saturating_add(1);
                const REQUIRED_FRAMES: u8 = 5;
                if arming.spawn_done_frames >= REQUIRED_FRAMES {
                    info!("[GenState] Generating -> CollisionResolution");
                    next.set(GenerationState::CollisionResolution);
                    arming.spawn_done_frames = 0;
                }
            } else {
                arming.spawn_done_frames = 0;
            }
        }
        GenerationState::CollisionResolution => {
            // increment and move on after N frames
            timer.frames = timer.frames.saturating_add(1);
            if timer.frames > 60 {
                info!("[GenState] CollisionResolution -> NavMeshBuilding");
                next.set(GenerationState::NavMeshBuilding);
                timer.frames = 0;
            }
        }
        GenerationState::NavMeshBuilding => {
            // If the resource exists and is empty, we are done. If it doesn't exist, assume done.
            let done = match active_tasks {
                Some(tasks) => tasks.is_empty(),
                None => true,
            };
            if done {
                info!("[GenState] NavMeshBuilding -> Completed");
                next.set(GenerationState::Completed);
            }
        }
        GenerationState::Completed => {}
    }
}

// On entering Generating, reset stability counters/flags
pub fn reset_generating_phase(
    mut stability: ResMut<SpawningStability>,
    mut gen_counter: ResMut<GeneratingFrameCounter>,
    mut activity: ResMut<SpawnActivity>,
) {
    stability.no_pending_stable_frames = 0;
    gen_counter.frames_in_generating = 0;
    activity.idle_frames = 0;
}


// While in Generating, tick the frame counter
pub fn tick_generating_counter(
    state: Option<Res<State<GenerationState>>>,
    mut gen_counter: ResMut<GeneratingFrameCounter>,
) {
    if let Some(gs) = state {
        if gs.get() == &GenerationState::Generating {
            gen_counter.frames_in_generating = gen_counter.frames_in_generating.saturating_add(1);
        }
    }
}

// Increment spawn activity idle counter each frame
pub fn tick_spawn_activity(mut activity: ResMut<SpawnActivity>) {
    activity.idle_frames = activity.idle_frames.saturating_add(1);
}

// --- UI overlay for current GenerationState ---
#[derive(Component)]
pub struct GenStateOverlay;

pub fn spawn_generation_state_overlay(mut commands: Commands) {
    // Root node in top-left corner
    let root = commands
        .spawn_empty()
        .insert(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(8.0),
            ..Default::default()
        })
        .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)))
        .insert(Name::new("GenStateOverlayRoot"))
        .id();

    commands.entity(root).with_children(|parent| {
        parent
            .spawn_empty()
            .insert(Text::new("GenerationState: Initializing"))
            .insert(TextFont { font_size: 16.0, ..Default::default() })
            .insert(TextColor(Color::WHITE))
            .insert(GenStateOverlay);
    });
}

pub fn update_generation_state_overlay(
    state: Option<Res<State<GenerationState>>>,
    active_tasks: Option<Res<oxidized_navigation::ActiveGenerationTasks>>,
    mut q: Query<&mut Text, With<GenStateOverlay>>,
) {
    let mut text = match q.get_single_mut() {
        Ok(t) => t,
        Err(_) => return,
    };
    let gs = state.as_ref().map(|s| s.get()).cloned().unwrap_or(GenerationState::Generating);
    let tasks = active_tasks.map(|t| t.len()).unwrap_or(0);
    text.0 = format!("GenerationState: {:?}\nActiveNavMeshTasks: {}", gs, tasks);
}

// High-level generation/state-of-world progression
#[derive(States, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, Default)]
pub enum GenerationState {
    #[default]
    Generating,
    CollisionResolution,
    NavMeshBuilding,
    Completed,
}

// Simple frame counter used to wait during CollisionResolution
#[derive(Resource, Default)]
pub struct CollisionResolutionTimer {
    pub frames: u16,
}

// Stability counters to avoid premature transition out of Generating
#[derive(Resource, Default)]
pub struct SpawningStability {
    pub no_pending_stable_frames: u8,
}

#[derive(Resource, Default)]
pub struct GeneratingFrameCounter {
    pub frames_in_generating: u16,
}

// Threshold used to decide which colliders should influence the navmesh.
// Any entity with ColliderPriority below this threshold will not receive
// NavMeshAffector at activation time (allowing paths to ignore e.g. trees).
#[derive(Resource, Clone, Copy)]
pub struct NavMeshPriorityThreshold(pub i8);

impl Default for NavMeshPriorityThreshold {
    fn default() -> Self { NavMeshPriorityThreshold(1) }
}

// Debounce arming for transition from Generating -> CollisionResolution
#[derive(Resource, Default)]
pub struct GenerationAdvanceArming {
    pub spawn_done_frames: u8,
}

// Marker used to delay adding NavMeshAffector until we're ready to build the navmesh
#[derive(Component, Default)]
pub struct QueuedNavMeshAffector;

// Tracks whether spawn-related systems have been active recently
#[derive(Resource, Default)]
pub struct SpawnActivity {
    pub idle_frames: u8,
}
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
use bevy::pbr::CascadeShadowConfig;

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

// Recursively collect an entity and all of its descendants
fn collect_entity_and_descendants(entity: Entity, children_query: &Query<&Children>, out: &mut Vec<Entity>) {
    out.push(entity);
    if let Ok(children) = children_query.get(entity) {
        for &child in children.iter() {
            collect_entity_and_descendants(child, children_query, out);
        }
    }
}

// On entering NavMeshBuilding, convert all queued affectors to real NavMeshAffectors
pub fn activate_navmesh_affectors(
    mut commands: Commands,
    queued: Query<(Entity, Option<&ColliderPriority>), With<QueuedNavMeshAffector>>,
    threshold: Option<Res<NavMeshPriorityThreshold>>,
) {
    let thr = threshold.map(|t| t.0).unwrap_or(1);
    for (e, pri_opt) in queued.iter() {
        let include = match pri_opt { Some(ColliderPriority(p)) => *p >= thr, None => true };
        if include {
            commands.entity(e)
                .insert(NavMeshAffector)
                .remove::<QueuedNavMeshAffector>();
        } else {
            // Simply drop the queued flag so this collider does not affect the navmesh
            commands.entity(e).remove::<QueuedNavMeshAffector>();
        }
    }
}

// (SpawningComplete resource and monitor removed; readiness is computed inline in
//  advance_to_collision_resolution using pending queries and stability counters.)

// When generation/deferred tasks are done, move from Generating -> CollisionResolution
pub fn advance_to_collision_resolution(
    state: Res<State<GenerationState>>,
    gen_only_pending: Query<Entity, With<GenerationOnlyColliderPending>>,
    selective_pending: Query<Entity, With<SelectiveReplacementPending>>,
    mut stability: ResMut<SpawningStability>,
    generating_frames: Res<GeneratingFrameCounter>,
    activity: Res<SpawnActivity>,
    mut arming: ResMut<GenerationAdvanceArming>,
    mut next: ResMut<NextState<GenerationState>>,
) {
    if state.get() != &GenerationState::Generating { return; }

    // Pending work check updates stability
    let any_pending = !gen_only_pending.is_empty() || !selective_pending.is_empty();
    if any_pending {
        stability.no_pending_stable_frames = 0;
        arming.spawn_done_frames = 0;
        return;
    } else {
        stability.no_pending_stable_frames = stability.no_pending_stable_frames.saturating_add(1);
    }

    // Readiness thresholds
    const MIN_GENERATING_FRAMES: u16 = 30;
    const REQUIRED_STABLE_FRAMES: u8 = 20;
    const REQUIRED_IDLE_FRAMES: u8 = 5;
    let ready = generating_frames.frames_in_generating >= MIN_GENERATING_FRAMES
        && stability.no_pending_stable_frames >= REQUIRED_STABLE_FRAMES
        && activity.idle_frames >= REQUIRED_IDLE_FRAMES;

    if ready {
        // Debounce before advancing
        arming.spawn_done_frames = arming.spawn_done_frames.saturating_add(1);
        const REQUIRED_FRAMES: u8 = 5;
        if arming.spawn_done_frames >= REQUIRED_FRAMES {
            info!("[GenState] Generating -> CollisionResolution");
            next.set(GenerationState::CollisionResolution);
            arming.spawn_done_frames = 0;
        }
    } else {
        arming.spawn_done_frames = 0;
    }
}

// Wait a small number of frames to allow physics/colliders to settle
pub fn collision_resolution_waiter(
    state: Res<State<GenerationState>>,
    mut timer: ResMut<CollisionResolutionTimer>,
    mut next: ResMut<NextState<GenerationState>>,
) {
    if state.get() != &GenerationState::CollisionResolution { return; }
    // increment and move on after N frames
    timer.frames = timer.frames.saturating_add(1);
    if timer.frames > 60 {
        info!("[GenState] CollisionResolution -> NavMeshBuilding");
        next.set(GenerationState::NavMeshBuilding);
        // reset for potential reuse
        timer.frames = 0;
    }
}

// Monitor navmesh tile generation; when done move to Completed
pub fn navmesh_build_monitor(
    state: Res<State<GenerationState>>,
    mut next: ResMut<NextState<GenerationState>>,
    active_tasks: Option<Res<oxidized_navigation::ActiveGenerationTasks>>,
) {
    if state.get() != &GenerationState::NavMeshBuilding { return; }
    // If the resource exists and is empty, we are done. If it doesn't exist, assume done.
    let done = match active_tasks {
        Some(tasks) => tasks.is_empty(),
        None => true,
    };
    if done {
        info!("[GenState] NavMeshBuilding -> Completed");
        next.set(GenerationState::Completed);
    }
}

// Enqueue pending processing for any root tagged GenerationOnlyCollider
pub fn enqueue_generation_only_colliders(
    mut commands: Commands,
    tagged: Query<(Entity, &Tags), (Without<GenerationOnlyColliderPending>, Without<GenerationOnlyCollidersStripped>)>,
) {
    for (root, tags) in tagged.iter() {
        if tags.contains("GenerationOnlyCollider") {
            commands.entity(root).insert(GenerationOnlyColliderPending::default());
        }
    }
}

// Wait until the subtree stabilizes, then strip colliders under the tagged root
pub fn strip_generation_only_colliders_progressor(
    mut commands: Commands,
    mut pending_q: Query<(Entity, &mut GenerationOnlyColliderPending, Option<&Name>)>,
    children_query: Query<&Children>,
    collider_query: Query<&Collider>,
) {
    for (root, mut pending, name_opt) in pending_q.iter_mut() {
        let mut to_visit = Vec::new();
        collect_entity_and_descendants(root, &children_query, &mut to_visit);
        let descendant_count = to_visit.len();
        let collider_count = to_visit.iter().filter(|&&e| collider_query.get(e).is_ok()).count();

        if descendant_count == pending.last_descendant_count && collider_count == pending.last_collider_count {
            pending.stable_frames = pending.stable_frames.saturating_add(1);
        } else {
            pending.stable_frames = 0;
        }

        pending.last_descendant_count = descendant_count;
        pending.last_collider_count = collider_count;

        if pending.stable_frames >= 3 {
            let mut removed_count = 0usize;
            for e in to_visit.iter().copied() {
                if collider_query.get(e).is_ok() {
                    commands.entity(e).remove::<Collider>();
                    removed_count += 1;
                }
            }

            info!(
                "[GenOnlyCollider] Stripped {} Collider components under entity {:?} ({:?})",
                removed_count, root, name_opt.map(|n| n.as_str().to_string())
            );

            commands.entity(root)
                .insert(GenerationOnlyCollidersStripped)
                .remove::<GenerationOnlyColliderPending>();
        }
    }
}

pub fn mesh_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<MeshSpawnEvent>,
    material_cache: Res<MaterialCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
                .insert(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS)
                .insert(ActiveCollisionTypes::all())
                .insert(QueuedNavMeshAffector)
                .insert(InheritedVisibility::default());

            // Set parent if applicable
            if let Some(parent) = event.parent {
                commands.entity(entity_id).set_parent(parent);
            }
        } else {
            println!("Material not found: {}", material_name);
        }
    }
    if processed { activity.idle_frames = 0; }
}

pub fn scene_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<SceneSpawnEvent>,
    asset_server: Res<AssetServer>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
                        .insert(ColliderPriority(internal_collider.priority))
                        .insert(Damping { linear_damping: 10.0, angular_damping: 0.0 })
                        .insert(LockedAxes::ROTATION_LOCKED | LockedAxes::TRANSLATION_LOCKED_Y)
                        .insert(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS)
                        .insert(ActiveCollisionTypes::all())
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
                            entity_commands.insert(QueuedNavMeshAffector);
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
    if processed { activity.idle_frames = 0; }
}

pub fn point_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<PointLightSpawnEvent>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        let entity = spawn_point_light(
            &mut commands,
            event.light.clone(),
            Transform::from(event.transform.clone()),
        );
        if let Some(parent) = event.parent {
            commands.entity(entity).set_parent(parent);
        }
    }
    if processed { activity.idle_frames = 0; }
}

pub fn spot_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<SpotLightSpawnEvent>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        let entity = spawn_spot_light(
            &mut commands,
            event.light.clone(),
            Transform::from(event.transform.clone()),
        );
        if let Some(parent) = event.parent {
            commands.entity(entity).set_parent(parent);
        }
    }
    if processed { activity.idle_frames = 0; }
}

pub fn directional_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<DirectionalLightSpawnEvent>,
    existing: Query<Entity, With<MainDirectionalLight>>,
    parent_tags: Query<&Tags>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        // Determine if this should be the main world directional light
        let is_main = match event.parent {
            Some(parent) => parent_tags.get(parent).map(|t| t.contains("MainDirectionalLight")).unwrap_or(false),
            None => true,
        };

        // Choose target entity: update existing main if present, else spawn new; non-main always spawns new
        let mut created_new = false;
        let target = if is_main {
            if let Some(e) = existing.iter().next() { e } else { created_new = true; commands.spawn_empty().id() }
        } else {
            created_new = true;
            commands.spawn_empty().id()
        };

        // Common inserts
        let mut ecmd = commands.entity(target);
        ecmd
            .insert(event.light.clone())
            .insert(Transform::from(event.transform.clone()));
        if created_new { ecmd.insert(InheritedVisibility::default()); }

        if is_main {
            // Set up as the main directional light (do not parent under structure container)
            ecmd
                .insert(Name::new("MainDirectionalLight"))
                .insert(CascadeShadowConfig {
                    bounds: vec![0.0, 30.0, 90.0, 270.0],
                    overlap_proportion: 0.2,
                    minimum_distance: 0.0,
                })
                .insert(MainDirectionalLight);
        } else {
            // Regular directional light, parent under provided container
            ecmd.insert(Name::new("DirectionalLight"));
            if let Some(parent) = event.parent { commands.entity(parent).add_child(target); }
        }
    }
    if processed { activity.idle_frames = 0; }
}

pub fn ambient_light_spawn_listener(
    mut reader: EventReader<AmbientLightSpawnEvent>,
    mut ambient: ResMut<AmbientLight>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        *ambient = event.light.clone();
    }
    if processed { activity.idle_frames = 0; }
}

pub fn distance_fog_spawn_listener(
    mut reader: EventReader<DistanceFogSpawnEvent>,
    mut query: Query<&mut DistanceFog, With<MainCamera>>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        for mut fog in &mut query {
            *fog = event.fog.clone();
        }
    }
    if processed { activity.idle_frames = 0; }
}

pub fn sound_effect_spawn_listener(
    mut reader: EventReader<SoundEffectSpawnEvent>,
    sfx: Res<AudioChannel<SoundEffects>>,
    asset_server: Res<AssetServer>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        let handle: Handle<AudioSource> = asset_server.load(event.file.as_str());
        // Play as a one-shot on the SFX channel
        sfx.play(handle);
    }
    if processed { activity.idle_frames = 0; }
}

pub fn background_music_spawn_listener(
    mut reader: EventReader<BackgroundMusicSpawnEvent>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        let handle: Handle<AudioSource> = asset_server.load(event.file.as_str());
        // Stop any currently playing global track and start looping the new one
        audio.stop();
        audio.play(handle).looped();
    }
    if processed { activity.idle_frames = 0; }
}

pub fn nest_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<NestSpawnEvent>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
        if processed { activity.idle_frames = 0; }
}

pub fn choose_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<ChooseSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
    if processed { activity.idle_frames = 0; }
}

// Temporary no-op stubs to satisfy system registrations
pub fn choose_some_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<ChooseSomeSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
    if processed { activity.idle_frames = 0; }
}

pub fn rand_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<RandSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        let jiggled = jiggle_transform(&mut gen_rng, event.rand.clone(), event.transform.clone());
        let reference = event.reference.clone();
        let parent = event.parent;
        commands.queue(move |world: &mut World| {
            world.send_event(NestSpawnEvent { reference, transform: jiggled, parent });
        });
    }
    if processed { activity.idle_frames = 0; }
}

pub fn probability_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<ProbabilitySpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        if gen_rng.rng_mut().gen::<f32>() < event.probability {
            let reference = event.reference.clone();
            let transform = event.transform.clone();
            let parent = event.parent;
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform, parent });
            });
        } // else skip spawn
    }
    if processed { activity.idle_frames = 0; }
}

pub fn loop_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<LoopSpawnEvent>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
    if processed { activity.idle_frames = 0; }
}

pub fn nesting_loop_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<NestingLoopSpawnEvent>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
    if processed { activity.idle_frames = 0; }
}

pub fn noise_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<NoiseSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        // Container for grouping
        // Use a non-scaling container so child meshes are not scaled. Keep translation/rotation, zero out scale.
        let base = event.transform.clone();
        let container_tr = EulerTransform { scale: (1.0, 1.0, 1.0), ..base.clone() };
        let container = commands
            .spawn_empty()
            .insert(Transform::from(container_tr))
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
            // Apply desired radius scaling to local position so container can remain non-scaling.
            // Mapping axes: generator (x, y, z) -> world (X, Z, Y)
            //   - horizontal: X uses x, Z uses y
            //   - vertical: Y uses z (0.0 for 2D noise)
            let local = Vec3::new(
                base.scale.0 * x,
                base.scale.1 * z,
                base.scale.2 * y,
            );

            let euler = EulerTransform {
                translation: (local.x, local.y, local.z),
                rotation: (0.0, 0.0, 0.0),
                scale: (1.0, 1.0, 1.0),
            };

            let reference = event.reference.clone();
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform: euler, parent: Some(container) });
            });
        }
    }
    if processed { activity.idle_frames = 0; }
}

pub fn path_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<PathSpawnEvent>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
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
    if processed { activity.idle_frames = 0; }
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

// Despawn the lower-priority entity whenever two colliders with ColliderPriority make contact.
pub fn collider_priority_despawn_system(
    mut commands: Commands,
    mut contact_events: EventReader<ContactForceEvent>,
    mut collision_events: EventReader<CollisionEvent>,
    priorities: Query<&ColliderPriority>,
    name_query: Query<&Name>,
) {
    // Process contact force events (if any)
    for ev in contact_events.read() {
        let a = ev.collider1;
        let b = ev.collider2;
        if let (Ok(pa), Ok(pb)) = (priorities.get(a), priorities.get(b)) {
            let name_a = name_query.get(a).ok().map(|n| n.as_str().to_string());
            let name_b = name_query.get(b).ok().map(|n| n.as_str().to_string());
            info!(
                "[PriorityDespawn] ContactForce: {:?}({:?})[{}] <-> {:?}({:?})[{}]",
                a, name_a, pa.0, b, name_b, pb.0
            );
            if pa.0 == pb.0 { continue; }
            let loser = if pa.0 < pb.0 { a } else { b };
            let loser_name = name_query.get(loser).ok().map(|n| n.as_str().to_string());
            info!("[PriorityDespawn] Despawning lower priority entity: {:?} ({:?})", loser, loser_name);
            commands.entity(loser).despawn_recursive();
        }
    }

    // Also process basic collision start events (more reliable for kinematic bodies)
    for ev in collision_events.read() {
        if let CollisionEvent::Started(a, b, _) = ev {
            if let (Ok(pa), Ok(pb)) = (priorities.get(*a), priorities.get(*b)) {
                let name_a = name_query.get(*a).ok().map(|n| n.as_str().to_string());
                let name_b = name_query.get(*b).ok().map(|n| n.as_str().to_string());
                info!(
                    "[PriorityDespawn] CollisionStart: {:?}({:?})[{}] <-> {:?}({:?})[{}]",
                    a, name_a, pa.0, b, name_b, pb.0
                );
                if pa.0 == pb.0 { continue; }
                let loser = if pa.0 < pb.0 { *a } else { *b };
                let loser_name = name_query.get(loser).ok().map(|n| n.as_str().to_string());
                info!("[PriorityDespawn] Despawning lower priority entity: {:?} ({:?})", loser, loser_name);
                commands.entity(loser).despawn_recursive();
            }
        }
    }
}
