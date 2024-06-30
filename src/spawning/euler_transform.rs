use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::Transform;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EulerTransform {
    pub(crate) translation: (f32, f32, f32),   // Translation vector
    pub(crate) rotation: (f32, f32, f32),      // Rotation in Euler angles (degrees)
    pub(crate) scale: (f32, f32, f32),         // Scale vector
}

impl Default for EulerTransform {
    fn default() -> Self {
        EulerTransform {
            translation: (0.0, 0.0, 0.0),  // Default translation is zero
            rotation: (0.0, 0.0, 0.0),     // Default rotation is zero
            scale: (1.0, 1.0, 1.0),        // Default scale is one
        }
    }
}

impl From<EulerTransform> for Transform {
    fn from(euler_transform: EulerTransform) -> Self {
        let translation = Vec3::new(euler_transform.translation.0, euler_transform.translation.1, euler_transform.translation.2);
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            euler_transform.rotation.0.to_radians(),
            euler_transform.rotation.1.to_radians(),
            euler_transform.rotation.2.to_radians(),
        );
        let scale = Vec3::new(euler_transform.scale.0, euler_transform.scale.1, euler_transform.scale.2);

        Transform {
            translation,
            rotation,
            scale,
        }
    }
}

impl From<Transform> for EulerTransform {
    fn from(transform: Transform) -> Self {
        // Decompose the Transform's quaternion into Euler angles in degrees
        let (roll, pitch, yaw) = transform.rotation.to_euler(EulerRot::XYZ);

        EulerTransform {
            translation: (transform.translation.x, transform.translation.y, transform.translation.z),
            rotation: (roll.to_degrees(), pitch.to_degrees(), yaw.to_degrees()),
            scale: (transform.scale.x, transform.scale.y, transform.scale.z),
        }
    }
}


