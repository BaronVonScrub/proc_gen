use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RandData {
    Linear(f32),
    Gaussian(f32),
}