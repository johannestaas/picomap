//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn download_proprietary_firmware() {
    let status = Command::new("bash")
        .arg("./scripts/download_firmware.sh")
        .status()
        .unwrap();

    if !status.success() {
        panic!("download_firmware.sh failed");
    }
}

fn load_dotenv() {
    let path = Path::new(".env");
    if path.exists() {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let mut line = line.unwrap();

            if let Some(idx) = line.find('#') {
                line.truncate(idx);
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let mut value = value.trim();
                value = value.trim_matches('"').trim_matches('\'');

                println!("cargo:rustc-env={}={}", key, value);
            }
        }
    }
}

fn main() {
    // Need this or it won't re-run with WiFi password changes
    println!("cargo:rerun-if-changed=.env");
    download_proprietary_firmware();
    load_dotenv();
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed=memory.x");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    // Disable if you want USB-CDC and don't want to do SWD debugging.
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
