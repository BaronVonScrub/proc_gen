use serde::{Serialize, Deserialize};
use crate::core::structure::Structure;
use crate::spawning::object_logic::Ownership;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StructureReference {
    Raw {
        structure: Box<Structure>,
        ownership: Ownership,
    },
    Ref {
        structure: String,
        ownership: Ownership,
    },
}
