use bevy::prelude::*;

pub(crate) fn spawn_point_light(
    commands: &mut Commands,
    point_light: PointLight,
    transform: Transform
) -> Entity {
    let entity = commands.spawn_empty()
        .insert(point_light)
        .insert(transform)
        .insert(Name::new("Pointlight".to_string()))
        .insert(InheritedVisibility::default())
        .id();

    entity
}

pub(crate) fn spawn_spot_light(
    commands: &mut Commands,
    spot_light: SpotLight,
    transform: Transform
) -> Entity {
    let entity = commands.spawn_empty()
        .insert(spot_light)
        .insert(transform)
        .insert(Name::new("Spotlight".to_string()))
        .insert(InheritedVisibility::default())
        .id();
    commands.entity(entity).insert(Name::new("Spotlight".to_string()));

    entity
}
