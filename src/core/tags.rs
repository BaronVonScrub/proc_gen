use bevy::prelude::*;
use serde::{Serialize, Deserialize};
use bevy_inspector_egui::prelude::*;
use std::slice::Iter;

#[derive(Reflect, Component, Default, InspectorOptions, Debug, Clone)]
#[reflect(Component, InspectorOptions)]
pub struct Tags(pub Vec<String>);

impl Tags {
    pub fn iter(&self) -> Iter<'_, String> {
        self.0.iter()
    }

    pub fn contains(&self, tag: &str) -> bool {
        self.0.contains(&tag.to_string())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a> IntoIterator for &'a Tags {
    type Item = &'a String;
    type IntoIter = Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
