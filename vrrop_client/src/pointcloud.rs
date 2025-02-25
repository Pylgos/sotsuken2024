use fxhash::{FxHashMap, FxHashSet};
use nalgebra::{Point3, Vector2, Vector3};
use vrrop_common::CameraIntrinsics;

use crate::{ImagesMessage, OdometryMessage};

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub position: Point3<f32>,
    pub color: Vector3<u8>,
    pub size: f32,
}

pub type GridIndex = Vector3<i32>;

#[derive(Debug)]
pub struct SpacialGridMap {
    grid_size: f32,
    grids: FxHashMap<GridIndex, Vec<Point>>,
}

impl SpacialGridMap {
    pub fn new(grid_size: f32) -> Self {
        Self {
            grid_size,
            grids: FxHashMap::default(),
        }
    }

    pub fn grids(&self) -> &FxHashMap<GridIndex, Vec<Point>> {
        &self.grids
    }

    pub fn grid_size(&self) -> f32 {
        self.grid_size
    }

    pub fn grid_index(&self, point: &Point3<f32>) -> GridIndex {
        point
            .coords
            .map(|x| x / self.grid_size)
            .map(|x| x.floor() as i32)
    }

    pub fn add_point(&mut self, point: &Point) {
        let idx = self.grid_index(&point.position);
        self.grids.entry(idx).or_default().push(*point);
    }

    pub fn points_in_grid_mut(&mut self, grid_index: GridIndex) -> Option<&mut Vec<Point>> {
        self.grids.get_mut(&grid_index)
    }

    pub fn points_in_grid(&self, grid_index: GridIndex) -> Option<&Vec<Point>> {
        self.grids.get(&grid_index)
    }

    pub fn grids_touching_aabb(
        &self,
        min: Vector3<f32>,
        max: Vector3<f32>,
    ) -> impl Iterator<Item = GridIndex> + '_ {
        let min_idx = min.map(|x| (x / self.grid_size).floor() as i32);
        let max_idx = max.map(|x| (x / self.grid_size).floor() as i32);
        (min_idx.x..=max_idx.x)
            .flat_map(move |x| {
                (min_idx.y..=max_idx.y)
                    .flat_map(move |y| (min_idx.z..=max_idx.z).map(move |z| Vector3::new(x, y, z)))
            })
            .filter(|grid_index| self.grids.contains_key(grid_index))
    }

    pub fn grid_corners(&self, grid_index: GridIndex) -> [Point3<f32>; 8] {
        let min = grid_index.map(|x| x as f32 * self.grid_size);
        let max = min + Vector3::new(self.grid_size, self.grid_size, self.grid_size);
        [
            Point3::new(min.x, min.y, min.z),
            Point3::new(min.x, min.y, max.z),
            Point3::new(min.x, max.y, min.z),
            Point3::new(min.x, max.y, max.z),
            Point3::new(max.x, min.y, min.z),
            Point3::new(max.x, min.y, max.z),
            Point3::new(max.x, max.y, min.z),
            Point3::new(max.x, max.y, max.z),
        ]
    }

    pub fn all_points(&self) -> impl Iterator<Item = &Point> {
        self.grids.values().flat_map(|cell| cell.iter())
    }

    pub fn remove_grid(&mut self, grid_index: GridIndex) {
        self.grids.remove(&grid_index);
    }
}

pub struct PointCloud {
    grid_map: SpacialGridMap,
}

impl PointCloud {
    pub fn new(grid_size: f32) -> Self {
        Self {
            grid_map: SpacialGridMap::new(grid_size),
        }
    }

