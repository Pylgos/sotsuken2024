extends Node3D

var server = VrropServer.new()
var pointcloud = PointCloud.new()
var point_cloud_mesh = PointCloudMesh.new()
@onready var visualizer := $Visualizer
@onready var camera_marker := $Visualizer/CameraMarker
const shader = preload("res://point_cloud.gdshader")

# Called when the node enters the scene tree for the first time.
func _ready():
	server.start()
	server.images_received.connect(
		func(image: ImagesMessage):
			print(image)
			WorkerThreadPool.add_task(
				func():
					pointcloud.merge_images_msg(image)
					var mesh = PointCloudMesh.new()
					mesh.set_pointcloud(pointcloud)
					var mat = ShaderMaterial.new()
					mat.shader = shader
					mesh.surface_set_material(0, mat)
					visualizer.set_deferred("mesh", mesh)
			)
	)
	server.odometry_received.connect(
		func(odom: OdometryMessage):
			camera_marker.position = odom.translation()
			camera_marker.quaternion = odom.rotation()
	)


# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass
