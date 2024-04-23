use gl::types::{GLsizei, GLuint};
use once_cell::sync::Lazy;

fn check_err(context: &str) {
    unsafe {
        let err = gl::GetError();
        if err == gl::NO_ERROR {
            return;
        }
        println!("GL ERROR ({}): {:}", context, err);
    }
}

pub struct TextureCopy {
    frame_buffer: GLuint,
}

static TEXTURE_COPY: Lazy<TextureCopy> = Lazy::new(|| TextureCopy::new());

pub fn copy_texture(src: GLuint, dst: GLuint, width: GLsizei, height: GLsizei) {
    TEXTURE_COPY.copy(src, dst, width, height)
}

impl TextureCopy {
    pub fn new() -> Self {
        let mut frame_buffer: GLuint = 0;
        unsafe {
            gl::GenFramebuffers(1, &mut frame_buffer);
        }
        check_err("TextureCopy::new");
        Self { frame_buffer }
    }

    pub fn copy(&self, src: GLuint, dst: GLuint, width: GLsizei, height: GLsizei) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.frame_buffer);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                src,
                0,
            );
            gl::ReadBuffer(gl::COLOR_ATTACHMENT0);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, dst);
            gl::CopyTexSubImage2D(gl::TEXTURE_2D, 0, 0, 0, 0, 0, width, height);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        check_err("TextureCopy::copy");
    }
}

impl Drop for TextureCopy {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.frame_buffer);
        }
    }
}
