use serde::{Serialize, Deserialize};
use bevy::prelude::*;
use bevy_rapier3d::prelude::Collider;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ColliderBehaviour {
    Dynamic,
    GenerationDynamic,
    Kinematic,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ColliderInfo {
    pub collider_type: ColliderType,
    pub priority: i8,
    pub behaviour: ColliderBehaviour,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ColliderType {
    None,
    Ball {
        radius: f32,
    },
    Cylinder {
        half_height: f32,
        radius: f32,
    },
    RoundCylinder {
        half_height: f32,
        radius: f32,
        border_radius: f32,
    },
    Cone {
        half_height: f32,
        radius: f32,
    },
    RoundCone {
        half_height: f32,
        radius: f32,
        border_radius: f32,
    },
    Capsule {
        start: Vec3,
        end: Vec3,
        radius: f32,
    },
    CapsuleX {
        half_height: f32,
        radius: f32,
    },
    CapsuleY {
        half_height: f32,
        radius: f32,
    },
    CapsuleZ {
        half_height: f32,
        radius: f32,
    },
    Cuboid {
        hx: f32,
        hy: f32,
        hz: f32,
    },
    RoundCuboid {
        half_x: f32,
        half_y: f32,
        half_z: f32,
        border_radius: f32,
    },
    Segment {
        a: Vec3,
        b: Vec3,
    },
    Triangle {
        a: Vec3,
        b: Vec3,
        c: Vec3,
    },
    RoundTriangle {
        a: Vec3,
        b: Vec3,
        c: Vec3,
        border_radius: f32,
    },
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
