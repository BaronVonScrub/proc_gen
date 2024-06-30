use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SeededOrNot {
    Seeded(u64),
    Unseeded,
}