use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub file_name: String,
    pub file_size: u64,
    pub chunk_size: u32,
    pub chunk_hashes: Vec<String>,
    pub file_id: String,
}

pub fn compute_file_id(file_name: &str, file_size: u64, chunk_size: u32) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(format!("{}:{}:{}", file_name, file_size, chunk_size).as_bytes());
    hasher.finalize().to_hex().to_string()
}

pub fn create_manifest(file_path: &Path, chunk_size: u32) -> Result<Manifest> {
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid file name for {:?}", file_path))?
        .to_string();

    let mut file = File::open(file_path)
        .with_context(|| format!("Failed to open file {:?}", file_path))?;
    let file_size = file.metadata()?.len();

    let mut chunk_hashes = Vec::new();
    let mut buffer = vec![0u8; chunk_size as usize];

    if file_size == 0 {
        let hash = blake3::hash(&[]);
        chunk_hashes.push(hash.to_hex().to_string());
    } else {
        loop {
            let mut read_bytes = 0;
            while read_bytes < chunk_size as usize {
                let n = file.read(&mut buffer[read_bytes..])?;
                if n == 0 {
                    break;
                }
                read_bytes += n;
            }
            if read_bytes == 0 {
                break;
            }
            let hash = blake3::hash(&buffer[..read_bytes]);
            chunk_hashes.push(hash.to_hex().to_string());
        }
    }

    let file_id = compute_file_id(&file_name, file_size, chunk_size);

    Ok(Manifest {
        file_name,
        file_size,
        chunk_size,
        chunk_hashes,
        file_id,
    })
}
