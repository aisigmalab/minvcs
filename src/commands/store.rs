use crate::files::{system, store::ObjectManager};
use std::path::PathBuf;

pub fn run(path: &PathBuf) -> anyhow::Result<()> {
    let managed_directory = system::get_managed_directory_or_die();
    let object_manager = ObjectManager::new(managed_directory);
    let canonical_path = std::fs::canonicalize(path)?;
    println!("{}", canonical_path.display());
    object_manager.store_path(canonical_path.as_path())?;
    Ok(())
}