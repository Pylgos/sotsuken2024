extends Node3D
class_name Visualizer

@export var cloud_debug_material_normal: Material
@export var cloud_debug_material_modified: Material
@export var show_grid := false:
	set(value):
		_visualizer_lock.lock()
		if _visualizer:
			_visualizer.show_debug_mesh = value
		_visualizer_lock.unlock()
	get:
		var tmp: bool
		_visualizer_lock.lock()
		if _visualizer:
			tmp = _visualizer.show_debug_mesh
		else:
			tmp = false
		_visualizer_lock.unlock()
		return tmp
@export var grid_size := 1.0
@export var show_camera_marker := true:
	set(value):
		_camera_marker.visible = value
	get:
		return _camera_marker.visible

@onready var _camera_marker := $CameraMarker
var _visualizer: PointCloudVisualizer
var _visualizer_lock := Mutex.new()
var _material := ShaderMaterial.new()
const _shader := preload("res://point_cloud.gdshader")

func _init_visualizer():
	_visualizer_lock.lock()
	if _visualizer != null:
		_visualizer.queue_free()
	_visualizer_lock.unlock()

	var vis := PointCloudVisualizer.new()
	vis.material = _material
	vis.debug_mesh_material_normal = cloud_debug_material_normal
	vis.debug_mesh_material_modified = cloud_debug_material_modified
	vis.show_debug_mesh = show_grid
	vis.grid_size = grid_size
	vis.init()
	add_child(vis)

	_visualizer_lock.lock()
	_visualizer = vis
	_visualizer_lock.unlock()

func _ready():
	_material.shader = _shader
	_init_visualizer()

func start(client: Client) -> void:
	client.images_received.connect(
		func(image: ImagesMessage):
			WorkerThreadPool.add_task(
				func():
					_visualizer_lock.lock()
					var time := _visualizer.add_image(image)
					print(time)
					_visualizer_lock.unlock()
			)
	)
	client.odometry_received.connect(
		func(odom: OdometryMessage):
			_camera_marker.position = odom.translation()
			_camera_marker.quaternion = odom.rotation()
	)
	client.reset_command_sent.connect(
		func():
			self.reset()
	)

func reset() -> void:
	_init_visualizer()
