use std::sync::Arc;

use godot::classes::RefCounted;
use godot::engine::WeakRef;
use godot::global::weakref;
use godot::prelude::*;

use crate::{SharedGd, TOKIO_RUNTIME};

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct VrropClient {
    base: Base<RefCounted>,
    pub inner: Option<vrrop_client::Client>,
}

#[godot_api]
impl VrropClient {
    #[signal]
    fn odometry_received(&self, odometry: Gd<OdometryMessage>);

    #[signal]
    fn images_received(&self, images: Gd<ImagesMessage>);

    #[func(gd_self)]
    fn start(mut this: Gd<Self>, address: String) {
        let weak1: SharedGd<WeakRef> = SharedGd(weakref(this.to_variant()).to());
        let weak2 = SharedGd(weak1.clone());

        let _enter = TOKIO_RUNTIME.get().unwrap().enter();
        let server = tokio::runtime::Handle::current()
            .block_on(vrrop_client::Client::new(
                address,
                vrrop_client::Callbacks::new(
                    move |odometry| {
                        // godot_print!("Odometry: {:?}", odometry);
                        let odometry = OdometryMessage::new_gd(odometry);
                        let mut strong: Gd<VrropClient> = weak1.get_ref().to();
                        strong.call_deferred(
                            "emit_signal".into(),
                            &["odometry_received".to_variant(), odometry.to_variant()],
                        );
                    },
                    move |images| {
                        // godot_print!("Images received");
                        let images = ImagesMessage::new_gd(images);
                        let mut strong: Gd<VrropClient> = weak2.get_ref().to();
                        strong.call_deferred(
                            "emit_signal".into(),
                            &["images_received".to_variant(), images.to_variant()],
                        );
                    },
                ),
            ))
            .unwrap();
        this.bind_mut().inner = Some(server);
    }
}

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct ImagesMessage {
    base: Base<RefCounted>,
    pub inner: Option<vrrop_client::ImagesMessage>,
}

impl ImagesMessage {
    fn new_gd(inner: vrrop_client::ImagesMessage) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            base,
            inner: Some(inner),
        })
    }
}

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct OdometryMessage {
    base: Base<RefCounted>,
    pub inner: Option<vrrop_client::OdometryMessage>,
}

#[godot_api]
impl OdometryMessage {
    #[func]
    fn translation(&self) -> Vector3 {
        if let Some(tr) = self.inner.as_ref() {
            Vector3::new(tr.translation.x, tr.translation.y, tr.translation.z)
        } else {
            Vector3::new(0.0, 0.0, 0.0)
        }
    }

    #[func]
    fn rotation(&self) -> Quaternion {
        if let Some(rot) = self.inner.as_ref() {
            let v = rot.rotation.as_vector();
            Quaternion::new(v.x, v.y, v.z, v.w)
        } else {
            Quaternion::new(0.0, 0.0, 0.0, 1.0)
        }
    }
}

impl OdometryMessage {
    fn new_gd(inner: vrrop_client::OdometryMessage) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            base,
            inner: Some(inner),
        })
    }
}

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct VrropControlClient {
    base: Base<RefCounted>,
    inner: Option<Arc<vrrop_control_client::Client>>,
}

#[godot_api]
impl VrropControlClient {
    #[func]
    fn connect_to_server(&mut self, address: String) -> godot::global::Error {
        let _enter = TOKIO_RUNTIME.get().unwrap().enter();
        let client_result =
            tokio::runtime::Handle::current().block_on(vrrop_control_client::Client::new(&address));
        match client_result {
            Ok(client) => {
                self.inner = Some(Arc::new(client));
                godot::global::Error::OK
            }
            Err(e) => {
                godot_print!("Error connecting to server: {:?}", e);
                godot::global::Error::ERR_CANT_CONNECT
            }
        }
    }

    #[func]
    fn set_target_velocity(&self, forward: f64, turn: f64) {
        let client = self.inner.as_ref().unwrap().clone();
        TOKIO_RUNTIME.get().unwrap().spawn(async move {
            let _ = client
                .set_target_velocity(vrrop_control_client::SetTargetVelocity {
                    forward: forward as f32,
                    turn: turn as f32,
                })
                .await;
        });
    }

    #[func]
    fn set_leg_length(&self, length: f64) {
        let client = self.inner.as_ref().unwrap().clone();
        TOKIO_RUNTIME.get().unwrap().spawn(async move {
            let _ = client.set_leg_length(length as _).await;
        });
    }
}
