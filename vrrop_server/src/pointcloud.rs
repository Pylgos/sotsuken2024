use nalgebra::{Point3, Vector2, Vector3};
use vrrop_common::CameraIntrinsics;

use crate::{ImagesMessage, OdometryMessage};

pub struct PointCloud {
    pub points: Vec<Point3<f32>>,
    pub colors: Vec<Vector3<u8>>,
    pub sizes: Vec<f32>,
}

impl PointCloud {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            colors: Vec::new(),
            sizes: Vec::new(),
        }
    }

    pub fn push(&mut self, point: Point3<f32>, color: Vector3<u8>, size: f32) {
        self.points.push(point);
        self.colors.push(color);
        self.sizes.push(size);
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.colors.clear();
    }

    pub fn merge_images_msg(&mut self, image_msg: &ImagesMessage) {
        let extrinsics = odometry_to_extrinsics(image_msg.odometry);
        let color_projector = Projector::new(image_msg.color_intrinsics, extrinsics);
        let depth_projector = Projector::new(image_msg.depth_intrinsics, extrinsics);

        let mut i = 0;
        let mut removed_points = 0;
        loop {
            if i >= self.points.len() {
                break;
            }
            match (
                color_projector.point_to_pixel(self.points[i]),
                depth_projector.point_to_pixel(self.points[i]),
            ) {
                (Some(_), Some(depth_pixel)) => {
                    let orig_depth = depth_projector.point_depth(self.points[i]);
                    let depth = image_msg.depth.get_pixel(depth_pixel.x, depth_pixel.y)[0] as f32
                        * image_msg.depth_unit;
                    if orig_depth < depth + 0.1 {
                        self.points.swap_remove(i);
                        self.colors.swap_remove(i);
                        self.sizes.swap_remove(i);
                        removed_points += 1;
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        println!("points removed: {}", removed_points);

        let mut added_points = 0;
        for y in 0..image_msg.depth.height() {
            for x in 0..image_msg.depth.width() {
                let depth_pixel = Vector2::new(x, y);
                let depth = image_msg.depth.get_pixel(x, y)[0] as f32 * image_msg.depth_unit;
                if depth == 0.0 {
                    continue;
                }
                let point = depth_projector.pixel_to_point(depth_pixel, depth);
                if let Some(color_pixel) = color_projector.point_to_pixel(point) {
                    let color = image_msg.color.get_pixel(color_pixel.x, color_pixel.y).0;
                    let size = color_projector.point_size(depth);
                    self.push(point, Vector3::new(color[0], color[1], color[2]), size);
                    added_points += 1;
                }
            }
        }
        println!("number of points: {}  Δ: {}", self.points.len(), added_points - removed_points);
    }
}

fn odometry_to_extrinsics(odometry: OdometryMessage) -> nalgebra::Isometry3<f32> {
    let translation = nalgebra::Translation3::new(
        odometry.translation.x,
        odometry.translation.y,
        odometry.translation.z,
    );
    nalgebra::Isometry3::from_parts(translation, odometry.rotation)
}

struct Projector {
    intrinsics: CameraIntrinsics,
    extrinsics: nalgebra::Isometry3<f32>,
    inv_extrinsics: nalgebra::Isometry3<f32>,
}

impl Projector {
    fn new(intrinsics: CameraIntrinsics, extrinsics: nalgebra::Isometry3<f32>) -> Self {
        let inv_extrinsics = extrinsics.inverse();
        Self {
            intrinsics,
            extrinsics,
            inv_extrinsics,
        }
    }

    fn point_to_pixel(&self, point: Point3<f32>) -> Option<Vector2<u32>> {
        let point = self.inv_extrinsics * point;
        let x = (self.intrinsics.fx * -point.y / point.x + self.intrinsics.cx) as i64;
        let y = (self.intrinsics.fy * -point.z / point.x + self.intrinsics.cy) as i64;
        if 0 <= x && x < self.intrinsics.width as i64 && 0 <= y && y < self.intrinsics.height as i64
        {
            Some(Vector2::new(x as u32, y as u32))
        } else {
            None
        }
    }

    fn point_depth(&self, point: Point3<f32>) -> f32 {
        let point = self.inv_extrinsics * point;
        point.x
    }

    fn pixel_to_point(&self, pixel: Vector2<u32>, depth: f32) -> Point3<f32> {
        let y = -(pixel.x as f32 - self.intrinsics.cx) / self.intrinsics.fx;
        let z = -(pixel.y as f32 - self.intrinsics.cy) / self.intrinsics.fy;
        self.extrinsics * Point3::new(depth, y * depth, z * depth)
    }

    fn point_size(&self, depth: f32) -> f32 {
        depth / self.intrinsics.fx
    }
}
