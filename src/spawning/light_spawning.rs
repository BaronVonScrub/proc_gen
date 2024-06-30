use bevy::prelude::*;

pub(crate) fn spawn_point_light(
    commands: &mut Commands,
    point_light: PointLight,
    transform: Transform
) -> Entity {
    let entity = commands.spawn(PointLightBundle {
        point_light,
        transform,
        ..Default::default()
    }).id();
    commands.entity(entity).insert(Name::new("Pointlight".to_string()));

    entity
}

pub(crate) fn spawn_spot_light(
    commands: &mut Commands,
    spot_light: SpotLight,
    transform: Transform
) -> Entity {
    let entity = commands.spawn(SpotLightBundle {
        spot_light,
        transform,
        ..Default::default()
    }).id();
    commands.entity(entity).insert(Name::new("Spotlight".to_string()));

    entity
}
