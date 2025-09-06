use std::any::Any;
use bevy::prelude::*;
use crate::spawning::euler_transform::EulerTransform;

#[typetag::serde]
pub trait Spawnable: Send + Sync {
    fn spawn_event(&self, transform: &EulerTransform, parent: Option<Entity>) -> Box<dyn Any + Send + Sync>;
}
