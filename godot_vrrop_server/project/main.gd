extends Node3D

var server := VrropServer.new()
var pointcloud_guard := Mutex.new()
var pointcloud := PointCloud.new()
var point_cloud_mesh := PointCloudMesh.new()
@onready var visualizer := $Visualizer
@onready var camera_marker := $Visualizer/CameraMarker
const shader = preload("res://point_cloud.gdshader")
var material = ShaderMaterial.new()

# Called when the node enters the scene tree for the first time.
func _ready():
	material.shader = shader
	server.start()
	server.images_received.connect(
		func(image: ImagesMessage):
			print(image)
			update_pointcloud(image)
			#WorkerThreadPool.add_task()
	)
	server.odometry_received.connect(
		func(odom: OdometryMessage):
			camera_marker.position = odom.translation()
			camera_marker.quaternion = odom.rotation()
	)

func update_pointcloud(image: ImagesMessage) -> void:
	pointcloud_guard.lock()
	var prev_pointcloud := pointcloud
	pointcloud_guard.unlock()

	var new_pointcloud = pointcloud.merge_images_msg(image)
	pointcloud_guard.lock()
	pointcloud = new_pointcloud
	pointcloud_guard.unlock()

	var mesh = PointCloudMesh.new()
	mesh.set_pointcloud(new_pointcloud)
	mesh.surface_set_material(0, material)
	visualizer.set_deferred("mesh", mesh)


# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass
