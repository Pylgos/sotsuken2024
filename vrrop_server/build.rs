use std::env;
use std::path::PathBuf;

fn main() {
    let dst = cmake::build("cpp");
    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-lib=dylib=slam_core");

    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("cpp/slam_core.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        // .parse_callbacks(Box::new(bindgen::CargoCallbacks::)
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
