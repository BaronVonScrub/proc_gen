#[allow(unused_variables)]

use bevy::prelude::*;
#[cfg(feature = "atmosphere")]
use bevy_atmosphere::prelude::{AtmosphereModel, Nishita};
use bevy::render::mesh::MeshAabb;
use bevy_rapier3d::prelude::{ActiveCollisionTypes, ActiveEvents, CollisionEvent, Collider, ContactForceEvent, ContactForceEventThreshold, Damping, Dominance, LockedAxes, RigidBody, Sleeping};
use oxidized_navigation::NavMeshAffector;
use crate::event_system::spawn_events::*;
use crate::core::tmaterial::TMaterial;
use crate::serialization::caching::MaterialCache;
use std::path::Path;
use std::collections::HashSet;
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
use crate::core::components::{PathPolyline, PathPolylineList};
use bevy::math::EulerRot;
use crate::materials::path_blend::{GroundPathMaterial, PathBlendMaterial, PathBlendParams, make_path_blend_material};
use bevy_pbr::StandardMaterial;

// In non-debug builds, silence generation logging in this module by shadowing println!/info!
// This keeps verbose generation output behind the `debug` feature flag without altering call sites.
#[cfg(not(feature = "debug"))]
macro_rules! println { ($($arg:tt)*) => {}; }

// Buffer for PathSpawnEvent produced during PathResolve; these will be flushed in the next Generating
#[derive(Resource, Default)]
pub struct ResolvedPathSpawns(pub Vec<PathSpawnEvent>);

// Apply all stored polylines from any PathPolylineList entity to any PathBlend material
// whose entity shares at least one matching tag. Each list is applied independently,
// and segments are built per-polyline without connecting across polylines.
pub fn apply_stored_polylines_to_tagged_pathblend(
    lists: Query<(&Tags, &PathPolylineList)>,
    targets: Query<(&Tags, &MeshMaterial3d<PathBlendMaterial>)>,
    mut mats: ResMut<Assets<PathBlendMaterial>>,
) {
    let mut total_targets = 0usize;
    let mut total_polylines = 0usize;
    for (list_tags, list) in &lists {
        // Count polylines in this list
        let list_poly_count = list.0.len();
        if list_poly_count == 0 { continue; }
        // For each target, apply if any tag matches
        for (t_tags, mat_handle) in &targets {
            let mut matches = false;
            'outer: for lt in &list_tags.0 {
                for tt in &t_tags.0 { if lt == tt { matches = true; break 'outer; } }
            }
            if !matches { continue; }
            if let Some(mat) = mats.get_mut(&mat_handle.0) {
                mat.extension.params.set_segments_from_polylines(&list.0);
                total_targets += 1;
                total_polylines += list_poly_count;
                eprintln!(
                    "[PathBlend] Applied {} stored polylines to tagged target; total segments now {}",
                    list_poly_count,
                    mat.extension.params.flags.z
                );
            } else {
                eprintln!("[PathBlend] Target PathBlendMaterial handle not found while applying stored polylines");
            }
        }
    }
    if total_targets > 0 {
        eprintln!("[PathBlend] Applied stored polylines to {} target(s) ({} polylines)", total_targets, total_polylines);
    }
}

// Apply published world-space path points into the ground PathBlend material
pub fn apply_world_path_points_to_material(
    mut reader: EventReader<PathWorldPointsEvent>,
    mut mats: ResMut<Assets<PathBlendMaterial>>,
    target: Option<Res<GroundPathMaterial>>,
) {
    let Some(target) = target else {
        eprintln!("[PathBlend] GroundPathMaterial resource not yet available; skipping PathWorldPointsEvent(s)");
        return;
    };
    let Some(handle) = &target.0 else {
        eprintln!("[PathBlend] GroundPathMaterial handle not set yet; skipping PathWorldPointsEvent(s)");
        return;
    };
    if let Some(mat) = mats.get_mut(handle) {
        // Collect each event's polyline and then set all segments at once, without connecting between polylines
        let mut polylines: Vec<Vec<Vec3>> = Vec::new();
        let mut total_points = 0usize;
        let mut events = 0usize;
        for ev in reader.read() {
            eprintln!("[PathBlend] Applying PathWorldPointsEvent with {} points:", ev.points.len());
            for (i, p) in ev.points.iter().enumerate() {
                eprintln!("  [World][{}] {:?}", i, p);
            }
            if ev.points.len() >= 2 { polylines.push(ev.points.clone()); }
            total_points += ev.points.len();
            events += 1;
        }
        if !polylines.is_empty() {
            mat.extension.params.set_segments_from_polylines(&polylines);
            eprintln!(
                "[PathBlend] Applied {} PathWorldPointsEvent(s) ({} polylines, {} points); total segments now {}",
                events,
                polylines.len(),
                total_points,
                mat.extension.params.flags.z
            );
        }
    } else {
        eprintln!("[PathBlend] PathBlendMaterial handle not found in asset storage");
    }
}

// Bring in the Bevy log `info!` macro when debug feature is enabled,
// otherwise define a no-op so the calls compile away in release.
#[cfg(feature = "debug")]
use bevy::log::info;
#[cfg(not(feature = "debug"))]
macro_rules! info { ($($arg:tt)*) => {}; }

// When present (and true), the sun_position of the Atmosphere Nishita model is aligned each frame to the
// authored MainDirectionalLight orientation. This resource is authored via .arch through the AtmosphereNishita key.
#[derive(Resource, Default)]
pub struct AtmosphereAlignToMainLight(pub bool);

// Set the global AtmosphereModel Nishita parameters from authored data (only when 'atmosphere' feature is enabled)
#[cfg(feature = "atmosphere")]
pub fn atmosphere_nishita_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<AtmosphereNishitaSpawnEvent>,
) {
    for ev in reader.read() {
        let mut n = Nishita::default();
        let sp = ev.sun_position;
        let len2 = sp.length_squared();
        n.sun_position = if len2 > 0.0 { sp / len2.sqrt() } else { Vec3::new(0.0, 1.0, 0.0) };
        // Multiply defaults by provided multipliers
        let d = Nishita::default();
        n.rayleigh_coefficient = Vec3::new(
            d.rayleigh_coefficient.x * ev.rayleigh_multiplier.x,
            d.rayleigh_coefficient.y * ev.rayleigh_multiplier.y,
            d.rayleigh_coefficient.z * ev.rayleigh_multiplier.z,
        );
        n.mie_coefficient = d.mie_coefficient * ev.mie_multiplier;
        n.mie_direction = ev.mie_direction;
        commands.insert_resource(AtmosphereModel::new(n));
        // Also set (or clear) the alignment preference resource per authored value
        commands.insert_resource(AtmosphereAlignToMainLight(ev.align_to_main_light));
    }
}

pub fn loop_param_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<LoopParamSpawnEvent>,
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
            .insert(Name::new("LoopParam"))
            .id();

        if let Some(parent) = event.parent {
            commands.entity(container).set_parent(parent);
        }

        // Precompute base vectors
        let origin = event.origin;
        let distance = event.distance;

        for i in 0..event.count {
            let fi = i as f32;
            // Rotation per index (degrees)
            let rot_deg = event.rotation * fi;
            let rot = Quat::from_euler(
                EulerRot::XYZ,
                rot_deg.x.to_radians(),
                rot_deg.y.to_radians(),
                rot_deg.z.to_radians(),
            );
            // Direction is +X rotated by rot
            let dir = rot * Vec3::X;
            let base_pos = origin + dir * distance;

            // Child modifiers applied index times
            let child_pos = event.child_position * fi; // additive per index
            let child_rot = event.child_rotation * fi; // additive degrees per index
            // Scale: exponential per-index modifier: (1 + m)^i so m=0 => 1^i = 1 (neutral)
            let child_scale = Vec3::new(
                (1.0 + event.child_scale.x).powf(fi),
                (1.0 + event.child_scale.y).powf(fi),
                (1.0 + event.child_scale.z).powf(fi),
            );

            let euler = EulerTransform {
                translation: (base_pos.x + child_pos.x, base_pos.y + child_pos.y, base_pos.z + child_pos.z),
                rotation: (child_rot.x, child_rot.y, child_rot.z),
                scale: (child_scale.x, child_scale.y, child_scale.z),
            };

            let reference = event.reference.clone();
            commands.queue(move |world: &mut World| {
                world.send_event(NestSpawnEvent { reference, transform: euler, parent: Some(container) });
            });
        }
    }
    if processed { activity.idle_frames = 0; }
}

