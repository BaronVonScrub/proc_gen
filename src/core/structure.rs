use crate::spawning::object_logic::Ownership;
use bevy_prng::WyRand;
use crate::spawning::euler_transform::EulerTransform;
use serde::{Serialize, Deserialize};
use rand::prelude::SliceRandom;
use crate::core::structure_key::StructureKey;
use crate::core::structure_reference::StructureReference;

use crate::core::structure_error::StructureError;
use crate::management::structure_management::import_structure;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Structure {
    pub structure_name: String,
    pub tags: Vec<String>,
    pub data: Vec<(StructureKey, EulerTransform)>,
}

impl Structure {
    pub fn create_random_substructure(&self, n: &usize, rng: &mut WyRand) -> Self {
        let selected_data = if *n >= self.data.len() {
            self.data.clone()
        } else {
            self.data
                .choose_multiple(rng, *n)
                .cloned()
                .collect::<Vec<_>>()
        };

        Structure {
            structure_name: format!("{:?} Random Substructure", self.structure_name),
            tags: self.tags.clone(),
            data: selected_data,
        }
    }
}

impl<'a> TryFrom<&'a StructureReference> for Structure {
    type Error = StructureError;

    fn try_from(value: &'a StructureReference) -> Result<Self, Self::Error> {
        match value {
            StructureReference::Raw { structure, ownership } => {
                let mut cloned_structure = structure.as_ref().clone();
                if let Ownership::Team(team_id) = ownership {
                    propagate_team_ownership(&mut cloned_structure, *team_id);
                } else if let Ownership::Inherit = ownership {
                    return Err(StructureError::InheritOwnershipAtTopLevel(
                        format!("Inherit ownership cannot be used at the top level for structure '{:?}'", structure),
                    ));
                }

                Ok(cloned_structure)
            },
            StructureReference::Ref { structure, ownership } => {
                let mut imported_structure = import_structure(structure.clone())
                    .map_err(|e| StructureError::ImportFailed(format!("Failed to import structure '{}': {}", structure, e)))?;
                if let Ownership::Team(team_id) = ownership {
                    propagate_team_ownership(&mut imported_structure, *team_id);
                } else if let Ownership::Inherit = ownership {
                    return Err(StructureError::InheritOwnershipAtTopLevel(
                        format!("Inherit ownership cannot be used at the top level for structure '{}'", structure),
                    ));
                }

                Ok(imported_structure)
            }
        }
    }

}

pub(crate) fn propagate_team_ownership(structure: &mut Structure, team_id: u8) {
    for (key, _transform) in &mut structure.data {
        match key {
            StructureKey::Nest(reference) |
            StructureKey::Choose { list: reference } |
            StructureKey::ChooseSome { list: reference, .. } |
            StructureKey::Rand { reference, .. } |
            StructureKey::ProbabilitySpawn { reference, .. } |
            StructureKey::Loop { reference, .. } |
            StructureKey::NestingLoop { reference, .. } |
            StructureKey::NoiseSpawn { reference, .. } |
            StructureKey::PathSpawn { reference, .. } |
            StructureKey::PathToTag { reference, .. } |
            StructureKey::RandDistDir { reference, .. } |
            StructureKey::Reflection { reference, .. } => {
                update_ownership(reference, team_id);
            }
            StructureKey::Object { ownership, .. } => {
                if let Ownership::Inherit = ownership {
                    *ownership = Ownership::Team(team_id);
                }
            }
            _ => {}
        }
    }
}

fn update_ownership(reference: &mut StructureReference, team_id: u8) {
    match reference {
        StructureReference::Raw { ownership, structure } => {
            if let Ownership::Inherit = ownership {
                *ownership = Ownership::Team(team_id);
            }
            propagate_team_ownership(structure, team_id);
        }
        StructureReference::Ref { ownership, .. } => {
            if let Ownership::Inherit = ownership {
                *ownership = Ownership::Team(team_id);
            }
        }
    }
}
