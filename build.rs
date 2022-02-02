use std::path::Path;
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=soundboard.toml");
    println!("cargo:rerun-if-changed=soundboards");

    let target_dir_path = Path::new(&env::var("OUT_DIR").unwrap())
        .join("..")
        .join("..")
        .join("..");
    copy_file(&target_dir_path, "soundboard.toml");

    let out_soundboards_path = Path::new(&target_dir_path).join("soundboards");
    if out_soundboards_path.exists() && out_soundboards_path.is_dir() {
        std::fs::remove_dir_all(&out_soundboards_path).unwrap();
    }
    let copy_options = fs_extra::dir::CopyOptions::new();
    if Path::new("soundboards").exists() {
        fs_extra::dir::copy("soundboards", target_dir_path, &copy_options).expect("copy failed");
    }

    tonic_build::compile_protos("src/download/ttsclient/cloud_tts.proto")?;
    Ok(())
}

fn copy_file<S: AsRef<std::ffi::OsStr> + ?Sized, P: Copy + AsRef<Path>>(
    target_dir_path: &S,
    file_name: P,
) {
    fs::copy(file_name, Path::new(&target_dir_path).join(file_name)).unwrap();
}
