use std::any::Any;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use crate::spawning::euler_transform::EulerTransform;
use typetag::serde;

#[typetag::serde]
pub trait Spawnable: Send + Sync {
    fn spawn_event(&self, transform: &EulerTransform, parent: Option<Entity>) -> Box<dyn Any + Send + Sync>;
}