    pub fn merge_images_msg(&mut self, image_msg: &ImagesMessage) -> (FxHashSet<GridIndex>, std::time::Duration) {
        let start = std::time::Instant::now();
        let prev_point_count = self.grid_map.all_points().count();

        let max_depth = 5.0;

        let extrinsics = odometry_to_extrinsics(image_msg.odometry);
        let color_projector = Projector::new(image_msg.color_intrinsics, extrinsics);
        let depth_projector = Projector::new(image_msg.depth_intrinsics, extrinsics);

        let (min, max) = color_projector.aabb(max_depth);
        let target_grids: Vec<GridIndex> = self
            .grid_map
            .grids_touching_aabb(min, max)
            .filter(|grid_index| {
                self.grid_map.grid_corners(*grid_index).iter().any(|point| {
                    color_projector.point_to_pixel(*point).is_some()
                        && color_projector.point_depth(*point) < max_depth
                })
            })
            .collect();

        let mut modified_grids = FxHashSet::default();
        for grid_index in target_grids {
            let mut modified = false;
            let mut i = 0;
            let Some(points) = self.grid_map.points_in_grid_mut(grid_index) else {
                continue;
            };
            loop {
                let Some(point) = points.get(i) else {
                    break;
                };
                if let (Some(_), Some(depth_pixel)) = (
                    color_projector.point_to_pixel(point.position),
                    depth_projector.point_to_pixel(point.position),
                ) {
                    let orig_depth = depth_projector.point_depth(point.position);
                    let depth = image_msg.depth.get_pixel(depth_pixel.x, depth_pixel.y)[0] as f32
                        * image_msg.depth_unit;
                    let remove = depth > orig_depth - 0.5;
                    if remove {
                        modified = true;
                        points.swap_remove(i);
                        continue;
                    }
                }
                i += 1;
            }
            if points.is_empty() {
                self.grid_map.remove_grid(grid_index);
            }
            if modified {
                modified_grids.insert(grid_index);
            }
        }

        for y in 0..image_msg.depth.height() {
            for x in 0..image_msg.depth.width() {
                let depth_pixel = Vector2::new(x, y);
                let depth = image_msg.depth.get_pixel(x, y)[0] as f32 * image_msg.depth_unit;
                if depth == 0.0 || depth > max_depth {
                    continue;
                }
                let point = depth_projector.pixel_to_point(depth_pixel, depth);
                if let Some(color_pixel) = color_projector.point_to_pixel(point) {
                    let color = image_msg.color.get_pixel(color_pixel.x, color_pixel.y).0;
                    let size = color_projector.point_size(depth);
                    self.grid_map.add_point(&Point {
                        position: point,
                        color: Vector3::new(color[0], color[1], color[2]),
                        size,
                    });
                    modified_grids.insert(self.grid_map.grid_index(&point));
                }
            }
        }

        let point_count = self.grid_map.all_points().count();
        println!("merge_images_msg: took {:?}", start.elapsed());
        println!(
            "point count: {} -> {}    delta {}",
            prev_point_count,
            point_count,
            point_count as i64 - prev_point_count as i64
        );
        println!("grid count: {}", self.grid_map().grids().len());
        (modified_grids, start.elapsed())
    }

    pub fn grid_map(&self) -> &SpacialGridMap {
        &self.grid_map
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

    fn aabb(&self, depth: f32) -> (Vector3<f32>, Vector3<f32>) {
        let origin = self.extrinsics.translation.vector;
        let c1 = self.pixel_to_point(Vector2::new(0, 0), depth);
        let c2 = self.pixel_to_point(Vector2::new(self.intrinsics.width - 1, 0), depth);
        let c3 = self.pixel_to_point(Vector2::new(0, self.intrinsics.height - 1), depth);
        let c4 = self.pixel_to_point(
            Vector2::new(self.intrinsics.width - 1, self.intrinsics.height - 1),
            depth,
        );
        let min = Vector3::new(
            origin.x.min(c1.x).min(c2.x).min(c3.x).min(c4.x),
            origin.y.min(c1.y).min(c2.y).min(c3.y).min(c4.y),
            origin.z.min(c1.z).min(c2.z).min(c3.z).min(c4.z),
        );
        let max = Vector3::new(
            origin.x.max(c1.x).max(c2.x).max(c3.x).max(c4.x),
            origin.y.max(c1.y).max(c2.y).max(c3.y).max(c4.y),
            origin.z.max(c1.z).max(c2.z).max(c3.z).max(c4.z),
        );
        (min, max)
    }
}
