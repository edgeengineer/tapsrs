//! Build script for Transport Services library
//! Handles platform-specific configuration

use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    println!("cargo:rerun-if-changed=build.rs");

    // Platform-specific configurations
    match target_os.as_str() {
        "ios" => {
            println!("cargo:rustc-link-lib=framework=Foundation");
            println!("cargo:rustc-link-lib=framework=Security");
        }
        "macos" => {
            println!("cargo:rustc-link-lib=framework=Foundation");
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=Network");
        }
        "android" => {
            // Android-specific configurations
            println!("cargo:rustc-link-lib=log");
        }
        "windows" => {
            // Windows-specific configurations
            println!("cargo:rustc-link-lib=ws2_32");
            println!("cargo:rustc-link-lib=userenv");
        }
        _ => {}
    }

    // Architecture-specific configurations
    match target_arch.as_str() {
        "aarch64" => {
            // ARM64-specific optimizations
        }
        "x86_64" => {
            // x86_64-specific optimizations
        }
        _ => {}
    }

    // Generate cbindgen headers if FFI feature is enabled
    #[cfg(feature = "ffi")]
    {
        let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

        cbindgen::Builder::new()
            .with_crate(crate_dir)
            .with_language(cbindgen::Language::C)
            .generate()
            .expect("Unable to generate bindings")
            .write_to_file("transport_services.h");
    }
}
