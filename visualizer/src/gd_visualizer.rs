use crate::decoder::VideoFrame;
use crate::visualizer::Visualizer;
use crate::GODOT_GL_INFO;
use godot::builtin::{Color, Rid, Vector2};
use godot::engine::{ITexture2D, Image, RefCounted, RenderingServer, Texture2D};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Texture2D)]
pub struct ExtGLTexture {
    height: i32,
    width: i32,
    texture: Rid,
    base: Base<Texture2D>,
}

#[godot_api]
impl ExtGLTexture {
    #[func]
    fn get_image(&self) -> Gd<Image> {
        RenderingServer::singleton()
            .texture_2d_get(self.texture)
            .unwrap()
    }

    #[func]
    fn __get_rid(&self) -> Rid {
        self.texture
    }
}

impl ExtGLTexture {
    fn render(&mut self, frame: VideoFrame) {
        let height = frame.height() as _;
        let width = frame.width() as _;
        if height != self.height || width != self.width || self.texture.is_invalid() {
            if self.texture.is_valid() {
                RenderingServer::singleton().free_rid(self.texture);
            }
            let mut dummy_image =
                Image::create(width, height, false, godot::engine::image::Format::RGBA8).unwrap();
            dummy_image.fill(Color::from_rgb(1.0, 1.0, 1.0));
            self.height = height;
            self.width = width;
            self.texture = RenderingServer::singleton().texture_2d_create(dummy_image);
        }
        let gl_texture = RenderingServer::singleton().texture_get_native_handle(self.texture);
        frame.copy_to_texture2d(&GODOT_GL_INFO.context, gl_texture as _);
    }
}

impl Drop for ExtGLTexture {
    fn drop(&mut self) {
        RenderingServer::singleton().free_rid(self.texture);
    }
}

#[godot_api]
impl ITexture2D for ExtGLTexture {
    fn init(base: Base<Texture2D>) -> Self {
        Self {
            height: 0,
            width: 0,
            texture: Rid::Invalid,
            base,
        }
    }

    fn draw(&self, canvas_item: Rid, pos: Vector2, modulate: Color, transpose: bool) {
        RenderingServer::singleton()
            .canvas_item_add_texture_rect_ex(
                canvas_item,
                Rect2::new(pos, Vector2::new(self.width as _, self.height as _)),
                self.texture,
            )
            .tile(false)
            .modulate(modulate)
            .transpose(transpose)
            .done();
    }

    fn draw_rect(
        &self,
        canvas_item: Rid,
        rect: Rect2,
        tile: bool,
        modulate: Color,
        transpose: bool,
    ) {
        RenderingServer::singleton()
            .canvas_item_add_texture_rect_ex(canvas_item, rect, self.texture)
            .tile(tile)
            .modulate(modulate)
            .transpose(transpose)
            .done();
    }

    fn draw_rect_region(
        &self,
        canvas_item: Rid,
        rect: Rect2,
        src_rect: Rect2,
        modulate: Color,
        transpose: bool,
        clip_uv: bool,
    ) {
        RenderingServer::singleton()
            .canvas_item_add_texture_rect_region_ex(canvas_item, rect, self.texture, src_rect)
            .modulate(modulate)
            .transpose(transpose)
            .clip_uv(clip_uv)
            .done();
    }

    fn get_height(&self) -> i32 {
        self.height
    }

    fn get_width(&self) -> i32 {
        self.width
    }

    fn has_alpha(&self) -> bool {
        false
    }

    fn is_pixel_opaque(&self, _x: i32, _y: i32) -> bool {
        true
    }
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct VisualizerTest {
    visualizer: Option<Visualizer>,
    texture: Option<Gd<ExtGLTexture>>,
}

#[godot_api]
impl IRefCounted for VisualizerTest {
    fn init(_base: Base<RefCounted>) -> Self {
        let _enter = crate::TOKIO_RUNTIME.enter();
        Self {
            visualizer: Some(
                Visualizer::new(GODOT_GL_INFO.context.clone(), GODOT_GL_INFO.display.clone())
                    .unwrap(),
            ),
            texture: None,
        }
    }
}

#[godot_api]
impl VisualizerTest {
    #[func]
    fn get_texture(&mut self) -> Option<Gd<ExtGLTexture>> {
        self.texture.clone()
    }

    #[func]
    fn update(&mut self) {
        let _enter = crate::TOKIO_RUNTIME.enter();
        let mut maybe_frame = None;
        while let Ok(frame) = self.visualizer.as_mut().unwrap().try_recv() {
            maybe_frame = Some(frame);
        }
        let Some(frame) = maybe_frame else {
            return;
        };

        let texture = self.texture.get_or_insert_with(|| ExtGLTexture::new_gd());

        texture.bind_mut().render(frame);
    }
}

impl Drop for VisualizerTest {
    fn drop(&mut self) {
        let _enter = crate::TOKIO_RUNTIME.enter();
        println!("dropping!");
        let mut receiver = None;
        std::mem::swap(&mut self.visualizer, &mut receiver);
        tokio::runtime::Handle::current()
            .block_on(receiver.unwrap().close())
            .unwrap();
    }
}
