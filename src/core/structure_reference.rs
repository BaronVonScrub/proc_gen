use serde::{Serialize, Deserialize};
use crate::proc_gen::core::structure::Structure;
use crate::proc_gen::spawning::object_logic::Ownership;

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
