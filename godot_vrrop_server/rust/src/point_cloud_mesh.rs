use godot::classes::ImmediateMesh;
use godot::engine::mesh::PrimitiveType;
use godot::engine::IImmediateMesh;
use godot::prelude::*;

use crate::binding::PointCloud;

#[derive(GodotClass)]
#[class(base=ImmediateMesh)]
struct PointCloudMesh {
    base: Base<ImmediateMesh>,
}

#[godot_api]
impl PointCloudMesh {
    #[func]
    fn set_pointcloud(&mut self, cloud: Gd<PointCloud>) {
        let mut base = self.base_mut();
        base.clear_surfaces();
        base.call(
            "surface_begin".into(),
            &[PrimitiveType::POINTS.to_variant()],
        );
        let cloud = &cloud.bind().inner;
        for point in cloud.points.iter() {
            let color = point.color;
            let position = point.position;
            let size = point.size;
            let mut vertex_color = Color::from_rgba8(color.x, color.y, color.z, 0);
            vertex_color.a = size * 100.0;
            base.surface_set_color(vertex_color);
            base.surface_add_vertex(Vector3::new(
                position.x as real,
                position.y as real,
                position.z as real,
            ));
        }
        base.surface_end();
    }
}

#[godot_api]
impl IImmediateMesh for PointCloudMesh {
    fn init(base: Base<ImmediateMesh>) -> Self {
        Self { base }
    }
}
