//! Embeds assets: icons/*.svg -> OUT_DIR/icons.rs, templates/*.json -> OUT_DIR/templates.rs.
use std::{env, fs, path::Path};

fn embed(subdir: &str, ext: &str, static_name: &str, out_file: &str) {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join(subdir);
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == ext).then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    let mut out = format!("pub static {static_name}: &[(&str, &[u8])] = &[\n");
    for n in names {
        let path = dir.join(format!("{n}.{ext}")).display().to_string();
        out += &format!("    ({n:?}, include_bytes!({path:?})),\n");
    }
    out += "];\n";
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join(out_file), out).unwrap();
}

fn main() {
    embed("icons", "svg", "ICONS", "icons.rs");
    embed("templates", "json", "TEMPLATES", "templates.rs");
}
