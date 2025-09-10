use bevy::prelude::*;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct MainDirectionalLight;

// Holds a polyline of world-space points for a computed path.
// Attach this to an entity with a `Tags` component to retrieve it by label via a query.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct PathPolyline(pub Vec<Vec3>);

// Holds multiple world-space polylines (e.g., PathToAllTags results) under a single label entity.
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct PathPolylineList(pub Vec<Vec<Vec3>>);