// On entering Generating, flush any PathSpawnEvent captured during PathResolve
pub fn flush_resolved_paths_on_enter_generating(
    mut resolved: ResMut<ResolvedPathSpawns>,
    mut commands: Commands,
) {
    if resolved.0.is_empty() { return; }
    println!("[PathBuffer] Flushing {} buffered PathSpawnEvent(s) to Generating", resolved.0.len());
    let to_send = std::mem::take(&mut resolved.0);
    commands.queue(move |world: &mut World| {
        for ev in to_send.into_iter() {
            println!("[PathBuffer] -> emit PathSpawnEvent: points={}, parent={:?}", ev.points.len(), ev.parent);
            world.send_event(ev);
        }
    });
}

// Variant: path to ALL entities that have the given tag, emitting one PathSpawnEvent per target
pub fn path_to_all_tags_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<PathToAllTagsSpawnEvent>,
    tag_query: Query<(&GlobalTransform, &Tags)>,
    nav_mesh: Option<Res<oxidized_navigation::NavMesh>>,
    settings: Option<Res<oxidized_navigation::NavMeshSettings>>,
    active_tasks: Option<Res<oxidized_navigation::ActiveGenerationTasks>>,
    #[cfg(feature = "debug")] mut all_dbg: ResMut<AllPathsDebug>,
    parent_query: Query<&GlobalTransform>,
    mut activity: ResMut<SpawnActivity>,
    store_query: Query<(Entity, &Tags)>,
    mut resolved: ResMut<ResolvedPathSpawns>,
    mut highest: ResMut<HighestPassIndex>,
    cur_pass: Option<Res<CurrentPass>>,
) {
    let mut processed = false;
    for event in reader.read() {
        // If navmesh is still baking, defer deterministically
        if let Some(tasks) = active_tasks.as_ref() {
            if !tasks.is_empty() {
                let ev = event.clone();
                commands.queue(move |world: &mut World| { world.send_event(ev); });
                continue;
            }
        }
        // Compute world-space base transform for this event
        let local_tf = Transform::from(event.transform.clone());
        let world_tf = match event.parent.and_then(|p| parent_query.get(p).ok()) {
            Some(parent_gt) => parent_gt.compute_transform() * local_tf,
            None => local_tf,
        };
        let base = world_tf.translation;

        // Eagerly ensure a storage holder exists if store_as was provided, even before paths resolve
        if let Some(label) = event.store_as.as_ref() {
            let existing = store_query.iter().find(|(_, t)| t.contains(label)).map(|(e, _)| e);
            if existing.is_none() {
                let e = commands
                    .spawn_empty()
                    .insert(PathPolylineList::default())
                    .insert(Tags(vec![label.clone()]))
                    .insert(Name::new(format!("PathPolylineList: {}", label)))
                    .id();
                if let Some(parent) = event.parent { commands.entity(e).set_parent(parent); }
                eprintln!("[PathStore] Created holder 'PathPolylineList: {}' under parent {:?}", label, event.parent);
            }
        }

        // Collect all end positions with the requested tag
        let mut targets: Vec<Vec3> = Vec::new();
        for (gt, tags) in tag_query.iter() {
            if tags.0.iter().any(|t| t == &event.tag) { targets.push(gt.translation()); }
        }
        if targets.is_empty() {
            println!("[PathToAllTags] No entities found with tag '{}' yet; will retry next frame", event.tag);
            let ev = event.clone();
            commands.queue(move |world: &mut World| { world.send_event(ev); });
            continue;
        }

        let (Some(nav_mesh), Some(settings)) = (nav_mesh.as_ref(), settings.as_ref()) else {
            println!("[PathToAllTags] NavMesh/Settings not available yet; will retry next frame");
            let ev = event.clone();
            commands.queue(move |world: &mut World| { world.send_event(ev); });
            continue;
        };

        // For each target, build a path and enqueue a PathSpawnEvent
        if let Ok(tiles) = nav_mesh.get().read() {
            // Helper that captures tiles/settings without naming the private NavMeshTiles type
            let try_between = |s: Vec3, e: Vec3| -> Option<Vec<Vec3>> {
                match oxidized_navigation::query::find_path(&tiles, &settings, s, e, None, None) {
                    Ok(points) if points.len() >= 2 => Some(points),
                    _ => None,
                }
            };
            for end_pos_raw in targets {
                // Rotate local start by world rotation; snap Y to end plane
                let mut start_world = base + world_tf.rotation * event.start;
                start_world.y = end_pos_raw.y + 0.05;
                let end_pos = end_pos_raw;

                // Build path points (optionally with manual checkpoints)
                let mut path_points: Vec<Vec3> = Vec::new();
                if let Some(local_points) = &event.manual_points {
                    let mut world_manual: Vec<Vec3> = Vec::with_capacity(local_points.len());
                    for lp in local_points.iter() {
                        let mut wp = base + world_tf.rotation * *lp;
                        wp.y = end_pos.y + 0.05;
                        world_manual.push(wp);
                    }
                    let nav_start = world_manual.last().copied().unwrap_or(start_world);
                    let base_path = match try_between(nav_start, end_pos) {
                        Some(p) => p,
                        None => {
                            let radii = [0.15f32, 0.3, 0.45, 0.6, 0.8, 1.0];
                            let mut found: Option<Vec<Vec3>> = None;
                            'outer_bp: for r in radii.into_iter() {
                                let steps = 16;
                                for i in 0..steps {
                                    let theta = (i as f32) * std::f32::consts::TAU / (steps as f32);
                                    let cand = Vec3::new(nav_start.x + r * theta.cos(), nav_start.y, nav_start.z + r * theta.sin());
                                    if let Some(p) = try_between(cand, end_pos) { found = Some(p); break 'outer_bp; }
                                }
                            }
                            match found { Some(p) => p, None => continue } // skip this target this frame
                        }
                    };
                    path_points.clear();
                    path_points.push(start_world);
                    path_points.extend(world_manual.into_iter());
                    if base_path.len() > 1 { path_points.extend_from_slice(&base_path[1..]); }
                } else {
                    if let Some(points) = try_between(start_world, end_pos) {
                        path_points = points;
                    } else {
                        // Probe around start
                        let radii = [0.15f32, 0.3, 0.45, 0.6, 0.8, 1.0];
                        let mut found = None;
                        'outer: for r in radii.into_iter() {
                            let steps = 16;
                            for i in 0..steps {
                                let theta = (i as f32) * std::f32::consts::TAU / (steps as f32);
                                let cand = Vec3::new(start_world.x + r * theta.cos(), start_world.y, start_world.z + r * theta.sin());
                                if let Some(points) = try_between(cand, end_pos) { found = Some(points); break 'outer; }
                            }
                        }
                        match found { Some(points) => { path_points = points; }, None => continue }
                    }
                }

                // Log all computed world-space points before further processing/storage
        if !path_points.is_empty() {
            println!("[PathToTag] World polyline ({} points):", path_points.len());
            for (i, p) in path_points.iter().enumerate() {
                println!("  [World][{}] {:?}", i, p);
            }
        }
        if path_points.len() < 2 { continue; }

                // Optional wobble (same approach as single-target variant)
                let wobble_prefix_len: usize = match &event.manual_points { Some(lps) => lps.len(), None => 0 };
                if let Some(wob) = event.wobble.as_ref() {
                    if path_points.len() >= 2 && wob.checkpoint_spacing > 0.01 && wob.wavelength > 0.01 {
                        // Precompute segment lengths and cumulative arclengths
                        let mut seg_lengths: Vec<f32> = Vec::with_capacity(path_points.len() - 1);
                        let mut cum: Vec<f32> = Vec::with_capacity(path_points.len());
                        cum.push(0.0);
                        for w in path_points.windows(2) {
                            let l = w[1].distance(w[0]);
                            seg_lengths.push(l);
                            cum.push(cum.last().copied().unwrap_or(0.0) + l);
                        }
                        let total_len = *cum.last().unwrap_or(&0.0);
                        let s_start = if wobble_prefix_len < cum.len() { cum[wobble_prefix_len] } else { 0.0 };
                        if wobble_prefix_len < path_points.len() {
                            let sample_at = |s: f32| -> (Vec3, Vec3) {
                                let mut s_rem = s.clamp(0.0, total_len);
                                for (i, &l) in seg_lengths.iter().enumerate() {
                                    if l <= 1e-5 { continue; }
                                    if s_rem <= l {
                                        let t = s_rem / l;
                                        let p0 = path_points[i];
                                        let p1 = path_points[i+1];
                                        let pos = p0.lerp(p1, t);
                                        let mut tan = p1 - p0; tan.y = 0.0; let tan = tan.normalize_or_zero();
                                        return (pos, if tan.length_squared() < 1.0e-8 { Vec3::X } else { tan });
                                    }
                                    s_rem -= l;
                                }
                                let last = path_points.last().copied().unwrap();
                                let prev = path_points[path_points.len()-2];
                                let mut tan = last - prev; tan.y = 0.0; let tan = tan.normalize_or_zero();
                                (last, if tan.length_squared() < 1.0e-8 { Vec3::X } else { tan })
                            };

                            let mut amp = wob.amplitude;
                            for _attempt in 0..3 {
                                let mut checkpoints: Vec<Vec3> = Vec::new();
                                checkpoints.push(path_points[wobble_prefix_len]);
                                let mut s = (s_start + wob.checkpoint_spacing).min(total_len);
                                let mut made_offset = false;
                                while s < total_len - 1.0e-3 {
                                    let (base_p, tan) = sample_at(s);
                                    let side = Vec3::Y.cross(tan).normalize_or_zero();
                                    let offset = amp * (std::f32::consts::TAU * s / wob.wavelength + wob.phase).sin();
                                    let mut cp = base_p + side * offset; cp.y = base_p.y; checkpoints.push(cp);
                                    made_offset = true; s += wob.checkpoint_spacing;
                                }
                                if !made_offset {
                                    let mid = (s_start + total_len) * 0.5;
                                    if mid > s_start + 1e-4 && mid < total_len - 1e-4 {
                                        let (base_p, tan) = sample_at(mid);
                                        let side = Vec3::Y.cross(tan).normalize_or_zero();
                                        let offset = amp * (std::f32::consts::TAU * mid / wob.wavelength + wob.phase).sin();
                                        let mut cp = base_p + side * offset; cp.y = base_p.y; checkpoints.push(cp);
                                    }
                                }
                                checkpoints.push(path_points.last().copied().unwrap());

                                let mut new_path: Vec<Vec3> = Vec::new();
                                if wobble_prefix_len > 0 { new_path.extend_from_slice(&path_points[..=wobble_prefix_len]); }
                                else { new_path.push(path_points[0]); }

                                let mut ok = true;
                                for w in checkpoints.windows(2) {
                                    match oxidized_navigation::query::find_path(&tiles, &settings, w[0], w[1], None, None) {
                                        Ok(sub) if sub.len() >= 2 => { new_path.extend_from_slice(&sub[1..]); }
                                        _ => { ok = false; break; }
                                    }
                                }
                                if ok && new_path.len() >= 2 { path_points = new_path; break; }
                                else { amp *= 0.5; }
                            }
                        }
                    }
                }

                // Debug store
                #[cfg(feature = "debug")] {
                    all_dbg.paths.push(path_points.clone());
                    if all_dbg.paths.len() > 256 { all_dbg.paths.remove(0); }
                }

                // Convert to local points for the container and enqueue PathSpawnEvent
                let inv_world = world_tf.compute_matrix().inverse();
                let local_points: Vec<Vec3> = path_points.iter().map(|p| inv_world.transform_point3(*p)).collect();

                let reference = event.reference.clone();
                let tension = event.tension;
                let spread = event.spread.clone();
                let count = event.count;
                let transform = event.transform.clone();
                let parent = event.parent;
                let points_len = local_points.len();
                resolved.0.push(PathSpawnEvent { reference, points: local_points, tension, spread, count, transform, parent });
                println!("[PathBuffer] Buffered PathSpawnEvent (to_all): points={}, parent={:?}", points_len, parent);
                if let Some(cp) = cur_pass.as_ref() { if highest.0 < cp.0.saturating_add(1) { highest.0 = cp.0.saturating_add(1); } }
            }
        }

        processed = true;
    }
    if processed { activity.idle_frames = 0; }
}
#[cfg(not(feature = "debug"))]
macro_rules! info { ($($arg:tt)*) => {}; }

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

