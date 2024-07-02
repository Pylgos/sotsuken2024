use std::{ops::Deref, sync::OnceLock};

use godot::prelude::*;

mod binding;
mod point_cloud_mesh;

static TOKIO_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

#[derive(Debug)]
struct SharedGd<T: GodotClass>(Gd<T>);
unsafe impl<T: GodotClass> Send for SharedGd<T> {}
unsafe impl<T: GodotClass> Sync for SharedGd<T> {}
impl<T: GodotClass> Deref for SharedGd<T> {
    type Target = Gd<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Core {
            godot_print!("Loading VRROP server extension...");
            let runtime = tokio::runtime::Runtime::new().unwrap();
            TOKIO_RUNTIME.set(runtime).unwrap();
        }
    }
}
