use std::{ffi::c_void, mem::MaybeUninit, ops::Deref, ptr::NonNull};

use crate::slam_core_sys::*;
use image::{ImageBuffer, Luma, Primitive, Rgb};
use nalgebra::{Quaternion, UnitQuaternion, Vector3};
use vrrop_common::CameraIntrinsics;

pub struct SlamCore<'a> {
    inner: *mut slam_core_t,
    callback: Option<FfiCallback<'a>>,
    color_intrinsics: CameraIntrinsics,
    depth_intrinsics: CameraIntrinsics,
}

pub type ColorImage = ImageBuffer<Rgb<u8>, ImageData<u8>>;
pub type DepthImage = ImageBuffer<Luma<u16>, ImageData<u16>>;

pub struct ImageData<T: Primitive> {
    data: NonNull<T>,
    len: usize,
    inner: NonNull<slam_core_image_t>,
    _phantom: std::marker::PhantomData<T>,
}

unsafe impl<T: Primitive> Send for ImageData<T> {}
unsafe impl<T: Primitive> Sync for ImageData<T> {}

impl<T: Primitive> ImageData<T> {
    fn data(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.data.as_ptr(), self.len) }
    }
}

impl<T: Primitive> Deref for ImageData<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl<T: Primitive> From<NonNull<slam_core_image_t>> for ImageData<T> {
    fn from(image: NonNull<slam_core_image_t>) -> Self {
        unsafe {
            Self {
                data: NonNull::new(std::mem::transmute::<*mut c_void, *mut T>(
                    slam_core_image_get_data(image.as_ptr()),
                ))
                .unwrap(),
                len: slam_core_image_get_size(image.as_ptr()),
                inner: image,
                _phantom: std::marker::PhantomData,
            }
        }
    }
}

impl<T: Primitive> Drop for ImageData<T> {
    fn drop(&mut self) {
        unsafe { slam_core_image_destroy(self.inner.as_ptr()) };
    }
}

pub struct OdometryEvent {
    pub translation: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
    pub color_image: ColorImage,
    pub depth_image: DepthImage,
}

struct FfiCallback<'a>(Box<Box<dyn Fn(OdometryEvent) + 'a + Send>>);
struct FfiCallbackRef<'a>(*const Box<dyn Fn(OdometryEvent) + 'a + Send>);

impl<'a> FfiCallback<'a> {
    fn new<F>(cb: F) -> Self
    where
        F: Fn(OdometryEvent) + 'a + Send,
    {
        Self(Box::new(Box::new(cb)))
    }

    fn as_ref(&self) -> FfiCallbackRef<'a> {
        FfiCallbackRef(self.0.as_ref())
    }
}

impl<'a> FfiCallbackRef<'a> {
    fn as_ptr(&self) -> *mut c_void {
        unsafe { std::mem::transmute(self.0) }
    }

    unsafe fn from_ptr(ptr: *const c_void) -> Self {
        std::mem::transmute(ptr)
    }

    fn call(&self, ev: OdometryEvent) {
        unsafe { (*self.0)(ev) }
    }
}

unsafe extern "C" fn odometry_event_handler(
    userdata: *mut std::ffi::c_void,
    raw_ev: *const slam_core_odometry_event_t,
) {
    let raw_ev = raw_ev.as_ref().unwrap();
    let cb = FfiCallbackRef::from_ptr(userdata);
    let rust_ev: OdometryEvent = OdometryEvent {
        translation: Vector3::new(
            raw_ev.translation[0],
            raw_ev.translation[1],
            raw_ev.translation[2],
        ),
        rotation: UnitQuaternion::new_normalize(Quaternion::new(
            raw_ev.rotation[0],
            raw_ev.rotation[1],
            raw_ev.rotation[2],
            raw_ev.rotation[3],
        )),
        color_image: unsafe {
            ColorImage::from_raw(
                slam_core_image_get_width(raw_ev.color),
                slam_core_image_get_height(raw_ev.color),
                ImageData::from(NonNull::new(raw_ev.color).unwrap()),
            )
            .unwrap()
        },
        depth_image: unsafe {
            DepthImage::from_raw(
                slam_core_image_get_width(raw_ev.depth),
                slam_core_image_get_height(raw_ev.depth),
                ImageData::from(NonNull::new(raw_ev.depth).unwrap()),
            )
            .unwrap()
        },
    };
    cb.call(rust_ev);
}

impl<'a> SlamCore<'a> {
    pub fn new() -> Self {
        let inner = unsafe { slam_core_create() };
        let mut color_intrinsics = MaybeUninit::uninit();
        let mut depth_intrinsics = MaybeUninit::uninit();
        unsafe {
            slam_core_get_intrinstics(
                inner,
                color_intrinsics.as_mut_ptr(),
                depth_intrinsics.as_mut_ptr(),
            )
        }
        Self {
            inner,
            callback: None,
            color_intrinsics: convert_intrinsics(unsafe { &color_intrinsics.assume_init() }),
            depth_intrinsics: convert_intrinsics(unsafe { &depth_intrinsics.assume_init() }),
        }
    }

    pub fn register_odometry_event_handler(&mut self, handler: impl Fn(OdometryEvent) + 'a + Send) {
        self.callback = Some(FfiCallback::new(handler));
        unsafe {
            slam_core_register_odometry_event_handler(
                self.inner,
                self.callback.as_ref().unwrap().as_ref().as_ptr(),
                Some(odometry_event_handler),
            )
        };
    }

    pub fn color_intrinsics(&self) -> &CameraIntrinsics {
        &self.color_intrinsics
    }

    pub fn depth_intrinsics(&self) -> &CameraIntrinsics {
        &self.depth_intrinsics
    }
}

impl<'a> Drop for SlamCore<'a> {
    fn drop(&mut self) {
        unsafe { slam_core_delete(self.inner) };
    }
}

fn convert_intrinsics(intrinsics: &slam_core_camera_intrinsics_t) -> CameraIntrinsics {
    CameraIntrinsics {
        width: intrinsics.width,
        height: intrinsics.height,
        fx: intrinsics.fx,
        fy: intrinsics.fy,
        cx: intrinsics.cx,
        cy: intrinsics.cy,
    }
}
