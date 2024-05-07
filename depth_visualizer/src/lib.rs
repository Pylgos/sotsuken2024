use godot::prelude::*;
use once_cell::sync::Lazy;

mod decoder;
mod gd_visualizer;
mod visualizer;

static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Runtime::new().unwrap());

struct VisualizerExtensionLibrary;

#[gdextension]
unsafe impl ExtensionLibrary for VisualizerExtensionLibrary {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Core => {
                gstreamer::log::set_default_threshold(gstreamer::DebugLevel::Info);
                gstreamer::log::set_active(true);
                #[cfg(target_os = "android")]
                setup_logging();
                gstreamer::init().unwrap();
                let _ = *TOKIO_RUNTIME;
            }
            _ => {}
        }
    }

    fn on_level_deinit(level: InitLevel) {
        println!("deinit {:?}", level);
    }
}

#[cfg(target_os = "android")]
fn setup_logging() {
    // gstreamer::log::add_log_function(|cat, lvl, file, func, line, obj, msg| {
    //     let s = format!("{:?} {:?} {:} {:} {:} {:?} {:?}\0", cat, lvl, file, func, line, obj, msg);
    //     unsafe {
    //         ndk_sys::__android_log_write(ndk_sys::android_LogPriority::ANDROID_LOG_INFO.0 as _, "godot\0".as_ptr(), s.as_ptr());
    //     }
    // });

    let (rfd, wfd) = nix::unistd::pipe().unwrap();
    unsafe {
        nix::libc::dup2(wfd, 1);
        nix::libc::dup2(wfd, 2);
    }
    std::thread::spawn(move || {
        let mut buf: Vec<u8> = Vec::new();
        buf.resize(4096, 0);
        loop {
            match nix::unistd::read(rfd, &mut buf) {
                Ok(nread) => {
                    let mut b = buf[0..nread].to_vec();
                    b.push(0);
                    unsafe {
                        ndk_sys::__android_log_write(
                            ndk_sys::android_LogPriority::ANDROID_LOG_DEBUG.0 as _,
                            "godot\0".as_ptr(),
                            b.as_ptr(),
                        );
                    }
                }
                Err(_) => {
                    return;
                }
            }
        }
    });
}