// Persistent queue for deferred InPass items
#[derive(Resource, Default)]
pub struct PendingInPass(pub Vec<InPassSpawnEvent>);

// Collect InPass events into a persistent queue and update highest pass index
pub fn in_pass_spawn_listener(
    mut reader: EventReader<InPassSpawnEvent>,
    mut highest: ResMut<HighestPassIndex>,
    mut pending: ResMut<PendingInPass>,
) {
    for event in reader.read() {
        if event.index > highest.0 { highest.0 = event.index; }
        println!(
            "[InPass] queued index={} parent={:?} (highest now {})",
            event.index, event.parent, highest.0
        );
        pending.0.push(event.clone());
    }
}

// Drain and execute only the items for the current pass
pub fn process_pending_inpass(
    mut commands: Commands,
    mut pending: ResMut<PendingInPass>,
    cur: Res<CurrentPass>,
    mut activity: ResMut<SpawnActivity>,
) {
    if pending.0.is_empty() { return; }
    let mut rest: Vec<InPassSpawnEvent> = Vec::new();
    let mut any_spawned = false;
    for ev in pending.0.drain(..) {
        if ev.index == cur.0 {
            any_spawned = true;
            // Derive a label for logging
            #[cfg(feature = "debug")]
            {
                let label = match &ev.reference {
                    StructureReference::Raw { structure, .. } => structure.structure_name.clone(),
                    StructureReference::Ref { structure, .. } => structure.clone(),
                };
                println!(
                    "[InPass] spawning index={} structure='{}' parent={:?}",
                    ev.index, label, ev.parent
                );
            }
            match Structure::try_from(&ev.reference) {
                Ok(structure) => {
                    let _ = spawn_structure_data(
                        &mut commands,
                        &structure,
                        Transform::from(ev.transform.clone()),
                        ev.parent,
                    );
                }
                Err(e) => {
                    eprintln!("[InPass] Import error: {:?}", e);
                }
            }
        } else {
            rest.push(ev);
        }
    }
    pending.0 = rest;
    if any_spawned { activity.idle_frames = 0; }
}

#[derive(Resource, Default)]
pub struct AllPathsDebug {
    pub paths: Vec<Vec<Vec3>>, // list of polylines
}

// --- Generation pass tracking ---
#[derive(Resource, Default, Clone, Copy)]
pub struct CurrentPass(pub u8);

// Highest pass index seen in authored data so far. Total passes = HighestPassIndex + 1
#[derive(Resource, Default, Clone, Copy)]
pub struct HighestPassIndex(pub u8);

pub fn rand_dist_dir_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<RandDistDirSpawnEvent>,
    mut gen_rng: ResMut<GenRng>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        // Sample angle in degrees and distance uniformly
        let angle_deg = gen_rng.rng_mut().gen_range(event.angle_min_deg..=event.angle_max_deg);
        let angle_rad = angle_deg.to_radians();
        let dist = gen_rng.rng_mut().gen_range(event.dist_min..=event.dist_max);
        let offset = Vec3::new(angle_rad.cos() * dist, event.y, angle_rad.sin() * dist);
        println!(
            "[RandDistDir] angle_deg={:.2}, dist={:.2}, offset={:?}",
            angle_deg, dist, offset
        );

        // Apply offset relative to the provided base transform
        let mut euler = event.transform.clone();
        euler.translation = (
            euler.translation.0 + offset.x,
            euler.translation.1 + offset.y,
            euler.translation.2 + offset.z,
        );

        let reference = event.reference.clone();
        let parent = event.parent;
        commands.queue(move |world: &mut World| {
            // Create a container for applying the base transform, then nest the offset child under it
            // by reusing the Nest path: the child's local euler handles the offset.
            world.send_event(NestSpawnEvent { reference, transform: euler, parent });
        });
    }
    if processed { activity.idle_frames = 0; }
}

#[cfg(feature = "debug")]
pub fn draw_all_paths_debug(mut gizmos: Gizmos, dbg: Res<AllPathsDebug>) {
    if dbg.paths.is_empty() { return; }
    for poly in dbg.paths.iter() {
        if poly.len() < 2 { continue; }
        for w in poly.windows(2) {
            let a = w[0];
            let b = w[1];
            gizmos.line(a, b, Color::srgba(1.0, 0.3, 0.1, 1.0));
        }
    }
}

