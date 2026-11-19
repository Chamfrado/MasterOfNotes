fn main() {
    // Embed the Common-Controls v6 manifest through the linker so it reaches every binary,
    // including `cargo test` executables (tauri-build's default only covers the app .exe,
    // and tests using tauri::test die with STATUS_ENTRYPOINT_NOT_FOUND without it).
    let target = std::env::var("TARGET").unwrap_or_default();
    let mut windows = tauri_build::WindowsAttributes::new();
    if target.ends_with("windows-msvc") {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri build script");
}
