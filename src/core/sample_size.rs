use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SampleSize {
    UUniDim(i32),
    UBiDim(i32),
    UTriDim(i32),
    UniDim(i32),
    BiDim(i32, i32),
    TriDim(i32, i32, i32),
}