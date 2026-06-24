fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("Missing target_os");
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").expect("Missing target_env");

    if target_os == "windows" && target_env == "msvc" {
        const MANIFEST: &str = "manifest.xml";
        let manifest_path = std::fs::canonicalize(MANIFEST).unwrap();

        println!("cargo::rerun-if-changed={MANIFEST}");
        println!("cargo::rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo::rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest_path.to_str().expect("Manifest path is not utf8")
        );
    }
}
