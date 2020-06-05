extern crate reqwest;

use std::path::PathBuf;
use log::{error, info, trace, warn};
use anyhow::{anyhow, Context, Result};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;


fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub fn request_file(url : String) -> Result<PathBuf> {
    let string_hash = calculate_hash(&url).to_string();
    let mut file_path = std::env::temp_dir();
    file_path.push(string_hash);

    if file_path.exists() {
        return Ok(file_path);
    }

    let resp = reqwest::blocking::get(&url)?;
    if resp.status().is_success() {
        std::fs::write(&file_path, resp.bytes().unwrap())?;
        return Ok(file_path);
    } else {
        Err(anyhow!("request failed"))
    }
}