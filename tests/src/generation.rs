use bevy::prelude::{EventWriter, Mesh, Vec2};
use bevy_math::prelude::Plane3d;
use bevy_utils::default;
use proc_gen::core::tmaterial::TMaterial;
use proc_gen::spawning::euler_transform::EulerTransform;
use proc_gen::systems::events::ObjectSpawnEvent;

pub(crate) fn generate_map(
    mut spawn_writer: EventWriter<ObjectSpawnEvent>,
)
{
    spawn_writer.send(
        ObjectSpawnEvent::StructureSpawn{
            structure: "atmospheric_setup".to_string(),
            transform: Default::default(),
            parent: None,
        }
    );

    spawn_writer.send(
        ObjectSpawnEvent::StructureSpawn{
            structure: "Castle/damaged_castle".to_string(),
            transform: Default::default(),
            parent: None,
        }
    );


    spawn_writer.send(
        ObjectSpawnEvent::StructureSpawn{
            structure: "test_unit".to_string(),
            transform: EulerTransform {
                translation: ( 10.0, 0.0, -10.0 ),
                rotation: Default::default(),
                scale: (1.0,1.0,1.0),
            },
            parent: None,
        }
    );


    spawn_writer.send(
        ObjectSpawnEvent::MeshSpawn {
            mesh: Mesh::from(Plane3d { ..default() })
                .with_generated_tangents()
                .unwrap(),
            transform: EulerTransform {
                translation: (0.0, 0.0, 0.0),
                rotation: (0.0, 0.0, 0.0),
                scale: (20.0, 20.0, 20.0)
            },
            material: TMaterial::TiledMaterial {
                material_name: "Grass".to_string(),
                tiling_factor: Vec2::new(5.0, 5.0),
            },
            parent: None,
        }
    );
}
