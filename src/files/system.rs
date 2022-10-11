use std::env;
use std::fs;
use std::process;
use std::path::Path;
use std::path::PathBuf;
use anyhow::{ Context };

pub fn is_managed() -> bool {
    let current_dir = get_current_dir_or_die();

    return get_managed_directory(current_dir.as_path()).is_some();
}

pub fn get_managed_directory_or_die() -> PathBuf {
    let current_dir = get_current_dir_or_die();
    
    if let Some(managed_directory) = get_managed_directory(current_dir.as_path()) {
        return managed_directory;
    }
    eprintln!("Current directory: {} is not managed by minvcs", current_dir.display());
    process::exit(1);
}

pub fn get_managed_directory(path: &Path) -> Option<PathBuf> {
    let system_dir = path.join(".minvcs");
    if system_dir.exists() && system_dir.is_dir() {
        return Some(path.to_path_buf());
    }
    if let Some(parent) = path.parent() {
        return get_managed_directory(parent);
    }
    return None;
}

pub fn get_current_dir_or_die() -> PathBuf {
    match env::current_dir() {
        Ok(current_dir) => {
            current_dir
        },
        Err(e) => {
            eprintln!("Couldn't get current directory: {}", e);
            process::exit(1);
        }
    }
}

pub fn initialize_system_dir(path: &Path) -> anyhow::Result<()> {
    fs::create_dir(path.join(".minvcs").as_path()).context(format!("Couldn't initialize .minvcs directory in {}", path.display()))?;
    Ok(())
}