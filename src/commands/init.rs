use crate::files::system::{is_managed, get_current_dir_or_die, initialize_system_dir};

pub fn run() -> anyhow::Result<()> {
    if !is_managed() {
        let current_dir = get_current_dir_or_die();
        initialize_system_dir(current_dir.as_path())?;
        println!("Successfully initialized repository");
    } else {
        println!("This directory is already managed by minvcs");
    }
    Ok(())
}