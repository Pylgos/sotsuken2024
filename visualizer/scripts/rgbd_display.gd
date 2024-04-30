@tool
class_name RGBDDisplay
extends MeshInstance3D

@export var texture: Texture2D:
	set(value):
		_mat.set_shader_parameter("sampler", value)
		var height := value.get_height() / 2
		var width := value.get_width()
		if height == _prev_height and width == _prev_width:
			return
		_prev_height = height
		_prev_width = width
		_mat.set_shader_parameter("height", height)
		_mat.set_shader_parameter("width", width)
		_rebuild_surface(width, height)

@export var color_f: Vector2:
	set(value):
		_mat.set_shader_parameter("color_f", value)
	get:
		return _mat.get_shader_parameter("color_f")
@export var color_pp: Vector2:
	set(value):
		_mat.set_shader_parameter("color_pp", value)
	get:
		return _mat.get_shader_parameter("color_pp")

@export var depth_f: Vector2:
	set(value):
		_mat.set_shader_parameter("depth_f", value)
	get:
		return _mat.get_shader_parameter("depth_f")
@export var depth_pp: Vector2:
	set(value):
		_mat.set_shader_parameter("depth_pp", value)
	get:
		return _mat.get_shader_parameter("depth_pp")

@export var depth_min: int:
	set(value):
		_mat.set_shader_parameter("depth_min", value)
	get:
		return _mat.get_shader_parameter("depth_min")

@export var depth_max: int:
	set(value):
		_mat.set_shader_parameter("depth_max", value)
	get:
		return _mat.get_shader_parameter("depth_max")

const shader := preload("res://shaders/rgbd_display.gdshader")

var _mesh := ImmediateMesh.new()
var _mat := ShaderMaterial.new()
var _prev_width := -1
var _prev_height := -1

func _rebuild_surface(width: int, height: int) -> void:
	_mesh.clear_surfaces()
	_mesh.surface_begin(Mesh.PRIMITIVE_POINTS)
	for y in range(height):
		for x in range(width):
			_mesh.surface_set_uv(Vector2(float(x) / height, float(y) / height))
			_mesh.surface_add_vertex(Vector3(0, 0, 0))
	_mesh.surface_end()
	_mesh.surface_set_material(0, _mat)

func _init():
	_rebuild_surface(1, 1)
	_mat.shader = shader
	mesh = _mesh
