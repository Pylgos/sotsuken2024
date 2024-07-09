use nalgebra::{Point3, Vector2, Vector3};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use vrrop_common::CameraIntrinsics;

use crate::{ImagesMessage, OdometryMessage};

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub position: Point3<f32>,
    pub color: Vector3<u8>,
    pub age: u8,
    pub size: f32,
}

pub struct PointCloud {
    pub points: Vec<Point>,
}

impl PointCloud {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    pub fn merge_images_msg(&self, image_msg: &ImagesMessage) -> PointCloud {
        let maximum_point_count = 1000000;
        let max_age = 255;

        let extrinsics = odometry_to_extrinsics(image_msg.odometry);
        let color_projector = Projector::new(image_msg.color_intrinsics, extrinsics);
        let depth_projector = Projector::new(image_msg.depth_intrinsics, extrinsics);

        let filtered_points_iter = self
            .points
            .par_iter()
            .filter(|point| {
                match (
                    color_projector.point_to_pixel(point.position),
                    depth_projector.point_to_pixel(point.position),
                ) {
                    (Some(_), Some(depth_pixel)) => {
                        let orig_depth = depth_projector.point_depth(point.position);
                        let depth = image_msg.depth.get_pixel(depth_pixel.x, depth_pixel.y)[0]
                            as f32
                            * image_msg.depth_unit;
                        // if orig_depth < depth + 0.5 {
                        if depth != 0.0 {
                            false
                        } else {
                            true
                        }
                    }
                    _ => true,
                }
            }).filter_map(|point| {
                let new_age = point.age.saturating_add(1);
                if new_age >= max_age {
                    None
                } else {
                    Some(Point { age: new_age, ..*point })
                }
            });

        let additional_points_iter = (0..image_msg.depth.height())
            .into_par_iter()
            .flat_map(|y| {
                (0..image_msg.depth.width())
                    .into_par_iter()
                    .map(move |x| (x, y))
            })
            .filter_map(|(x, y)| {
                let depth_pixel = Vector2::new(x, y);
                let depth = image_msg.depth.get_pixel(x, y)[0] as f32 * image_msg.depth_unit;
                
                let (depth, age) = if depth == 0.0  {
                    (10.0, max_age - 1)
                } else if depth > 5.0 {
                    (depth, max_age - 1)
                } else {
                    (depth, 0)
                };
                let point = depth_projector.pixel_to_point(depth_pixel, depth);
                if let Some(color_pixel) = color_projector.point_to_pixel(point) {
                    let color = image_msg.color.get_pixel(color_pixel.x, color_pixel.y).0;
                    let size = color_projector.point_size(depth);
                    Some(Point {
                        age,
                        position: point,
                        color: Vector3::new(color[0], color[1], color[2]),
                        size,
                    })
                } else {
                    None
                }
            });

        let mut new_points: Vec<_> = filtered_points_iter.chain(additional_points_iter).collect();

        println!("number of points: {}  delta: {}", new_points.len(), new_points.len() as i64 - self.points.len() as i64);

        PointCloud { points: new_points }
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
        if point.x < 0.0 {
            return None;
        }
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
