use godot::classes::{ImmediateMesh, MeshInstance3D, Node3D};
use godot::engine::mesh::PrimitiveType;
use godot::engine::Material;
use godot::prelude::*;

use crate::binding::ImagesMessage;

#[derive(GodotClass)]
#[class(base=Node3D)]
struct PointCloudVisualizer {
    mesh: Gd<ImmediateMesh>,
    mesh_inst: Gd<MeshInstance3D>,
    cloud: vrrop_server::PointCloud,
    material: Option<Gd<Material>>,
    base: Base<Node3D>,
}

#[godot_api]
impl PointCloudVisualizer {
    #[func]
    fn add_image(&mut self, image: Gd<ImagesMessage>) {
        self.cloud = self.cloud.merge_images_msg(&image.bind().inner);
        let mesh = &mut self.mesh;
        mesh.clear_surfaces();
        mesh.call(
            "surface_begin".into(),
            &[PrimitiveType::POINTS.to_variant()],
        );
        for point in self.cloud.points.iter() {
            let color = point.color;
            let position = point.position;
            let size = point.size;
            let mut vertex_color = Color::from_rgba8(color.x, color.y, color.z, 0);
            vertex_color.a = size * 100.0;
            mesh.surface_set_color(vertex_color);
            mesh.surface_add_vertex(Vector3::new(
                position.x as real,
                position.y as real,
                position.z as real,
            ));
        }
        mesh.surface_end();
        if let Some(mat) = &self.material {
            mesh.surface_set_material(0, mat.clone());
        }
        self.mesh_inst.set_mesh(self.mesh.clone().upcast());
    }

    #[func]
    fn set_material(&mut self, material: Option<Gd<Material>>) {
        self.material = material;
    }
}

#[godot_api]
impl INode3D for PointCloudVisualizer {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            cloud: vrrop_server::PointCloud::new(),
            mesh: ImmediateMesh::new_gd(),
            mesh_inst: MeshInstance3D::new_alloc(),
            material: None,
        }
    }

    fn ready(&mut self) {
        let mut mesh_inst = self.mesh_inst.clone();
        mesh_inst.set_mesh(self.mesh.clone().upcast());
        self.base_mut().add_child(mesh_inst.upcast());
    }
}
