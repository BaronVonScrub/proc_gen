use bevy::prelude::*;
use bevy_prng::WyRand;
use bevy_rapier3d::prelude::*;
use crate::core::collider::ColliderType;
use crate::core::rand_data::RandData;
use statrs::distribution::{Normal};
use rand::{Rng, SeedableRng};
use rand::distributions::Distribution;
use crate::spawning::euler_transform::EulerTransform;

pub fn reflect_point(
    point: Vec3,
    reflection_plane: Plane3d,
    reflection_point: Vec3,
) -> Vec3 {
    let point_to_reflection_point = point - reflection_point;
    let norm_as_vec = Vec3::new(reflection_plane.normal.x, reflection_plane.normal.y, reflection_plane.normal.z);
    let projection = point_to_reflection_point.dot(*reflection_plane.normal) * norm_as_vec;
    point - 2.0 * projection
}

pub fn jiggle_transform(
    gen_rng: &mut ResMut<GenRng>,
    rand_data: RandData,
    original_transform: EulerTransform,
) -> EulerTransform {
    let random_floats: Vec<f32> = match rand_data {
        RandData::Linear(spread) => {
            (0..7).map(|_| gen_rng.rng_mut().gen::<f32>() * spread - spread / 2.0).collect()
        }
        RandData::Gaussian(standard_deviation) => {
            let normal_dist = Normal::new(0.0, standard_deviation as f64).unwrap();
            (0..7).map(|_| normal_dist.sample(gen_rng.rng_mut()) as f32).collect()
        }
    };

    EulerTransform {
        translation: (
            original_transform.translation.0 * random_floats[0],
            original_transform.translation.1 * random_floats[1],
            original_transform.translation.2 * random_floats[2],
        ),
        rotation: (
            original_transform.rotation.0 * random_floats[3],
            original_transform.rotation.1 * random_floats[4],
            original_transform.rotation.2 * random_floats[5],
        ),
        scale: (
            2.0f32.powf(original_transform.scale.0 * random_floats[6]),
            2.0f32.powf(original_transform.scale.1 * random_floats[6]),
            2.0f32.powf(original_transform.scale.2 * random_floats[6])
        ),
    }
}

pub fn create_collider(collider_type: &ColliderType) -> Option<Collider> {
    match collider_type {
        ColliderType::None => None,
        ColliderType::Ball { radius } => Some(Collider::ball(*radius)),
        ColliderType::Cylinder { half_height, radius } => Some(Collider::cylinder(*half_height, *radius)),
        ColliderType::RoundCylinder { half_height, radius, border_radius } =>
            Some(Collider::round_cylinder(*half_height, *radius, *border_radius)),
        ColliderType::Cone { half_height, radius } => Some(Collider::cone(*half_height, *radius)),
        ColliderType::RoundCone { half_height, radius, border_radius } =>
            Some(Collider::round_cone(*half_height, *radius, *border_radius)),
        ColliderType::Capsule { start, end, radius } => Some(Collider::capsule(*start, *end, *radius)),
        ColliderType::CapsuleX { half_height, radius } => Some(Collider::capsule_x(*half_height, *radius)),
        ColliderType::CapsuleY { half_height, radius } => Some(Collider::capsule_y(*half_height, *radius)),
        ColliderType::CapsuleZ { half_height, radius } => Some(Collider::capsule_z(*half_height, *radius)),
        ColliderType::Cuboid { hx, hy, hz } => Some(Collider::cuboid(*hx, *hy, *hz)),
        ColliderType::RoundCuboid { half_x, half_y, half_z, border_radius } =>
            Some(Collider::round_cuboid(*half_x, *half_y, *half_z, *border_radius)),
        ColliderType::Segment { a, b } => Some(Collider::segment(*a, *b)),
        ColliderType::Triangle { a, b, c } => Some(Collider::triangle(*a, *b, *c)),
        ColliderType::RoundTriangle { a, b, c, border_radius } =>
            Some(Collider::round_triangle(*a, *b, *c, *border_radius)),
    }
}

#[derive(Resource)]
pub struct GenRng(WyRand);

impl GenRng {

    pub fn new(seed: u64) -> Self {
        GenRng(WyRand::seed_from_u64(seed)) // Use the appropriate public method to create WyRand
    }

    pub fn rng_mut(&mut self) -> &mut WyRand {
        &mut self.0
    }
}