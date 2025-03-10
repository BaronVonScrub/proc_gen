#[derive(Debug)]
pub enum StructureError {
    CycleDetected(String),
    ImportFailed(String),
    Other(String),
    InheritOwnershipAtTopLevel(String),
}

impl From<&str> for StructureError {
    fn from(error: &str) -> Self {
        StructureError::Other(error.to_string())
    }
}
