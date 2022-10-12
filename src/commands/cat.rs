use crate::files::{system, store::{ObjectManager}};

pub fn run(hash: &str) -> anyhow::Result<()> {
    let managed_directory = system::get_managed_directory_or_die();
    let object_manager = ObjectManager::new(managed_directory);
    print!("{}", object_manager.retrieve_object(hash)?);
    Ok(())
}