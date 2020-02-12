extern crate fs_extra;
use fs_extra::dir::CopyOptions;
use std::fmt::Debug;
use std::path::Path;
use std::{env, fs};

const LOG4RS_CONFIG_DIR: &str = "config";

fn main() {
    println!("-----------------------------------");
    let target_dir_path = match env::var("OUT_DIR") {
        Ok(path) => path,
        Err(e) => panic!("OUT_DIR is not set {}", e),
    };
    config_copy(&target_dir_path, LOG4RS_CONFIG_DIR);
}

fn config_copy<S: AsRef<std::ffi::OsStr> + ?Sized, P: Debug + Copy + AsRef<Path>>(
    target_dir_path: &S,
    config_dir: P,
) {
    let mut options = CopyOptions::new();
    options.overwrite = true;
    println!(
        "config_dir:{:?} target:{:?}",
        config_dir,
        Path::new(&target_dir_path).join("../../..").join("config"),
    );
    match fs_extra::dir::copy(
        config_dir,
        Path::new(&target_dir_path).join("../../.."),
        &options,
    ) {
        Ok(u64) => (),
        Err(_) => panic!("Couldn't copy config"),
    };
}
