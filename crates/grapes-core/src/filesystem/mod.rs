use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::error::Result;

pub mod identity;
pub mod validation;

pub use identity::{file_identity, mtime_ns};

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif", "avif", "ari",
    "arw", "cr2", "cr3", "crm", "crw", "dcr", "dcs", "dng", "erf", "fff", "iiq", "kdc", "mef",
    "mos", "mrw", "nef", "nrw", "orf", "ori", "pef", "raf", "raw", "rw2", "rwl", "srw", "x3f",
    "3fr", "qtk",
];

#[derive(Debug, Clone)]
pub struct ScannedFolder {
    pub relative_path: String,
    pub parent_path: Option<String>,
    pub name: String,
    pub fs_file_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScannedAsset {
    pub relative_path: String,
    pub folder_path: String,
    pub filename: String,
    pub fs_file_id: Option<String>,
    pub file_size: i64,
    pub mtime_ns: i64,
    pub physical_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct ScanOutput {
    pub folders: Vec<ScannedFolder>,
    pub assets: Vec<ScannedAsset>,
    pub warnings: Vec<String>,
}

pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
        .unwrap_or(false)
}

pub fn scan_workspace(root: &Path) -> Result<ScanOutput> {
    let mut output = ScanOutput::default();
    let mut visited = HashSet::new();
    let grapes_dir = root.join(".grapes");
    scan_directory(root, "", 0, &grapes_dir, &mut visited, &mut output)?;

    output
        .folders
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    output
        .assets
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

fn scan_directory(
    directory: &Path,
    relative_path: &str,
    depth: usize,
    grapes_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    output: &mut ScanOutput,
) -> Result<()> {
    let canonical = match fs::canonicalize(directory) {
        Ok(path) => path,
        Err(error) => {
            output
                .warnings
                .push(format!("无法规范化目录 {}：{error}", directory.display()));
            return Ok(());
        }
    };
    if !visited.insert(canonical) {
        output
            .warnings
            .push(format!("跳过循环目录：{}", directory.display()));
        return Ok(());
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            output
                .warnings
                .push(format!("无法读取目录 {}：{error}", directory.display()));
            return Ok(());
        }
    };

    let mut entries = entries
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry),
            Err(error) => {
                output.warnings.push(format!("读取目录项失败：{error}"));
                None
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if depth == 0 && path == grapes_dir {
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                output
                    .warnings
                    .push(format!("无法读取类型 {}：{error}", path.display()));
                continue;
            }
        };
        if file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        let child_relative = if relative_path.is_empty() {
            name.clone()
        } else {
            format!("{relative_path}/{name}")
        };

        if file_type.is_dir() {
            match fs::metadata(&path) {
                Ok(_) => output.folders.push(ScannedFolder {
                    relative_path: child_relative.clone(),
                    parent_path: Some(relative_path.to_owned()),
                    name,
                    fs_file_id: file_identity(&path),
                }),
                Err(error) => {
                    output
                        .warnings
                        .push(format!("无法读取元数据 {}：{error}", path.display()));
                    continue;
                }
            }
            scan_directory(
                &path,
                &child_relative,
                depth + 1,
                grapes_dir,
                visited,
                output,
            )?;
        } else if file_type.is_file() && is_image_file(&path) {
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    output
                        .warnings
                        .push(format!("无法读取元数据 {}：{error}", path.display()));
                    continue;
                }
            };
            output.assets.push(ScannedAsset {
                relative_path: child_relative,
                folder_path: relative_path.to_owned(),
                filename: name,
                fs_file_id: file_identity(&path),
                file_size: metadata.len().min(i64::MAX as u64) as i64,
                mtime_ns: mtime_ns(&metadata),
                physical_path: path,
            });
        }
    }

    Ok(())
}

pub fn hash_file(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().as_bytes().to_vec())
}

pub fn hash_to_hex(hash: &[u8]) -> String {
    let mut output = String::with_capacity(hash.len() * 2);
    for byte in hash {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_extension_matching_is_case_insensitive() {
        assert!(is_image_file(Path::new("DSC001.JPG")));
        assert!(is_image_file(Path::new("photo.heic")));
        assert!(!is_image_file(Path::new("notes.txt")));
    }

    #[test]
    fn scan_skips_sidecar_and_symlinks() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir(temp.path().join(".grapes")).expect("sidecar");
        fs::write(temp.path().join(".grapes").join("index.jpg"), b"ignored")
            .expect("ignored image");
        fs::create_dir(temp.path().join("Travel")).expect("folder");
        fs::write(temp.path().join("Travel").join("photo.jpg"), b"photo").expect("photo");

        let output = scan_workspace(temp.path()).expect("scan");
        assert_eq!(output.folders.len(), 1);
        assert_eq!(output.assets.len(), 1);
        assert_eq!(output.assets[0].relative_path, "Travel/photo.jpg");
    }
}