pub fn path_to_tag_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<PathToTagSpawnEvent>,
    tag_query: Query<(&GlobalTransform, &Tags)>,
    nav_mesh: Option<Res<oxidized_navigation::NavMesh>>,
    settings: Option<Res<oxidized_navigation::NavMeshSettings>>,
    active_tasks: Option<Res<oxidized_navigation::ActiveGenerationTasks>>,
    #[cfg(feature = "debug")] mut all_dbg: ResMut<AllPathsDebug>,
    parent_query: Query<&GlobalTransform>,
    mut activity: ResMut<SpawnActivity>,
    store_query: Query<(Entity, &Tags)>,
    mut resolved: ResMut<ResolvedPathSpawns>,
    mut highest: ResMut<HighestPassIndex>,
    cur_pass: Option<Res<CurrentPass>>,
) {
    let mut processed = false;
    for event in reader.read() {
        // If navmesh is still baking, defer deterministically
        if let Some(tasks) = active_tasks.as_ref() {
            if !tasks.is_empty() {
                let ev = event.clone();
                commands.queue(move |world: &mut World| { world.send_event(ev); });
                continue;
            }
        }
        // Only mark processed when we actually succeed in producing a path
        // Compute world-space base transform for this event: parent GlobalTransform * local Transform
        let local_tf = Transform::from(event.transform.clone());
        let world_tf = match event.parent.and_then(|p| parent_query.get(p).ok()) {
            Some(parent_gt) => parent_gt.compute_transform() * local_tf,
            None => local_tf,
        };
        let base = world_tf.translation;

        // Eagerly ensure a storage holder exists if store_as was provided, even before a path resolves
        if let Some(label) = event.store_as.as_ref() {
            let existing = store_query.iter().find(|(_, t)| t.contains(label)).map(|(e, _)| e);
            if existing.is_none() {
                let e = commands
                    .spawn_empty()
                    .insert(PathPolylineList::default())
                    .insert(Tags(vec![label.clone()]))
                    .insert(Name::new(format!("PathPolylineList: {}", label)))
                    .id();
                if let Some(parent) = event.parent { commands.entity(e).set_parent(parent); }
            }
        }

        // Find nearest entity with the requested tag
        let mut best: Option<(Vec3, f32)> = None; // (position, dist2)
        for (gt, tags) in tag_query.iter() {
            if tags.0.iter().any(|t| t == &event.tag) {
                let pos = gt.translation();
                // Compare distances in world space; rotate local start by world rotation
                let mut start_world = base + world_tf.rotation * event.start;
                // Align Y to candidate's Y to improve polygon matching (door base may be elevated)
                start_world.y = pos.y;
                let d2 = pos.distance_squared(start_world);
                if best.map(|(_, bd2)| d2 < bd2).unwrap_or(true) {
                    best = Some((pos, d2));
                }
            }
        }

        let Some((end_pos, _)) = best else {
            println!("[PathToTag] No entity found with tag '{}' yet; will retry next frame", event.tag);
            let ev = event.clone();
            commands.queue(move |world: &mut World| { world.send_event(ev); });
            continue;
        };

        let (Some(nav_mesh), Some(settings)) = (nav_mesh.as_ref(), settings.as_ref()) else {
            println!("[PathToTag] NavMesh/Settings not available yet; will retry next frame");
            let ev = event.clone();
            commands.queue(move |world: &mut World| { world.send_event(ev); });
            continue;
        };

        // Compute path using oxidized_navigation (deterministic). If the initial start is not on the
        // navmesh, probe a few nearby offsets on the horizontal plane to find a valid start polygon.
        let mut path_points: Vec<Vec3> = Vec::new();
        if let Ok(tiles) = nav_mesh.get().read() {
            // Base start position rotated into world, then snapped near end's Y plane
            let mut start_world = base + world_tf.rotation * event.start;
            start_world.y = end_pos.y + 0.05;

            // Helper to compute a path between arbitrary points
            let try_between = |s: Vec3, e: Vec3| -> Option<Vec<Vec3>> {
                match oxidized_navigation::query::find_path(&tiles, &settings, s, e, None, None) {
                    Ok(points) if points.len() >= 2 => Some(points),
                    _ => None,
                }
            };

            // Helper that also probes around the start point if direct path fails
            let _try_between_with_probe = |s: Vec3, e: Vec3| -> Option<Vec<Vec3>> {
                if let Some(p) = try_between(s, e) { return Some(p); }
                let radii = [0.15f32, 0.3, 0.45, 0.6, 0.8, 1.0];
                for r in radii.into_iter() {
                    let steps = 16;
                    for i in 0..steps {
                        let theta = (i as f32) * std::f32::consts::TAU / (steps as f32);
                        let cand = Vec3::new(s.x + r * theta.cos(), s.y, s.z + r * theta.sin());
                        if let Some(p) = try_between(cand, e) { return Some(p); }
                    }
                }
                None
            };

            // If author provided manual checkpoints, PREPEND them and then navmesh from the FINAL manual point -> end.
            if let Some(local_points) = &event.manual_points {
                // 1) Transform manual points to world and snap Y near navmesh plane
                let mut world_manual: Vec<Vec3> = Vec::with_capacity(local_points.len());
                for lp in local_points.iter() {
                    let mut wp = base + world_tf.rotation * *lp;
                    wp.y = end_pos.y + 0.05;
                    world_manual.push(wp);
                }

                // 2) Determine navmesh start: last manual point (or start_world if list empty)
                let nav_start = world_manual.last().copied().unwrap_or(start_world);

                // 3) Compute base path from nav_start to end_pos (with probe fallback around nav_start)
                let base_path = match try_between(nav_start, end_pos) {
                    Some(p) => p,
                    None => {
                        // Probe around start if direct fails
                        let radii = [0.15f32, 0.3, 0.45, 0.6, 0.8, 1.0];
                        let mut found: Option<Vec<Vec3>> = None;
                        'outer_bp: for r in radii.into_iter() {
                            let steps = 16;
                            for i in 0..steps {
                                let theta = (i as f32) * std::f32::consts::TAU / (steps as f32);
                                let cand = Vec3::new(nav_start.x + r * theta.cos(), nav_start.y, nav_start.z + r * theta.sin());
                                if let Some(p) = try_between(cand, end_pos) { found = Some(p); break 'outer_bp; }
                            }
                        }
                        match found {
                            Some(p) => p,
                            None => {
                                // Could not find a valid base path yet; requeue and try again next frame
                                let ev = event.clone();
                                commands.queue(move |world: &mut World| { world.send_event(ev); });
                                continue;
                            }
                        }
                    }
                };

                // 4) Prepend: start_world + manual_points + base_path (skip base_path[0] which equals nav_start)
                path_points.clear();
                path_points.push(start_world);
                path_points.extend(world_manual.into_iter());
                if base_path.len() > 1 { path_points.extend_from_slice(&base_path[1..]); }
            } else {
                // No manual checkpoints: attempt direct path, then probe start if needed
                if let Some(points) = try_between(start_world, end_pos) {
                    println!(
                        "[PathToTag] Computed path ({} pts) from {:?} to {:?}",
                        points.len(), start_world, end_pos
                    );
                    println!("[PathToTag] Points: {:?}", points);
                    path_points = points;
                } else {
                    // Probe around start
                    let radii = [0.15f32, 0.3, 0.45, 0.6, 0.8, 1.0];
                    let mut found = None;
                    'outer: for r in radii.into_iter() {
                        let steps = 16; // 22.5-degree steps
                        for i in 0..steps {
                            let theta = (i as f32) * std::f32::consts::TAU / (steps as f32);
                            let cand = Vec3::new(start_world.x + r * theta.cos(), start_world.y, start_world.z + r * theta.sin());
                            if let Some(points) = try_between(cand, end_pos) {
                                println!("[PathToTag] Probed start at r={:.2}, theta={:.2} -> valid", r, theta);
                                found = Some(points);
                                break 'outer;
                            }
                        }
                    }
                    if let Some(points) = found {
                        println!("[PathToTag] Probing succeeded; path points: {:?}", points);
                        path_points = points;
                    } else {
                        // Could not find a valid start polygon yet; requeue silently and try again
                        let ev = event.clone();
                        commands.queue(move |world: &mut World| { world.send_event(ev); });
                        continue;
                    }
                }
            }

            // Optional wobble: apply starting AT the last manual point (index = manual_points.len())
            // This ensures even a single remaining segment (2-point base path) gets wobble applied.
            let wobble_prefix_len: usize = match &event.manual_points { Some(lps) => lps.len(), None => 0 };
                if let Some(wob) = event.wobble.as_ref() {
                    if path_points.len() >= 2 && wob.checkpoint_spacing > 0.01 && wob.wavelength > 0.01 {
                        // Precompute segment lengths and cumulative arclengths
                        let mut seg_lengths: Vec<f32> = Vec::with_capacity(path_points.len() - 1);
                        let mut cum: Vec<f32> = Vec::with_capacity(path_points.len());
                        cum.push(0.0);
                        for w in path_points.windows(2) {
                            let l = w[1].distance(w[0]);
                            seg_lengths.push(l);
                            cum.push(cum.last().copied().unwrap_or(0.0) + l);
                        }

                        let total_len = *cum.last().unwrap_or(&0.0);
                        // If wobble starts after a prefix (manual points), compute s_start at that index
                        let s_start = if wobble_prefix_len < cum.len() { cum[wobble_prefix_len] } else { 0.0 };
                        // If prefix covers the entire path, skip wobble; otherwise proceed (even for a single remaining segment)
                        if wobble_prefix_len >= path_points.len() { /* no remainder to wobble */ }
                        else {
                        // Helper to sample point and a robust horizontal tangent at arclength s
                        let sample_at = |s: f32| -> (Vec3, Vec3) {
                            let mut s_rem = s.clamp(0.0, total_len);
                            for (i, &l) in seg_lengths.iter().enumerate() {
                                if l <= 1e-5 { continue; }
                                if s_rem <= l {
                                    let t = s_rem / l;
                                    let p0 = path_points[i];
                                    let p1 = path_points[i+1];
                                    let pos = p0.lerp(p1, t);
                                    // Base tangent on current segment
                                    let mut tan = p1 - p0;
                                    // Project to XZ plane for lateral computation
                                    tan.y = 0.0;
                                    let mut tan = tan.normalize_or_zero();
                                    // If this segment has negligible horizontal direction (e.g., vertical),
                                    // fall back to a nearby segment with horizontal movement.
                                    if tan.length_squared() < 1.0e-8 {
                                        // Try previous segment
                                        if i > 0 {
                                            let mut prev = path_points[i] - path_points[i-1];
                                            prev.y = 0.0;
                                            tan = prev.normalize_or_zero();
                                        }
                                        // If still degenerate, try next-next segment
                                        if tan.length_squared() < 1.0e-8 && i + 2 < path_points.len() {
                                            let mut next2 = path_points[i+2] - path_points[i+1];
                                            next2.y = 0.0;
                                            tan = next2.normalize_or_zero();
                                        }
                                        // Final fallback: global X
                                        if tan.length_squared() < 1.0e-8 { tan = Vec3::X; }
                                    }
                                    return (pos, tan);
                                }
                                s_rem -= l;
                            }
                            // Fallback to end
                            let last = path_points.last().copied().unwrap();
                            let prev = path_points[path_points.len()-2];
                            let mut tan = last - prev;
                            tan.y = 0.0;
                            tan = tan.normalize_or_zero();
                            if tan.length_squared() < 1.0e-8 {
                                // Scan backwards for any horizontal movement
                                for w in path_points.windows(2).rev() {
                                    let mut d = w[1] - w[0];
                                    d.y = 0.0;
                                    tan = d.normalize_or_zero();
                                    if tan.length_squared() >= 1.0e-8 { break; }
                                }
                                if tan.length_squared() < 1.0e-8 { tan = Vec3::X; }
                            }
                            (last, tan)
                        };

                        // Attempt wobble up to 3 times, halving amplitude on failure
                        let mut amp = wob.amplitude;
                        let mut applied = false;
                        for _attempt in 0..3 {
                            // Build checkpoints: start at the first post-prefix point, then wobble offsets -> end
                            let mut checkpoints: Vec<Vec3> = Vec::new();
                            // initial checkpoint is the first point after the preserved prefix
                            checkpoints.push(path_points[wobble_prefix_len]);
                            let mut s = (s_start + wob.checkpoint_spacing).min(total_len);
                            let mut made_offset = false;
                            // Place offsets at regular spacing all the way up to just before the end
                            while s < total_len - 1.0e-3 {
                                let (base_p, tan) = sample_at(s);
                                let side = Vec3::Y.cross(tan).normalize_or_zero();
                                let offset = amp * (std::f32::consts::TAU * s / wob.wavelength + wob.phase).sin();
                                let mut cp = base_p + side * offset;
                                // Keep Y on base path height
                                cp.y = base_p.y;
                                checkpoints.push(cp);
                                made_offset = true;
                                s += wob.checkpoint_spacing;
                            }
                            // If remainder was too short for the loop above, place one offset at the midpoint of the remainder
                            if !made_offset {
                                let mid = (s_start + total_len) * 0.5;
                                if mid > s_start + 1e-4 && mid < total_len - 1e-4 {
                                    let (base_p, tan) = sample_at(mid);
                                    let side = Vec3::Y.cross(tan).normalize_or_zero();
                                    let offset = amp * (std::f32::consts::TAU * mid / wob.wavelength + wob.phase).sin();
                                    let mut cp = base_p + side * offset;
                                    cp.y = base_p.y;
                                    checkpoints.push(cp);
                                }
                            }
                            // No artificial pre-end checkpoint: sampling above runs to near total_len
                            // Always include the end point as the final checkpoint
                            checkpoints.push(path_points.last().copied().unwrap());

                            // Now pathfind through each consecutive pair of checkpoints
                            // Start with the preserved prefix (including the junction point)
                            let mut new_path: Vec<Vec3> = Vec::new();
                            if wobble_prefix_len > 0 {
                                new_path.extend_from_slice(&path_points[..=wobble_prefix_len]);
                            } else {
                                new_path.push(path_points[0]);
                            }
                            let mut ok = true;
                            for w in checkpoints.windows(2) {
                                match oxidized_navigation::query::find_path(&tiles, &settings, w[0], w[1], None, None) {
                                    Ok(sub) if sub.len() >= 2 => {
                                        // Avoid duplicating the junction point
                                        new_path.extend_from_slice(&sub[1..]);
                                    }
                                    _ => { ok = false; break; }
                                }
                            }

                            if ok && new_path.len() >= 2 {
                                println!("[PathToTag][Wobble] Applied wobble (amp={:.2}) with {} checkpoints -> {} pts", amp, checkpoints.len(), new_path.len());
                                path_points = new_path;
                                applied = true;
                                break;
                            } else {
                                amp *= 0.5;
                            }
                        }
                        if !applied {
                            println!("[PathToTag][Wobble] Failed to apply wobble after retries; using base path");
                        }
                        }
                    }
                }
        }

        if path_points.len() < 2 { continue; }

        // Store world-space path for global debug
        #[cfg(feature = "debug")] {
            all_dbg.paths.push(path_points.clone());
            if all_dbg.paths.len() > 256 { all_dbg.paths.remove(0); }
        }

        // Convert world-space points to the container's local space using the full inverse transform
        // (accounts for rotation and scale, not just translation)
        let inv_world = world_tf.compute_matrix().inverse();
        let local_points: Vec<Vec3> = path_points
            .iter()
            .map(|p| inv_world.transform_point3(*p))
            .collect();

        // Buffer PathSpawnEvent so it materializes during the next Generating pass
        let reference = event.reference.clone();
        let tension = event.tension;
        let spread = event.spread.clone();
        let count = event.count;
        let transform = event.transform.clone();
        let parent = event.parent;
        let points_len = local_points.len();
        println!("[PathBuffer] Buffered PathSpawnEvent: points={}, parent={:?}", points_len, parent);
        resolved.0.push(PathSpawnEvent { reference, points: local_points, tension, spread, count, transform, parent });
        if let Some(cp) = cur_pass.as_ref() { if highest.0 < cp.0.saturating_add(1) { highest.0 = cp.0.saturating_add(1); } }
        // Also publish the world-space polyline so materials can visualize it
        let world_points = path_points.clone();
        commands.queue(move |world: &mut World| {
            world.send_event(PathWorldPointsEvent { points: world_points });
        });

        // If requested, append each computed polyline into a list labeled with `store_as`
        if let Some(label) = event.store_as.as_ref() {
            // Find or create a holder entity with this tag
            let holder = if let Some((entity, _)) = store_query.iter().find(|(_, t)| t.contains(label)) {
                entity
            } else {
                let e = commands
                    .spawn_empty()
                    .insert(PathPolylineList::default())
                    .insert(Tags(vec![label.clone()]))
                    .insert(Name::new(format!("PathPolylineList: {}", label)))
                    .id();
                if let Some(parent) = event.parent { commands.entity(e).set_parent(parent); }
                eprintln!("[PathStore] Created holder late 'PathPolylineList: {}' under parent {:?}", label, event.parent);
                e
            };
            // Append into the list using a queued world access to mutate the component safely
            let to_add = path_points.clone();
            let label_clone = label.clone();
            commands.queue(move |world: &mut World| {
                let mut ent = world.entity_mut(holder);
                if let Some(mut list) = ent.get_mut::<PathPolylineList>() {
                    list.0.push(to_add);
                    eprintln!("[PathStore] Appended polyline to 'PathPolylineList: {}' (now {} polylines)", label_clone, list.0.len());
                } else {
                    ent.insert(PathPolylineList(vec![to_add]));
                    eprintln!("[PathStore] Inserted new PathPolylineList with 1 polyline into holder 'PathPolylineList: {}'", label_clone);
                }
            });
        }

        processed = true;
    }
    if processed { activity.idle_frames = 0; }
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
    // For pass-gated pending work
    pending_inpass: Res<PendingInPass>,
    cur_pass: Res<CurrentPass>,
) {
    match *state.get() {
        GenerationState::Generating => {
            // Observe activity resets: idle_frames resets to 0 whenever a spawn listener processed events.
            // If we ever observe a drop, mark that we've seen actual activity in this Generating phase.
            if activity.idle_frames < arming.last_idle_frames {
                arming.seen_activity = true;
            }
            arming.last_idle_frames = activity.idle_frames;
            // Pending work check updates stability
            // Also gate on any InPass items for the current pass that haven't been processed yet.
            let any_inpass_current = pending_inpass.0.iter().any(|ev| ev.index == cur_pass.0);
            let any_pending = !gen_only_pending.is_empty() || !selective_pending.is_empty() || any_inpass_current;
            if any_pending {
                stability.no_pending_stable_frames = 0;
                arming.spawn_done_frames = 0;
                if generating_frames.frames_in_generating % 30 == 0 {
                    #[cfg(feature = "debug")]
                    {
                        let gen_cnt = gen_only_pending.iter().count();
                        let sel_cnt = selective_pending.iter().count();
                        let ip_cnt = pending_inpass.0.iter().filter(|ev| ev.index == cur_pass.0).count();
                        println!(
                            "[GenState][Generating] pending: gen_only={} selective={} inpass_cur_pass={} (holding)",
                            gen_cnt, sel_cnt, ip_cnt
                        );
                    }
                }
                return;
            } else {
                stability.no_pending_stable_frames = stability.no_pending_stable_frames.saturating_add(1);
            }

            // Readiness thresholds: wait a bit longer and require multiple stable/idle frames
            const MIN_GENERATING_FRAMES: u16 = 15;
            const REQUIRED_STABLE_FRAMES: u8 = 3;
            const REQUIRED_IDLE_FRAMES: u8 = 3;
            // Require at least one observed activity reset before progressing out of Generating.
            let ready = arming.seen_activity
                && generating_frames.frames_in_generating >= MIN_GENERATING_FRAMES
                && stability.no_pending_stable_frames >= REQUIRED_STABLE_FRAMES
                && activity.idle_frames >= REQUIRED_IDLE_FRAMES;

            if ready {
                // Debounce before advancing
                arming.spawn_done_frames = arming.spawn_done_frames.saturating_add(1);
                const REQUIRED_FRAMES: u8 = 8;
                if arming.spawn_done_frames >= REQUIRED_FRAMES {
                    info!("[GenState] Generating -> CollisionResolution");
                    next.set(GenerationState::CollisionResolution);
                    arming.spawn_done_frames = 0;
                }
            } else {
                arming.spawn_done_frames = 0;
                if generating_frames.frames_in_generating % 30 == 0 {
                    println!(
                        "[GenState][Generating] waiting: frames={} stable_frames={} idle_frames={} (need >= {}, {}, {})",
                        generating_frames.frames_in_generating,
                        stability.no_pending_stable_frames,
                        activity.idle_frames,
                        MIN_GENERATING_FRAMES,
                        REQUIRED_STABLE_FRAMES,
                        REQUIRED_IDLE_FRAMES
                    );
                }
            }
        }
        GenerationState::CollisionResolution => {
            // Minimal wait (1 frame) just to flush any leftover contact events, then proceed.
            const MIN_COLLISION_FRAMES: u16 = 1;
            const MAX_COLLISION_FRAMES: u16 = 60;  // safety cap
            timer.frames = timer.frames.saturating_add(1);
            if timer.frames >= MIN_COLLISION_FRAMES || timer.frames > MAX_COLLISION_FRAMES {
                println!("[GenState] CollisionResolution -> NavMeshBuilding");
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
                println!("[GenState] NavMeshBuilding -> PathResolve");
                next.set(GenerationState::PathResolve);
            }
        }
        GenerationState::PathResolve => {
            // No-op here; advancement handled by advance_from_path_resolve
        }
        GenerationState::Completed => {}
    }
}

