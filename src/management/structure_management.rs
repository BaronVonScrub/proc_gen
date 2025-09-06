use std::collections::HashMap;
use std::sync::Mutex;
use std::fs::File;
use std::path::{PathBuf};
use ron::de::{from_reader, SpannedError};
use lazy_static::lazy_static;
use crate::core::structure::Structure;

lazy_static! {
    static ref STRUCTURE_CACHE: Mutex<HashMap<String, Structure>> = Mutex::new(HashMap::new());
}

/// Normalize a structure name like "Castle/castle_with_doors" into a path
/// by splitting on both '/' and '\\' and appending the .arch extension.
fn normalized_structure_relpath(structure_name: &str) -> PathBuf {
    let mut pb = PathBuf::new();
    for seg in structure_name.split(|c| c == '/' || c == '\\') {
        if !seg.is_empty() {
            pb.push(seg);
        }
    }
    pb.set_extension("arch");
    pb
}

pub fn import_structure(structure_name: String) -> Result<Structure, ron::Error> {
    let mut cache = STRUCTURE_CACHE.lock().unwrap();

    if let Some(cached_structure) = cache.get(&structure_name) {
        return Ok(cached_structure.clone());
    }

    // Determine candidate roots for assets, depending on where the app was launched from
    let base_path = std::env::current_dir().unwrap();
    eprintln!("[structure_management] CWD: {}", base_path.display());
    let candidates = [
        base_path.join("assets/structures"),
        base_path.join("tests/assets/structures"),
        base_path.join("tests/imported_assets/Default/structures"),
    ];
    let rel = normalized_structure_relpath(&structure_name);

    // Find first existing file among candidates
    let mut chosen_path = None;
    for root in candidates.iter() {
        let candidate = root.join(&rel);
        eprintln!("[structure_management] Trying: {} -> exists: {}", candidate.display(), candidate.exists());
        if candidate.exists() {
            chosen_path = Some(candidate);
            break;
        }
    }

    let file_path = if let Some(path) = chosen_path {
        eprintln!("[structure_management] Using structure file: {}", path.display());
        path
    } else {
        let tried_paths = candidates
            .iter()
            .map(|root| root.join(&rel))
            .collect::<Vec<_>>();
        for p in &tried_paths {
            eprintln!("❌ ERROR: Structure file NOT FOUND: {}", p.display());
        }
        let tried_joined = tried_paths
            .iter()
            .map(|p| format!("{}", p.display()))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ron::Error::Message(format!(
            "File not found in any candidate paths for structure '{}': [{}]",
            structure_name, tried_joined
        )));
    };

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
