fn main() {
    println!("cargo:rerun-if-changed=ui/");
    println!("cargo:rerun-if-changed=CHANGELOG.md");
    slint_build::compile("ui/main.slint").expect("Slint build failed");
}
