//! Draws Bloop's icon at build time (see icon_art.rs) and embeds it in the
//! executable, so the repo carries no binary art.
use image::{ImageBuffer, Rgba};
use std::path::PathBuf;

include!("icon_art.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=icon_art.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = write_icon(&out);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico.to_str().unwrap());
        res.set("FileDescription", "Bloop");
        res.set("ProductName", "Bloop");
        res.compile().unwrap();
    }
}
