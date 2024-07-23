extends Node3D

@export var cloud_debug_material_normal: Material
@export var cloud_debug_material_modified: Material
@export var ui: VrropUi

@onready var _camera_marker := $CameraMarker
var _server := VrropServer.new()
var _visualizer: PointCloudVisualizer
var _visualizer_lock := Mutex.new()
var _material = ShaderMaterial.new()
const _shader = preload("res://point_cloud.gdshader")

func _init_visualizer():
	_visualizer_lock.lock()
	if _visualizer != null:
		_visualizer.queue_free()
	_visualizer_lock.unlock()

	var vis := PointCloudVisualizer.new()
	vis.material = _material
	vis.debug_mesh_material_normal = cloud_debug_material_normal
	vis.debug_mesh_material_modified = cloud_debug_material_modified
	vis.show_debug_mesh = ui.show_grid
	vis.grid_size = ui.grid_size
	vis.init()
	add_child(vis)

	_visualizer_lock.lock()
	_visualizer = vis
	_visualizer_lock.unlock()

# Called when the node enters the scene tree for the first time.
func _ready():
	_material.shader = _shader
	_init_visualizer()
	_server.start()
	_server.images_received.connect(
		func(image: ImagesMessage):
			WorkerThreadPool.add_task(
				func():
					_visualizer_lock.lock()
					_visualizer.add_image(image)
					_visualizer_lock.unlock()
			)
	)
	_server.odometry_received.connect(
		func(odom: OdometryMessage):
			_camera_marker.position = odom.translation()
			_camera_marker.quaternion = odom.rotation()
	)

	ui.reset_point_cloud.connect(
		func():
			_init_visualizer()
	)
	ui.show_grid_changed.connect(
		func():
			_visualizer_lock.lock()
			_visualizer.show_debug_mesh = ui.show_grid
			_visualizer_lock.unlock()
	)
