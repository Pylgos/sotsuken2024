use fxhash::{FxHashMap, FxHashSet};
use godot::classes::{ImmediateMesh, MeshInstance3D, Node3D};
use godot::engine::mesh::PrimitiveType;
use godot::engine::Material;
use godot::prelude::*;
use vrrop_server::{GridIndex, PointCloud};

use crate::binding::ImagesMessage;

#[derive(GodotClass)]
#[class(base=Node3D)]
struct PointCloudVisualizer {
    #[export]
    debug_mesh_material_normal: Option<Gd<Material>>,
    #[export]
    debug_mesh_material_modified: Option<Gd<Material>>,
    #[export]
    show_debug_mesh: bool,
    meshes: FxHashMap<GridIndex, Gd<MeshInstance3D>>,
    debug_mesh_inst: Gd<MeshInstance3D>,
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

fn create_debug_mesh(
    cloud: &PointCloud,
    modified_grids: &FxHashSet<GridIndex>,
    normal: Gd<Material>,
    modified: Gd<Material>,
) -> Option<Gd<ImmediateMesh>> {
    let mut mesh = ImmediateMesh::new_gd();
    let add_cube = |mesh: &mut Gd<ImmediateMesh>, grid_index: GridIndex| {
        let c: [Vector3; 8] = cloud
            .grid_map()
            .grid_corners(grid_index)
            .map(|v| Vector3::new(v.x, v.y, v.z));
        mesh.surface_add_vertex(c[0]);
        mesh.surface_add_vertex(c[1]);
        mesh.surface_add_vertex(c[1]);
        mesh.surface_add_vertex(c[3]);
        mesh.surface_add_vertex(c[3]);
        mesh.surface_add_vertex(c[2]);
        mesh.surface_add_vertex(c[2]);
        mesh.surface_add_vertex(c[0]);

        mesh.surface_add_vertex(c[4]);
        mesh.surface_add_vertex(c[5]);
        mesh.surface_add_vertex(c[5]);
        mesh.surface_add_vertex(c[7]);
        mesh.surface_add_vertex(c[7]);
        mesh.surface_add_vertex(c[6]);
        mesh.surface_add_vertex(c[6]);
        mesh.surface_add_vertex(c[4]);

        mesh.surface_add_vertex(c[0]);
        mesh.surface_add_vertex(c[4]);
        mesh.surface_add_vertex(c[1]);
        mesh.surface_add_vertex(c[5]);
        mesh.surface_add_vertex(c[2]);
        mesh.surface_add_vertex(c[6]);
        mesh.surface_add_vertex(c[3]);
        mesh.surface_add_vertex(c[7]);
    };
    mesh.call("surface_begin".into(), &[PrimitiveType::LINES.to_variant()]);
    for grid_index in cloud
        .grid_map()
        .grids()
        .keys()
        .filter(|g| !modified_grids.contains(g))
    {
        add_cube(&mut mesh, *grid_index);
    }
    mesh.surface_end();
    mesh.call("surface_begin".into(), &[PrimitiveType::LINES.to_variant()]);
    for grid_index in modified_grids {
        add_cube(&mut mesh, *grid_index);
    }
    mesh.surface_end();
    mesh.surface_set_material(0, normal);
    mesh.surface_set_material(1, modified);
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
        for grid_index in modified_grids.iter().copied() {
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

        if self.show_debug_mesh {
            if let (Some(normal), Some(modified)) = (
                self.debug_mesh_material_normal.clone(),
                self.debug_mesh_material_modified.clone(),
            ) {
                if let Some(mesh) =
                    create_debug_mesh(&self.cloud, &modified_grids, normal, modified)
                {
                    self.debug_mesh_inst
                        .set_deferred("mesh".into(), mesh.to_variant());
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
            debug_mesh_material_normal: None,
            debug_mesh_material_modified: None,
            show_debug_mesh: false,
            debug_mesh_inst: MeshInstance3D::new_alloc(),
            cloud: vrrop_server::PointCloud::new(1.0),
            meshes: FxHashMap::default(),
            material: None,
        }
    }

    fn ready(&mut self) {
        let mesh = self.debug_mesh_inst.clone();
        self.base_mut().add_child(mesh.upcast());
    }
}
