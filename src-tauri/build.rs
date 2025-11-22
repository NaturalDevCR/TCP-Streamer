fn main() {
    tauri_build::build();

    // Force static linking for FLAC and ogg on Windows
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=static=FLAC");
        println!("cargo:rustc-link-lib=static=ogg");

        // Ensure Rust searches in the correct vcpkg directories
        if let Ok(vcpkg_root) = std::env::var("VCPKG_ROOT") {
            let target = std::env::var("VCPKG_DEFAULT_TRIPLET")
                .unwrap_or_else(|_| "x64-windows-static".to_string());

            let lib_path = format!("{}/installed/{}/lib", vcpkg_root, target);
            println!("cargo:rustc-link-search=native={}", lib_path);
        }
    }
}