// Timer used to ensure at least one frame in PathResolve state
#[derive(Resource, Default)]
pub struct PathResolveTimer { pub frames: u16 }

// On entering PathResolve, reset the timer
pub fn reset_path_resolve_phase(mut timer: ResMut<PathResolveTimer>) {
    timer.frames = 0;
}

// After one update in PathResolve, advance to Completed
pub fn advance_from_path_resolve(
    state: Res<State<GenerationState>>,
    mut timer: ResMut<PathResolveTimer>,
    mut next: ResMut<NextState<GenerationState>>,
    resolved: Res<ResolvedPathSpawns>,
) {
    if state.get() != &GenerationState::PathResolve { return; }
    timer.frames = timer.frames.saturating_add(1);
    // Give path systems multiple frames to resolve (retries, asset readiness, etc.)
    const PATH_RESOLVE_FRAMES: u16 = 60; // ~1s at 60 FPS
    if timer.frames >= PATH_RESOLVE_FRAMES {
        if !resolved.0.is_empty() {
            println!("[GenState] PathResolve -> Generating (have {} buffered PathSpawnEvent)", resolved.0.len());
            next.set(GenerationState::Generating);
        } else {
            println!("[GenState] PathResolve -> Completed");
            next.set(GenerationState::Completed);
        }
        timer.frames = 0;
    }
}

