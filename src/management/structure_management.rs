use std::collections::HashMap;
use std::sync::Mutex;
use std::fs::File;
use ron::de::{from_reader, SpannedError};
use lazy_static::lazy_static;
use crate::core::structure::Structure;

lazy_static! {
    static ref STRUCTURE_CACHE: Mutex<HashMap<String, Structure>> = Mutex::new(HashMap::new());
}

pub fn import_structure(structure_name: String) -> Result<Structure, ron::Error> {
    let mut cache = STRUCTURE_CACHE.lock().unwrap();

    if let Some(cached_structure) = cache.get(&structure_name) {
        return Ok(cached_structure.clone());
    }

    let file_path = format!("assets/structures/{}.arch", structure_name);
    let file = File::open(&file_path)?;
    let deserialized: Result<Structure, SpannedError> = from_reader(file);

    match deserialized {
        Ok(structure) => {
            cache.insert(structure_name.clone(), structure.clone());
            Ok(structure)
        }
        Err(e) => Err(e.into())
    }
}
