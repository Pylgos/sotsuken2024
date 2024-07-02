use std::borrow::{Borrow, BorrowMut};

use godot::engine::WeakRef;
use godot::global::weakref;
use godot::prelude::*;
use godot::{classes::RefCounted, engine::Image};
use image::EncodableLayout;

use crate::{SharedGd, TOKIO_RUNTIME};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct VrropServer {
    base: Base<RefCounted>,
    pub inner: Option<vrrop_server::Server>,
}

#[godot_api]
impl VrropServer {
    #[signal]
    fn odometry_received(&self, odometry: Gd<OdometryMessage>);

    #[signal]
    fn images_received(&self, images: Gd<ImagesMessage>);

    #[func(gd_self)]
    fn start(mut this: Gd<Self>) {
        let weak1: SharedGd<WeakRef> = SharedGd(weakref(this.to_variant()).to());
        let weak2 = SharedGd(weak1.clone());

        let _enter = TOKIO_RUNTIME.get().unwrap().enter();
        let server = tokio::runtime::Handle::current()
            .block_on(vrrop_server::Server::new(vrrop_server::Callbacks::new(
                move |odometry| {
                    // godot_print!("Odometry: {:?}", odometry);
                    let odometry = OdometryMessage::new_gd(odometry);
                    let mut strong: Gd<VrropServer> = weak1.get_ref().to();
                    strong.call_deferred(
                        "emit_signal".into(),
                        &["odometry_received".to_variant(), odometry.to_variant()],
                    );
                },
                move |images| {
                    // godot_print!("Images received");
                    let images = ImagesMessage::new_gd(images);
                    let mut strong: Gd<VrropServer> = weak2.get_ref().to();
                    strong.call_deferred(
                        "emit_signal".into(),
                        &["images_received".to_variant(), images.to_variant()],
                    );
                },
            )))
            .unwrap();
        this.bind_mut().inner = Some(server);
    }
}

#[godot_api]
impl IRefCounted for VrropServer {
    fn init(base: Base<RefCounted>) -> Self {
        Self { base, inner: None }
    }
}

#[derive(GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct ImagesMessage {
    base: Base<RefCounted>,
    pub inner: vrrop_server::ImagesMessage,
}

#[godot_api]
impl ImagesMessage {
    #[func]
    fn convert_depth(&self) -> Gd<Image> {
        let depth = &self.inner.depth;
        let depth_data: &[u8] = depth.as_bytes();
        Image::create_from_data(
            depth.width() as _,
            depth.height() as _,
            false,
            godot::classes::image::Format::LA8,
            PackedByteArray::from(depth_data),
        )
        .unwrap()
    }

    #[func]
    fn convert_color(&self) -> Gd<Image> {
        let color = &self.inner.color;
        let color_data: &[u8] = color.as_bytes();
        Image::create_from_data(
            color.width() as _,
            color.height() as _,
            false,
            godot::classes::image::Format::RGB8,
            PackedByteArray::from(color_data),
        )
        .unwrap()
    }
}

impl ImagesMessage {
    fn new_gd(inner: vrrop_server::ImagesMessage) -> Gd<Self> {
        Gd::from_init_fn(|base| Self { base, inner })
    }
}

#[derive(GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct OdometryMessage {
    base: Base<RefCounted>,
    pub inner: vrrop_server::OdometryMessage,
}

#[godot_api]
impl OdometryMessage {
    #[func]
    fn translation(&self) -> Vector3 {
        Vector3::new(
            self.inner.translation.x,
            self.inner.translation.y,
            self.inner.translation.z,
        )
    }

    #[func]
    fn rotation(&self) -> Quaternion {
        let v = self.inner.rotation.as_vector();
        Quaternion::new(v.x, v.y, v.z, v.w)
    }
}

impl OdometryMessage {
    fn new_gd(inner: vrrop_server::OdometryMessage) -> Gd<Self> {
        Gd::from_init_fn(|base| Self { base, inner })
    }
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct PointCloud {
    base: Base<RefCounted>,
    pub inner: vrrop_server::PointCloud,
}

#[godot_api]
impl PointCloud {
    #[func]
    fn merge_images_msg(&self, image_msg: Gd<ImagesMessage>) -> Gd<PointCloud> {
        Gd::from_init_fn(move |base| PointCloud {
            base,
            inner: self.inner.merge_images_msg(&image_msg.bind().inner),
        })
    }
}

#[godot_api]
impl IRefCounted for PointCloud {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            inner: vrrop_server::PointCloud::new(),
        }
    }
}
