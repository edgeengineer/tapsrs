use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let config = cbindgen::Config::from_file("cbindgen.toml")
        .expect("Unable to find cbindgen.toml");

    let header_path = PathBuf::from("target").join("taps.h");

    cbindgen::Builder::new()
      .with_crate(crate_dir)
      .with_config(config)
      .generate()
      .expect("Unable to generate bindings")
      .write_to_file(header_path);
} 