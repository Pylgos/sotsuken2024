use std::{sync::Mutex, thread::sleep, time::Duration};
use slam_core::SlamCore;

mod slam_core;
mod slam_core_sys;

fn main() {
    let last_color_image = Mutex::new(None);
    let last_depth_image = Mutex::new(None);
    let mut core = SlamCore::new();
    core.register_odometry_event_handler(|ev| {
        println!("translation: {:?}", ev.translation);
        println!("rotation   : {:?}", ev.rotation.euler_angles());
        println!("color      : {:?}", ev.color_image.get_pixel(0, 0));
        println!("depth      : {:?}", ev.depth_image.get_pixel(ev.depth_image.width() / 2, ev.depth_image.height() / 2));
        last_color_image.lock().unwrap().replace(ev.color_image);
        last_depth_image.lock().unwrap().replace(ev.depth_image);
    });
    sleep(Duration::from_secs(10));
    last_color_image.lock().unwrap().as_ref().unwrap().save("color.png").unwrap();
    last_depth_image.lock().unwrap().as_ref().unwrap().save("depth.png").unwrap();
}

