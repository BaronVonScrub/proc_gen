use std::path::Path;
use bevy::prelude::*;
use bevy_rapier3d::prelude::{ActiveEvents, ContactForceEventThreshold, Damping, Dominance, LockedAxes, RigidBody, Sleeping};
use oxidized_navigation::NavMeshAffector;
use crate::spawning::object_logic::{ObjectType, Pathfinder, PathState, Selectable};
use crate::core::structure_key::StructureKey;
use crate::core::collider::ColliderBehaviour;
use crate::spawning::helpers::*;

pub(crate) fn spawn_scene_from_path(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    model_data: &StructureKey,
    global_transform: Transform,
    local_transform: Transform
) -> Entity {
    let parent_entity = commands.spawn(SpatialBundle {
        transform: global_transform * local_transform,
        ..Default::default()
    }).id();

    if let StructureKey::Object { path, collider, offset, ownership, selectable, object_type } = model_data {
        let scene_handle: Handle<Scene> = asset_server.load(path);

        commands.entity(parent_entity).with_children(|parent| {
            parent.spawn(SceneBundle {
                scene: scene_handle,
                transform: Transform::from_translation(*offset),
                ..Default::default()
            });
        });

        let filename = Path::new(path)
            .file_name()
            .and_then(|file_name| file_name.to_str()).unwrap();

        commands.entity(parent_entity)
            .insert(Name::new(format!("{:?}", filename)))
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
                        let start_goal = (global_transform * local_transform).translation;
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
                    },
                    ColliderBehaviour::Kinematic => {
                        entity_commands.insert(RigidBody::KinematicPositionBased);
                    },
                }
            }
        }
    }

    parent_entity
}