extends Node3D

@export var material: Material
@onready var mesh_instance_3d = $MeshInstance3D

var _view_mesh := ImmediateMesh.new()

var _view_origin := Vector3(0, 0, 0)
var _view_rotation := Quaternion.IDENTITY
# color_intrinsics: CameraIntrinsics { width: 640, height: 480, fx: 611.76965, fy: 612.0515, cx: 323.71402, cy: 250.61005 }
var _max_depth := 10.0
var _view_width := 640
var _view_height := 480
var _view_fx := 611.76965
var _view_fy := 612.0515
var _view_cx := 323.71402
var _view_cy := 250.61005

func pixel_to_point(pixel: Vector2, depth: float, ext: Transform3D) -> Vector3:
	var y := -(pixel.x - _view_cx) / _view_fx;
	var z := -(pixel.y - _view_cy) / _view_fy;
	return ext * Vector3(depth, y * depth, z * depth)

func _ready():
	_view_mesh.clear_surfaces()
	_view_mesh.surface_begin(Mesh.PRIMITIVE_LINES)
	var transf := Transform3D(Basis(_view_rotation), _view_origin)
	_view_mesh.surface_add_vertex(_view_origin)
	_view_mesh.surface_add_vertex(pixel_to_point(Vector2(0, 0), _max_depth, transf))
	_view_mesh.surface_add_vertex(_view_origin)
	_view_mesh.surface_add_vertex(pixel_to_point(Vector2(_view_width, 0), _max_depth, transf))
	_view_mesh.surface_add_vertex(_view_origin)
	_view_mesh.surface_add_vertex(pixel_to_point(Vector2(0, _view_height), _max_depth, transf))
	_view_mesh.surface_add_vertex(_view_origin)
	_view_mesh.surface_add_vertex(pixel_to_point(Vector2(_view_width, _view_height), _max_depth, transf))
	_view_mesh.surface_end()
	_view_mesh.surface_set_material(0, material)
	mesh_instance_3d.mesh = _view_mesh
