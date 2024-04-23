use std::os::raw::c_void;

use anyhow::{bail, Context as _, Result};
use godot::prelude::*;
use gst_gl::prelude::*;
use gstreamer::prelude::*;
use gstreamer_gl as gst_gl;
use once_cell::sync::Lazy;

use gstreamer_gl_egl as gst_gl_egl;
#[cfg(target_os = "linux")]
use gstreamer_gl_x11 as gst_gl_x11;

mod texture_copy;
mod decoder;
mod gd_visualizer;
mod visualizer;

static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Runtime::new().unwrap());

struct VisualizerExtensionLibrary;

pub(crate) struct GlInfo {
    pub(crate) context: gst_gl::GLContext,
    pub(crate) display: gst_gl::GLDisplay,
}

fn try_init_egl() -> Result<GlInfo> {
    use gst_gl::GLAPI;
    unsafe {
        let egl = khronos_egl::Instance::new(khronos_egl::Static);
        let context = egl
            .get_current_context()
            .context("failed to get current egl context")?;
        println!("got egl context {:?}", context);
        let display = egl
            .get_current_display()
            .context("failed to get current egl display")?;

        let gst_display = gst_gl_egl::GLDisplayEGL::with_egl_display(display.as_ptr() as usize)?
            .upcast::<gst_gl::GLDisplay>();
        let gst_context = gst_gl::GLContext::new_wrapped(
            &gst_display,
            context.as_ptr() as usize,
            gst_gl::GLPlatform::EGL,
            GLAPI::GLES2,
        );
        Ok(GlInfo {
            context: gst_context.unwrap(),
            display: gst_display,
        })
    }
}

#[cfg(target_os = "linux")]
fn try_init_glx() -> Result<GlInfo> {
    use gst_gl::GLAPI;
    unsafe {
        let context = glx::GetCurrentContext();
        if context.is_null() {
            bail!("failed to get glx context")
        }

        let display = glx::GetCurrentDisplay();
        if display.is_null() {
            bail!("failed to get glx display")
        }

        let gst_display =
            gst_gl_x11::GLDisplayX11::with_display(display as usize)?.upcast::<gst_gl::GLDisplay>();
        let gst_context = gst_gl::GLContext::new_wrapped(
            &gst_display,
            context as usize,
            gst_gl::GLPlatform::GLX,
            GLAPI::OPENGL | GLAPI::OPENGL3,
        );
        Ok(GlInfo {
            context: gst_context.unwrap(),
            display: gst_display.upcast(),
        })
    }
}

fn init_gl() -> GlInfo {
    if let Ok(info) = try_init_egl() {
        return info;
    }
    #[cfg(target_os = "linux")]
    if let Ok(info) = try_init_glx() {
        return info;
    }
    panic!("failed to init gl")
}

pub(crate) static GODOT_GL_INFO: Lazy<GlInfo> = Lazy::new(|| {
    let info = init_gl();
    let ctx = info.context.clone();
    gl::load_with(|symbol| {
        let addr = ctx.proc_address(symbol) as *const c_void;
        if addr.is_null() {
            println!("failed to load: {symbol}")
        }
        addr
    });
    info.context.activate(true).unwrap();
    info.context.fill_info().unwrap();
    info
});

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
            InitLevel::Scene => {
                let _ = *GODOT_GL_INFO;
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
