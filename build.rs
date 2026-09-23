use std::{env, fs, io, path::{Path, PathBuf}};

fn concat_parts(source_dir: &Path, output: &Path) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(source_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect();
    entries.sort();

    let mut combined = String::new();
    for path in entries {
        println!("cargo:rerun-if-changed={}", path.display());
        combined.push_str(&fs::read_to_string(path)?);
    }
    fs::write(output, combined)
}

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    println!("cargo:rerun-if-changed=src/parts/modem_manager");
    println!("cargo:rerun-if-changed=src/parts/ui");

    concat_parts(
        &manifest.join("src/parts/modem_manager"),
        &out.join("modem_manager_impl.rs"),
    )?;
    concat_parts(
        &manifest.join("src/parts/ui"),
        &out.join("ui_impl.rs"),
    )?;

    // ui_impl.rs is generated inside OUT_DIR, so legacy relative include_str!
    // paths resolve one directory above OUT_DIR. Mirror the two embedded UI
    // resources there to keep generated-source compilation deterministic.
    let generated_root = out
        .parent()
        .expect("OUT_DIR must have a parent directory");
    fs::copy(manifest.join("CHANGELOG.md"), generated_root.join("CHANGELOG.md"))?;
    fs::create_dir_all(generated_root.join("assets"))?;
    fs::copy(
        manifest.join("assets/style.css"),
        generated_root.join("assets/style.css"),
    )?;

    println!("cargo:rerun-if-changed=CHANGELOG.md");
    println!("cargo:rerun-if-changed=assets/style.css");
    Ok(())
}
