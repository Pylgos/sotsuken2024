use gl_generator::{Api, Fallbacks, GlobalGenerator, Profile, Registry};
use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let dest = env::var("OUT_DIR").unwrap();
    let mut file = File::create(&Path::new(&dest).join("bindings.rs")).unwrap();

    if target_os == "linux" {
        let extensions = [];
        Registry::new(Api::Gl, (4, 6), Profile::Core, Fallbacks::All, extensions)
            .write_bindings(GlobalGenerator, &mut file)
            .unwrap();
    }

    if target_os == "android" {
        let extensions = ["GL_OES_EGL_image_external"];
        Registry::new(
            Api::Gles2,
            (2, 0),
            Profile::Core,
            Fallbacks::All,
            extensions,
        )
        .write_bindings(GlobalGenerator, &mut file)
        .unwrap();
    }
}
