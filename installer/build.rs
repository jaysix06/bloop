//! Embeds the release `bloop.exe` into the installer and gives the
//! installer Bloop's icon. Build the app first: `scripts\package.ps1`.
use image::{ImageBuffer, Rgba};
use std::path::PathBuf;

include!("../icon_art.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../icon_art.rs");
    println!("cargo:rerun-if-env-changed=BLOOP_PAYLOAD");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let payload = std::env::var_os("BLOOP_PAYLOAD")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../target/release/bloop.exe"));
    let payload = payload.canonicalize().unwrap_or_else(|_| {
        panic!(
            "{} not found: build Bloop first with `cargo build --release -p bloop` (or run scripts/package.ps1)",
            payload.display()
        )
    });
    println!("cargo:rerun-if-changed={}", payload.display());
    println!("cargo:rustc-env=BLOOP_PAYLOAD={}", payload.display());

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = write_icon(&out);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico.to_str().unwrap());
        res.set("FileDescription", "Bloop Setup");
        res.set("ProductName", "Bloop");
        res.compile().unwrap();
    }
}
