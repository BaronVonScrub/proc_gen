use bevy::prelude::*;
use crate::core::structure::Structure;
use crate::management::structure_management::import_structure;
use crate::event_system::spawn_events::StructureSpawnEvent;
use crate::spawning::euler_transform::EulerTransform;

pub fn structure_spawn_listener(
    mut commands: Commands,
    mut reader: EventReader<StructureSpawnEvent>,
) {
    for event in reader.read() {
        if let Err(e) = spawn_structure(&mut commands, event) {
            eprintln!("Error spawning structure: {}", e);
        }
    }
}

fn spawn_structure(
    commands: &mut Commands,
    event: &StructureSpawnEvent,
) -> Result<Option<Entity>, String> {
    let structure_name = &event.structure;

    // Create the entity
    let entity = commands.spawn_empty()
        .insert(Name::new(structure_name.clone()))
        .insert(Transform::from(event.transform.clone()))
        .insert(InheritedVisibility::default())
        .id();

    // Attach to parent if specified
    if let Some(parent) = event.parent {
        commands.entity(parent).add_child(entity);
    }

    // Load the structure
    let structure = import_structure(structure_name.clone())
        .map_err(|e| format!("Failed to import structure {}: {}", structure_name, e))?;

    // Spawn its components using events
    spawn_structure_data(commands, &structure, Transform::IDENTITY, Some(entity))?;

    Ok(Some(entity))
}

pub(crate) fn spawn_structure_data(
    commands: &mut Commands,
    structure: &Structure,
    parent_transform: Transform,
    parent: Option<Entity>,
) -> Result<Option<Entity>, String> {
    for (key, local_transform) in &structure.data {
        let combined_transform = parent_transform * Transform::from(local_transform.clone());
        key.dispatch_event(EulerTransform::from(combined_transform), parent, commands);
    }

    Ok(parent)
}