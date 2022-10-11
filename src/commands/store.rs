use crate::files::{system, store::{ObjectManager, StoreableObject}};
use std::path::PathBuf;

pub fn run(path: &PathBuf) -> anyhow::Result<()> {
    let managed_directory = system::get_managed_directory_or_die();
    let objectManager = ObjectManager::new(managed_directory);
    let canonical_path = std::fs::canonicalize(path)?;
    println!("{}", canonical_path.display());
    objectManager.store_object(&StoreableObject::File { path: canonical_path })?;
    Ok(())
}