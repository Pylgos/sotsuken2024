extends Node3D

var server := VrropServer.new()
@onready var point_cloud_visualizer := $PointCloudVisualizer
@onready var camera_marker := $CameraMarker
const _shader = preload("res://point_cloud.gdshader")
var _visualizer_lock := Mutex.new()
var _material = ShaderMaterial.new()

# Called when the node enters the scene tree for the first time.
func _ready():
	_material.shader = _shader
	point_cloud_visualizer.set_material(_material)
	server.start()
	server.images_received.connect(
		func(image: ImagesMessage):
			WorkerThreadPool.add_task(
				func():
					_visualizer_lock.lock()
					point_cloud_visualizer.add_image(image)
					_visualizer_lock.unlock()
			)
	)
	server.odometry_received.connect(
		func(odom: OdometryMessage):
			camera_marker.position = odom.translation()
			camera_marker.quaternion = odom.rotation()
	)
