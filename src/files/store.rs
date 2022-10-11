use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

static EXCLUDE_MINVCS: &str = ".minvcs";

pub enum StoreableObject {
    File { path: PathBuf }
}

pub struct ObjectManager {
    root_dir: PathBuf,
}

impl ObjectManager {
    pub fn new(root_dir: PathBuf) -> ObjectManager {
        ObjectManager { root_dir }
    }

    pub fn get_object_dir(&self) -> PathBuf {
        self.root_dir.join(".minvcs").join("objects")
    }

    pub fn store_object(&self, object: &StoreableObject) -> anyhow::Result<()> {
        match object {
            StoreableObject::File { path } => {
                let mut exclude_list: HashSet<PathBuf> = HashSet::new();
                let minvcs_path = self.root_dir.join(EXCLUDE_MINVCS);
                exclude_list.insert(minvcs_path);
                self.store_file(path.as_path(), &mut exclude_list)?
            },
        };
        Ok(())
    }

    fn store_file(&self, path: &Path, exclude_list: &mut HashSet<PathBuf>) -> anyhow::Result<String> {
        if !path.exists() {
            return Err(anyhow::anyhow!(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }
        if !path.starts_with(self.root_dir.as_path()) {
            return Err(anyhow::anyhow!(format!(
                "Path is not managed by the repo: {}",
                path.display()
            )));
        }
        let (body, header) = if path.is_dir() {
            ObjectManager::read_excludes(path, exclude_list)?;

            let mut children: Vec<(String, String)> = Vec::new();
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if let Some(file_name) = entry.file_name().to_str() {
                    let child_path = entry.path();
                    if exclude_list.contains(child_path.as_path()) {
                        println!("Excluded {}", file_name);
                        continue;
                    }
                    let child = (
                        file_name.to_string(),
                        self.store_file(child_path.as_path(), exclude_list)?);
                    children.push(child);
                } else {
                    println!("Only Unicode file name is supported. Excluded {}", entry.path().display());
                    continue;
                }
            }
            let body = ObjectManager::create_dir_body(children)?;
            let header = format!("directory {}\0", body.len());
            (body, header)
        } else if path.is_file() {
            let body = std::fs::read(path)?;
            let header = format!("file {}\0", body.len());
            (body, header)
        } else {
            return Err(anyhow::anyhow!(format!("Encountered path which is not dir or file: {}", path.display())));
        };

        let content = [header.as_bytes(), &body[..]].concat();
        let mut sha256 = Sha256::new();
        sha256.update(&content);
        let hash = sha256.finalize();
        let hash_string: String = hash
            .iter()
            .map(|n| format!("{:02x}", n))
            .collect::<String>();
        println!("File: {}, Hash: {}", path.display(), hash_string);

        let store_dir = self.get_object_dir().join(&hash_string[..2]);

        std::fs::create_dir_all(store_dir.as_path())?;
        let file = File::create(store_dir.join(&hash_string[2..]))?;
        let mut encoder = ZlibEncoder::new(file, Compression::fast());
        encoder.write_all(&content)?;
        encoder.finish()?;
        Ok(hash_string)
    }

    fn create_dir_body(mut children: Vec<(String, String)>) -> anyhow::Result<Vec<u8>> {
        let mut result: Vec<u8> = Vec::new();
        children.sort();
        for (file_name, hash) in children {
            result.write_all(format!("{} {}\n", hash, file_name).as_bytes())?;
        }
        Ok(result)
    }

    fn read_excludes(path: &Path, exclude_list: &mut HashSet<PathBuf>) -> anyhow::Result<()> {
        let excludes_path = path.join(".minvcs_excludes");
        if excludes_path.exists() {
            println!("Found {}", excludes_path.display());
            for line in fs::read_to_string(excludes_path)?.lines() {
                let exclude_item = line.trim();
                if !exclude_item.is_empty() {
                    let exclude_path = path.join(line.trim());
                    println!("Excludes {}", exclude_path.display());
                    exclude_list.insert(exclude_path);
                }
            }
        }
        Ok(())
    }
}
