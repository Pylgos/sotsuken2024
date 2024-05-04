use crate::decoder::VideoFrame;
use crate::visualizer::Visualizer;
use godot::engine::{ITexture2D, Image, RefCounted, ImageTexture};
use godot::obj::WithBaseField;
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=ImageTexture)]
pub struct ExtGLTexture {
    base: Base<ImageTexture>,
}

impl ExtGLTexture {
    fn render(&mut self, frame: VideoFrame) {
        let height = frame.height() as _;
        let width = frame.width() as _;
        let mut data: PackedByteArray = PackedByteArray::new();
        data.resize((width * height * 4) as _);
        frame.copy_to(data.as_mut_slice()).unwrap();
        let img = Image::create_from_data(width, height, false, godot::engine::image::Format::RGBA8, data).unwrap();
        self.base_mut().set_image(img);
    }
}

#[godot_api]
impl ITexture2D for ExtGLTexture {
    fn init(base: Base<ImageTexture>) -> Self {
        Self {
            base,
        }
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
                Visualizer::new().unwrap(),
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