// On entering Generating, reset stability counters/flags
pub fn reset_generating_phase(
    mut stability: ResMut<SpawningStability>,
    mut gen_counter: ResMut<GeneratingFrameCounter>,
    mut activity: ResMut<SpawnActivity>,
    mut arming: ResMut<GenerationAdvanceArming>,
) {
    stability.no_pending_stable_frames = 0;
    gen_counter.frames_in_generating = 0;
    activity.idle_frames = 0;
    arming.spawn_done_frames = 0;
    arming.seen_activity = false;
    // Initialize high so the first reset to 0 by any spawn listener is detected as a drop
    arming.last_idle_frames = u8::MAX;
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
    cur_pass: Option<Res<CurrentPass>>,
    highest_pass: Option<Res<HighestPassIndex>>,
    mut q: Query<&mut Text, With<GenStateOverlay>>,
) {
    let mut text = match q.get_single_mut() {
        Ok(t) => t,
        Err(_) => return,
    };
    let gs = state.as_ref().map(|s| s.get()).cloned().unwrap_or(GenerationState::Generating);
    let tasks = active_tasks.map(|t| t.len()).unwrap_or(0);
    let cp = cur_pass.map(|p| p.0).unwrap_or(0);
    let hp = highest_pass.map(|p| p.0).unwrap_or(0);
    text.0 = format!("GenerationState: {:?}\nActiveNavMeshTasks: {}\nPass: {}/{}", gs, tasks, cp + 1, hp + 1);
}

// High-level generation/state-of-world progression
#[derive(States, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, Default)]
pub enum GenerationState {
    #[default]
    Generating,
    CollisionResolution,
    NavMeshBuilding,
    // New intermediate state where we resolve paths (now that navmesh is ready)
    PathResolve,
    Completed,
}

