use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha2::Digest;
use sha2::Sha256;
use tempfile::NamedTempFile;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

static EXCLUDE_MINVCS: &str = ".minvcs";

pub type Hash = String;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DirectoryNode {
    name: String,
    hash: String,
}

pub struct SnapshotMetadata {
    pub author: String,
    pub comment: String,
    pub parents: Vec<String>,
}

pub enum StoredObject {
    File { body: Vec<u8>, hash: Hash },
    Directory { children: Vec<DirectoryNode>, hash: Hash },
    Snapshot { body: Vec<u8>, hash: Hash }, // for now
}

impl fmt::Display for SnapshotMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "author:{}\n", self.author)?;
        for parent in &self.parents {
            write!(f, "parent:{}\n", parent)?;
        }
        write!(f, "\n{}", self.comment)
    }
}

impl fmt::Display for DirectoryNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.hash, self.name)
    }
}

impl fmt::Display for StoredObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use StoredObject::*;
        match *self {
            File {ref body, ref hash} => {
                write!(f, "File {}\n{}\n", hash, String::from_utf8_lossy(body))
            },
            Directory {ref children, ref hash} => {
                write!(f, "Directory {}\n", hash)?;
                for child in children {
                    write!(f, "{}\n", child)?
                }
                Ok(())
            },
            Snapshot {ref body, ref hash} => {
                write!(f, "Snapshot {}\n{}\n", hash, String::from_utf8_lossy(body))
            },
        }

    }
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

    pub fn get_object_file_path<'a>(&self, hash: &'a str) -> (PathBuf, &'a str) {
        (self.get_object_dir().join(&hash[..2]), &hash[2..])
    }

    pub fn get_head_file_path(&self) -> PathBuf {
        self.root_dir.join(".minvcs").join("head")
    }

    pub fn get_head(&self) -> anyhow::Result<Option<Hash>> {
        let head_file_path = self.get_head_file_path();
        if !head_file_path.exists() {
            return Ok(None);
        }
        let mut head_file = File::open(head_file_path)?;
        let mut hash = String::new();
        head_file.read_to_string(&mut hash)?;
        return Ok(Some(hash));
    }

    pub fn store_path(&self, path: &Path) -> anyhow::Result<String> {
        let mut exclude_list: HashSet<PathBuf> = HashSet::new();
        let minvcs_path = self.root_dir.join(EXCLUDE_MINVCS);
        exclude_list.insert(minvcs_path);
        self.store_file_tree(path, &mut exclude_list)
    }

    pub fn snapshot(&self, metadata: &SnapshotMetadata) -> anyhow::Result<()> {
        let root_hash = self.store_path(&self.root_dir)?;
        let snapshot_hash = self.store_snapshot(&root_hash, metadata)?;
        self.move_head(&snapshot_hash)
    }

    pub fn retrieve_object(&self, hash: &str) -> anyhow::Result<StoredObject> {
        let (store_dir, store_file_name) = self.get_object_file_path(hash);
        let object_file_path = store_dir.join(store_file_name);
        let compressed_object = fs::read(object_file_path.as_path())?;
        let mut object = Vec::new();
        let mut decoder = ZlibDecoder::new(&compressed_object[..]);
        decoder.read_to_end(&mut object)?;
        let mut sha256 = Sha256::new();
        sha256.update(&object);
        let object_hash = sha256.finalize();
        let object_hash_string: String = object_hash.iter()
                .map(|n| format!("{:02x}", n))
                .collect::<String>();
        if hash != object_hash_string {
            return Err(anyhow::anyhow!(format!("Object file is invalid (Mismatching hash): {}", object_hash_string)))
        }
        if let Some(null_idx) = object.iter().position(|&x| x == 0) {
            let header = String::from_utf8(object[0..null_idx].to_vec())?;
            let body = &object[null_idx + 1..];
            let object_type = header.split(" ").next();
            return match object_type {
                Some("file") => {
                    Ok(StoredObject::File { body: body.to_vec(), hash: object_hash_string })
                },
                Some("directory") => {
                    let mut children = Vec::new();
                    for line in String::from_utf8(body.to_vec())?.lines() {
                        if let Some((child_hash, child_file_name)) = line.split_once(" ") {
                            children.push(DirectoryNode { name: child_file_name.to_string(), hash: child_hash.to_string() });
                        } else {
                            return Err(anyhow::anyhow!(format!("Object file is invalid (directory): {}", object_file_path.display())))
                        }
                    }
                    Ok(StoredObject::Directory { children, hash: object_hash_string })
                },
                Some("snapshot") => {
                    Ok(StoredObject::Snapshot { body: body.to_vec(), hash: object_hash_string })
                },
                Some(_) => {
                    Err(anyhow::anyhow!(format!("Object file is invalid (Unknown object type): {}", object_file_path.display())))
                }
                None => {
                    Err(anyhow::anyhow!(format!("Object file is invalid (No type in header): {}", object_file_path.display())))
                }
            }
        } else {
            Err(anyhow::anyhow!(format!("Object file is invalid (No header): {}", object_file_path.display())))
        }
    }

    /// Stores all files under the given path except the ones in exclude_list
    /// and returns the hash of the object corresponding to the given path
    fn store_file_tree(&self, path: &Path, exclude_list: &mut HashSet<PathBuf>) -> anyhow::Result<String> {
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
                        self.store_file_tree(child_path.as_path(), exclude_list)?);
                    children.push(child);
                } else {
                    println!("Only Unicode file name is supported. Excluded {}", entry.path().display());
                    continue;
                }
            }
            let body = ObjectManager::create_dir_body(&mut children)?;
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
        let hash_string = self.store_binary_compressed(&content)?;
        println!("File: {}, Hash: {}", path.display(), hash_string);
        Ok(hash_string)
    }

    /// Take binary as &Vec<u8>, calculate hash, and store it as a proper file name with zlib compression
    fn store_binary_compressed(&self, content: &Vec<u8>) -> anyhow::Result<String> {
        let mut sha256 = Sha256::new();
        sha256.update(&content);
        let hash = sha256.finalize();
        let hash_string: String = hash
            .iter()
            .map(|n| format!("{:02x}", n))
            .collect::<String>();

        let (store_dir, store_file_name) = self.get_object_file_path(&hash_string);

        std::fs::create_dir_all(store_dir.as_path())?;
        let file = File::create(store_dir.join(store_file_name))?;
        let mut encoder = ZlibEncoder::new(file, Compression::fast());
        encoder.write_all(&content)?;
        encoder.finish()?;
        Ok(hash_string)
    }

    pub fn move_head(&self, hash: &str) -> anyhow::Result<()> {
        let temp_head_file = NamedTempFile::new()?;
        write!(temp_head_file.as_file(), "{}", hash)?;
        fs::copy(temp_head_file.path(), self.get_head_file_path())?;
        println!("Head successfully updated to {}", hash);
        Ok(())
    }

    fn store_snapshot(&self, hash: &str, metadata: &SnapshotMetadata) -> anyhow::Result<String> {
        let body = format!("{}\n{}", hash, metadata);
        let header = format!("snapshot {}\0", body.as_bytes().len());
        let content = [header.as_bytes(), body.as_bytes()].concat();
        let hash_string = self.store_binary_compressed(&content)?;
        println!("Snapshot Root: {}, Snapshot Hash: {}", hash, hash_string);
        Ok(hash_string)
    }

    fn create_dir_body(children: &mut Vec<(String, String)>) -> anyhow::Result<Vec<u8>> {
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
