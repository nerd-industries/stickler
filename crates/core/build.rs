//! Embeds every SVG in assets/icons as (name, bytes) pairs.
use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/icons");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "svg").then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    let mut out = String::from("pub static ICONS: &[(&str, &[u8])] = &[\n");
    for n in names {
        out += &format!("    ({n:?}, include_bytes!({:?})),\n", dir.join(format!("{n}.svg")).display().to_string());
    }
    out += "];\n";
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("icons.rs"), out).unwrap();
}