// Simple frame counter used to wait during CollisionResolution
#[derive(Resource, Default)]
pub struct CollisionResolutionTimer {
    pub frames: u16,
    pub quiet_frames: u16,
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

// Buffer for path events authored during Generating; these will be flushed in PathResolve
#[derive(Resource, Default)]
pub struct PendingPathEvents {
    pub to_tag: Vec<PathToTagSpawnEvent>,
    pub to_all: Vec<PathToAllTagsSpawnEvent>,
    pub plain: Vec<PathSpawnEvent>,
}

// Buffer for Nest events authored during PathResolve; these will be flushed in the next Generating
#[derive(Resource, Default)]
pub struct PendingNests(pub Vec<NestSpawnEvent>);

// During Generating, capture path events and store them for PathResolve. Also eagerly create holders.
pub fn buffer_path_events(
    mut commands: Commands,
    mut r_to_tag: EventReader<PathToTagSpawnEvent>,
    mut r_to_all: EventReader<PathToAllTagsSpawnEvent>,
    mut r_plain: EventReader<PathSpawnEvent>,
    mut pending: ResMut<PendingPathEvents>,
    store_query: Query<(Entity, &Tags)>,
    mut activity: ResMut<SpawnActivity>,
    mut highest: ResMut<HighestPassIndex>,
    cur_pass: Option<Res<CurrentPass>>,
) {
    let mut any = false;
    for ev in r_to_tag.read() {
        if let Some(label) = ev.store_as.as_ref() {
            let existing = store_query.iter().find(|(_, t)| t.contains(label)).map(|(e, _)| e);
            if existing.is_none() {
                let e = commands
                    .spawn_empty()
                    .insert(PathPolylineList::default())
                    .insert(Tags(vec![label.clone()]))
                    .insert(Name::new(format!("PathPolylineList: {}", label)))
                    .id();
                if let Some(parent) = ev.parent { commands.entity(e).set_parent(parent); }
                eprintln!("[PathStore] (Buffer) Created holder 'PathPolylineList: {}' under parent {:?}", label, ev.parent);
            }
        }
        pending.to_tag.push(ev.clone());
        any = true;
    }
    for ev in r_to_all.read() {
        if let Some(label) = ev.store_as.as_ref() {
            let existing = store_query.iter().find(|(_, t)| t.contains(label)).map(|(e, _)| e);
            if existing.is_none() {
                let e = commands
                    .spawn_empty()
                    .insert(PathPolylineList::default())
                    .insert(Tags(vec![label.clone()]))
                    .insert(Name::new(format!("PathPolylineList: {}", label)))
                    .id();
                if let Some(parent) = ev.parent { commands.entity(e).set_parent(parent); }
                eprintln!("[PathStore] (Buffer) Created holder 'PathPolylineList: {}' under parent {:?}", label, ev.parent);
            }
        }
        pending.to_all.push(ev.clone());
        if let Some(cp) = cur_pass.as_ref() { if highest.0 < cp.0.saturating_add(1) { highest.0 = cp.0.saturating_add(1); } }
        any = true;
    }
    for ev in r_plain.read() {
        pending.plain.push(ev.clone());
        if let Some(cp) = cur_pass.as_ref() { if highest.0 < cp.0.saturating_add(1) { highest.0 = cp.0.saturating_add(1); } }
        any = true;
    }
    if any { activity.idle_frames = 0; }
}

// On entering PathResolve, flush buffered path events back into the event system
pub fn flush_path_events_on_enter(
    mut pending: ResMut<PendingPathEvents>,
    mut w_to_tag: EventWriter<PathToTagSpawnEvent>,
    mut w_to_all: EventWriter<PathToAllTagsSpawnEvent>,
    mut w_plain: EventWriter<PathSpawnEvent>,
) {
    if !pending.to_tag.is_empty() { eprintln!("[PathStore] Flushing {} PathToTag events to PathResolve", pending.to_tag.len()); }
    if !pending.to_all.is_empty() { eprintln!("[PathStore] Flushing {} PathToAllTags events to PathResolve", pending.to_all.len()); }
    if !pending.plain.is_empty() { eprintln!("[PathStore] Flushing {} PathSpawn events to PathResolve", pending.plain.len()); }
    for ev in pending.to_tag.drain(..) { w_to_tag.send(ev); }
    for ev in pending.to_all.drain(..) { w_to_all.send(ev); }
    for ev in pending.plain.drain(..) { w_plain.send(ev); }
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
    pub seen_activity: bool,
    pub last_idle_frames: u8,
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
        println!("[GenState] NavMeshBuilding -> Completed");
        next.set(GenerationState::Completed);
    }
}

