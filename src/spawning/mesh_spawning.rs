use bevy_rapier3d::prelude::{ActiveEvents, Collider, RigidBody};
use bevy::prelude::*;
use oxidized_navigation::NavMeshAffector;
use crate::core::tmaterial::TMaterial;
use crate::serialization::caching::MaterialCache;
use bevy::render::mesh::MeshAabb;

pub fn spawn_mesh(
    commands: &mut Commands,
    material_cache: &Res<MaterialCache>,
    meshes: &mut ResMut<Assets<Mesh>>,
    mesh: &Mesh,
    transform: Transform,
    material: &TMaterial,
) {
    let (material_name, adjusted_mesh) = match material {
        TMaterial::BasicMaterial { material_name } => {
            (material_name.clone(), mesh.clone()) // No tiling factor adjustment needed
        },
        TMaterial::TiledMaterial { material_name, tiling_factor } => {
            let mut mesh = mesh.clone();
            if let Some(bevy::render::mesh::VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
                for uv in uvs.iter_mut() {
                    uv[0] *= tiling_factor.x;
                    uv[1] *= tiling_factor.y;
                }
            }
            (material_name.clone(), mesh)
        },
    };
    let bounding_box = adjusted_mesh.compute_aabb().unwrap();
    let half_extents = bounding_box.half_extents;
    let collider_size = Vec3::new(half_extents.x, half_extents.y, half_extents.z);

    if let Some(material_handle) = material_cache.get(&material_name) {
        let mesh_handle = meshes.add(adjusted_mesh);

        commands.spawn_empty()
            .insert(Mesh3d(mesh_handle))
            .insert(MeshMaterial3d((*material_handle).clone()))
            .insert(transform)
            .insert(Name::new("Floor"))
            .insert(Collider::cuboid(collider_size.x, collider_size.y, collider_size.z))
            .insert(RigidBody::KinematicPositionBased)
            .insert(ActiveEvents::CONTACT_FORCE_EVENTS)
            .insert(NavMeshAffector)
            .insert(InheritedVisibility::default());
    } else {
        println!("Material not found: {}", material_name);
    }
}