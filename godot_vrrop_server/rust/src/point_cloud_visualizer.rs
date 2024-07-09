use fxhash::FxHashMap;
use godot::classes::{ImmediateMesh, MeshInstance3D, Node3D};
use godot::engine::mesh::PrimitiveType;
use godot::engine::Material;
use godot::prelude::*;
use vrrop_server::{GridIndex, PointCloud};

use crate::binding::ImagesMessage;

#[derive(GodotClass)]
#[class(base=Node3D)]
struct PointCloudVisualizer {
    meshes: FxHashMap<GridIndex, Gd<MeshInstance3D>>,
    cloud: PointCloud,
    material: Option<Gd<Material>>,
    base: Base<Node3D>,
}

fn create_mesh(
    grid_index: GridIndex,
    cloud: &PointCloud,
    material: Gd<Material>,
) -> Option<Gd<ImmediateMesh>> {
    let points = cloud.grid_map().points_in_grid(grid_index)?;
    let mut mesh = ImmediateMesh::new_gd();
    mesh.call(
        "surface_begin".into(),
        &[PrimitiveType::POINTS.to_variant()],
    );
    for point in points {
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
    mesh.surface_set_material(0, material);
    Some(mesh)
}

#[godot_api]
impl PointCloudVisualizer {
    #[func]
    fn add_image(&mut self, image: Gd<ImagesMessage>) {
        let Some(material) = self.material.clone() else {
            return;
        };
        let image = image.bind();
        let Some(image) = image.inner.as_ref() else {
            return;
        };
        let modified_grids = self.cloud.merge_images_msg(image);
        for grid_index in modified_grids {
            if let Some(mesh) = create_mesh(grid_index, &self.cloud, material.clone()) {
                if let Some(mesh_inst) = self.meshes.get_mut(&grid_index) {
                    mesh_inst.set_deferred("mesh".into(), mesh.to_variant());
                } else {
                    let mut mesh_inst = MeshInstance3D::new_alloc();
                    mesh_inst.set_deferred("mesh".into(), mesh.to_variant());
                    self.base_mut()
                        .call_deferred("add_child".into(), &[mesh_inst.to_variant()]);
                    self.meshes.insert(grid_index, mesh_inst.clone());
                }
            }
        }
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
            cloud: vrrop_server::PointCloud::new(1.0),
            meshes: FxHashMap::default(),
            material: None,
        }
    }
}