// On entering Completed, either advance to the next pass (if any), or remain Completed
pub fn advance_pass_or_finish(
    mut next: ResMut<NextState<GenerationState>>,
    mut cur: ResMut<CurrentPass>,
    highest: Res<HighestPassIndex>,
) {
    if cur.0 < highest.0 {
        cur.0 = cur.0.saturating_add(1);
        println!("[Pass] Advancing to pass {}", cur.0 + 1);
        next.set(GenerationState::Generating);
    } else {
        println!("[Pass] Final pass reached: {}", cur.0 + 1);
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
        #[cfg(not(feature = "debug"))]
        let _ = &name_opt;
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
            #[cfg(not(feature = "debug"))]
            let _ = &removed_count;
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
    // For direct PathBlend application
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mut ext_mats: ResMut<Assets<PathBlendMaterial>>,
    mut ground_res: ResMut<GroundPathMaterial>,
    asset_server: Res<AssetServer>,
    mut activity: ResMut<SpawnActivity>,
    tag_query: Query<&Tags>,
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
            TMaterial::PathBlend { material_name, tiling_factor, .. } => {
                // For early mesh setup, behave like TiledMaterial: clone and scale UVs.
                // The actual PathBlend material handle is created later when inserting components.
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
            // Branch on material type: regular StandardMaterial vs PathBlendMaterial
            match &event.material {
                TMaterial::PathBlend { near_albedo_path, near_metallic_roughness_path, near_ao_path, .. } => {
                    // Build PathBlend material from the cached base StandardMaterial
                    let base = std_mats.get(material_handle).cloned().unwrap_or_else(StandardMaterial::default);
                    // Tuned defaults for high-visibility path blending
                    let mut p = PathBlendParams::default();
                    p.fade_radius = 0.2;
                    p.min_blend = 0.0;
                    p.max_blend = 1.0;
                    p.near_base_color = Vec4::new(0.32, 0.24, 0.16, 1.0);
                    p.near_metallic = 0.0;
                    p.near_roughness = 0.95;
                    p.thickness_scale = 1.0;
                    p.base_width = 0.5;
                    p.set_falloff_mode(crate::materials::path_blend::falloff_mode::LINEAR);
                    p.set_invert(false);
                    // Initially no segments; they will be supplied via PathWorldPointsEvent
                    p.clear_segments();
                    let near_albedo = near_albedo_path.as_ref().map(|p| asset_server.load(p.as_str()));
                    let near_mr = near_metallic_roughness_path.as_ref().map(|p| asset_server.load(p.as_str()));
                    let near_ao = near_ao_path.as_ref().map(|p| asset_server.load(p.as_str()));
                    let handle = make_path_blend_material(&mut ext_mats, base, p, &[], near_albedo, near_mr, near_ao);

                    commands.entity(entity_id)
                        .insert(Mesh3d(mesh_handle))
                        .insert(MeshMaterial3d(handle.clone()))
                        .insert(Transform::from(event.transform.clone()))
                        .insert(Name::new("Mesh"))
                        .insert(Collider::cuboid(collider_size.x, collider_size.y, collider_size.z))
                        .insert(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS)
                        .insert(ActiveCollisionTypes::all())
                        .insert(QueuedNavMeshAffector)
                        .insert(InheritedVisibility::default());

                    // Set the ground path material handle if not already set
                    // Always update to the latest spawned ground material so subsequent generations bind correctly
                    ground_res.0 = Some(handle);
                }
                _ => {
                    commands.entity(entity_id)
                        .insert(Mesh3d(mesh_handle))
                        .insert(MeshMaterial3d((*material_handle).clone()))
                        .insert(Transform::from(event.transform.clone()))
                        .insert(Name::new("Mesh"))
                        .insert(Collider::cuboid(collider_size.x, collider_size.y, collider_size.z))
                        .insert(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS)
                        .insert(ActiveCollisionTypes::all())
                        .insert(QueuedNavMeshAffector)
                        .insert(InheritedVisibility::default());
                }
            }

            // Set parent if applicable, and propagate Tags from the parent so matching works
            if let Some(parent) = event.parent {
                commands.entity(entity_id).set_parent(parent);
                if let Ok(tags) = tag_query.get(parent) {
                    commands.entity(entity_id).insert(tags.clone());
                }
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

        if let StructureKey::Object { path, collider, offset, ownership, selectable, object_type, visibility } = &event.data {
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

            // Apply authored visibility mode, defaulting to Visible when unspecified
            let mode = visibility.clone().unwrap_or(crate::core::structure_key::VisibilityMode::Visible);
            match mode {
                crate::core::structure_key::VisibilityMode::Visible => {
                    commands.entity(parent_entity).insert(Visibility::Visible);
                    #[cfg(feature = "debug")]
                    println!("[Spawn][Visibility] {:?} -> Visible", parent_entity);
                }
                crate::core::structure_key::VisibilityMode::Hidden => {
                    commands.entity(parent_entity).insert(Visibility::Hidden);
                    #[cfg(feature = "debug")]
                    println!("[Spawn][Visibility] {:?} -> Hidden", parent_entity);
                }
                crate::core::structure_key::VisibilityMode::Inherit => {
                    // Do not insert Visibility; allow inheritance from ancestors
                    #[cfg(feature = "debug")]
                    println!("[Spawn][Visibility] {:?} -> Inherit (no explicit Visibility)", parent_entity);
                }
            }

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
                            // Units should not affect the navmesh
                        }
                        // All other object types, including Cosmetic, should affect the navmesh if they have colliders
                        _ => {
                            entity_commands.insert(QueuedNavMeshAffector);
                        }
                    }

                    // Rigid bodies are disabled for now; keep colliders only
                    let _ = internal_collider.behaviour;
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

// Generic directional light spawner: always a regular light, parented under the provided container (if any)
pub fn directional_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<DirectionalLightSpawnEvent>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        let entity = commands
            .spawn_empty()
            .insert(event.light.clone())
            .insert(Transform::from(event.transform.clone()))
            .insert(InheritedVisibility::default())
            .insert(Name::new("DirectionalLight"))
            .id();
        if let Some(parent) = event.parent {
            commands.entity(entity).set_parent(parent);
        }
    }
    if processed { activity.idle_frames = 0; }
}

// Dedicated spawner for the single main directional light (sun)
pub fn main_directional_light_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<MainDirectionalLightSpawnEvent>,
    existing: Query<Entity, With<MainDirectionalLight>>,
    mut activity: ResMut<SpawnActivity>,
) {
    let mut processed = false;
    for event in reader.read() {
        processed = true;
        let target = existing.iter().next().unwrap_or_else(|| commands.spawn_empty().id());
        let mut ecmd = commands.entity(target);
        ecmd
            .insert(event.light.clone())
            .insert(Transform::from(event.transform.clone()))
            .insert(InheritedVisibility::default())
            .insert(Name::new("MainDirectionalLight"))
            .insert(CascadeShadowConfig {
                bounds: vec![0.0, 30.0, 90.0, 270.0],
                overlap_proportion: 0.2,
                minimum_distance: 0.0,
            })
            .insert(MainDirectionalLight);
        if let Some(parent) = event.parent {
            commands.entity(target).set_parent(parent);
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
                    .insert(GlobalTransform::default())
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
        info!(
            "[RandJiggle] t=({:.3},{:.3},{:.3}) r=({:.1},{:.1},{:.1}) s=({:.2},{:.2},{:.2})",
            jiggled.translation.0, jiggled.translation.1, jiggled.translation.2,
            jiggled.rotation.0, jiggled.rotation.1, jiggled.rotation.2,
            jiggled.scale.0, jiggled.scale.1, jiggled.scale.2
        );
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
            SpreadData::Constant(spacing) => {
                // Even spacing along the ORIGINAL polyline (event.points), inclusive of endpoints
                let pts = &event.points;
                if pts.len() < 2 {
                    pts.clone()
                } else {
                    let mut seg_lengths: Vec<f32> = Vec::with_capacity(pts.len() - 1);
                    let mut cum: Vec<f32> = Vec::with_capacity(pts.len());
                    cum.push(0.0);
                    for w in pts.windows(2) {
                        let l = w[1].distance(w[0]);
                        seg_lengths.push(l);
                        cum.push(cum.last().copied().unwrap_or(0.0) + l);
                    }
                    let total_len = *cum.last().unwrap_or(&0.0);
                    let step = spacing.max(0.001);

                    // Helper to sample along the polyline at arclength s
                    let sample_at = |s: f32| -> Vec3 {
                        let mut s_rem = s.clamp(0.0, total_len);
                        for (i, &l) in seg_lengths.iter().enumerate() {
                            if l <= 1e-6 { continue; }
                            if s_rem <= l {
                                let t = s_rem / l;
                                return pts[i].lerp(pts[i+1], t);
                            }
                            s_rem -= l;
                        }
                        *pts.last().unwrap()
                    };

                    let mut out: Vec<Vec3> = Vec::new();
                    let mut s = 0.0;
                    while s + 1.0e-4 < total_len {
                        out.push(sample_at(s));
                        s += step;
                    }
                    // Include the endpoint only if there's at least one full spacing remaining
                    let last_s = if out.is_empty() { 0.0 } else { (s - step).max(0.0) };
                    let remainder = total_len - last_s;
                    if remainder + 1.0e-4 >= step {
                        out.push(*pts.last().unwrap());
                    }
                    out
                }
            }
            _ => {
                panic!("This spread type not supported yet!");
            }
        };
        // Compute simple tangents using neighboring points (forward differences at ends)
        let n = positions.len();
        let mut tangents: Vec<Vec3> = Vec::with_capacity(n);
        for i in 0..n {
            let prev = if i > 0 { positions[i - 1] } else { positions[i] };
            let next = if i + 1 < n { positions[i + 1] } else { positions[i] };
            let mut t = next - prev;
            // Use horizontal tangent for yaw alignment
            t.y = 0.0;
            let mut t = t.normalize_or_zero();
            if t.length_squared() < 1.0e-6 { t = Vec3::Z; }
            tangents.push(t);
        }

        for (point, tan) in positions.into_iter().zip(tangents.into_iter()) {
            // Yaw so +Z faces along the tangent
            let yaw_rad = tan.x.atan2(tan.z);
            let yaw_deg = yaw_rad.to_degrees();
            let euler = EulerTransform {
                translation: (point.x, point.y, point.z),
                rotation: (0.0, yaw_deg, 0.0),
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
            #[cfg(not(feature = "debug"))]
            let _ = &name_opt;
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
    // Track entities we already queued for despawn this frame to avoid double-despawn warnings
    let mut already_despawned: HashSet<Entity> = HashSet::new();
    // Process contact force events (if any)
    for ev in contact_events.read() {
        let a = ev.collider1;
        let b = ev.collider2;
        if let (Ok(pa), Ok(pb)) = (priorities.get(a), priorities.get(b)) {
            let name_a = name_query.get(a).ok().map(|n| n.as_str().to_string());
            let name_b = name_query.get(b).ok().map(|n| n.as_str().to_string());
            #[cfg(not(feature = "debug"))]
            { let _ = &name_a; let _ = &name_b; }
            info!(
                "[PriorityDespawn] ContactForce: {:?}({:?})[{}] <-> {:?}({:?})[{}]",
                a, name_a, pa.0, b, name_b, pb.0
            );
            if pa.0 == pb.0 { continue; }
            let loser = if pa.0 < pb.0 { a } else { b };
            let loser_name = name_query.get(loser).ok().map(|n| n.as_str().to_string());
            #[cfg(not(feature = "debug"))]
            { let _ = &loser_name; }
            if already_despawned.insert(loser) {
                info!("[PriorityDespawn] Despawning lower priority entity: {:?} ({:?})", loser, loser_name);
                commands.queue(move |world: &mut World| {
                    if let Ok(ent) = world.get_entity_mut(loser) {
                        ent.despawn_recursive();
                    }
                });
            }
        }
    }

    // Also process basic collision start events (more reliable for kinematic bodies)
    for ev in collision_events.read() {
        if let CollisionEvent::Started(a, b, _) = ev {
            if let (Ok(pa), Ok(pb)) = (priorities.get(*a), priorities.get(*b)) {
                let name_a = name_query.get(*a).ok().map(|n| n.as_str().to_string());
                let name_b = name_query.get(*b).ok().map(|n| n.as_str().to_string());
                #[cfg(not(feature = "debug"))]
                { let _ = &name_a; let _ = &name_b; }
                info!(
                    "[PriorityDespawn] CollisionStarted: {:?}({:?})[{}] <-> {:?}({:?})[{}]",
                    *a, name_a, pa.0, *b, name_b, pb.0
                );
                if pa.0 == pb.0 { continue; }
                let loser = if pa.0 < pb.0 { *a } else { *b };
                let loser_name = name_query.get(loser).ok().map(|n| n.as_str().to_string());
                #[cfg(not(feature = "debug"))]
                { let _ = &loser_name; }
                if already_despawned.insert(loser) {
                    info!("[PriorityDespawn] Despawning lower priority entity: {:?} ({:?})", loser, loser_name);
                    commands.queue(move |world: &mut World| {
                        if let Ok(ent) = world.get_entity_mut(loser) {
                            ent.despawn_recursive();
                        }
                    });
                }
            }
        }
    }
}
