use crate::files::system;

pub fn run() -> anyhow::Result<()> {
    let managed_directory = system::get_managed_directory_or_die();
    Ok(())
}