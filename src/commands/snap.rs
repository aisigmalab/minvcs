use crate::files::system;
use crate::files::store::{ObjectManager, SnapshotMetadata};

pub fn run() -> anyhow::Result<()> {
    let managed_directory = system::get_managed_directory_or_die();
    let object_manager = ObjectManager::new(managed_directory);
    let author = "author".to_string();
    let comment = "test comment".to_string();
    object_manager.snapshot(&SnapshotMetadata {author, comment})?;
    Ok(())
}