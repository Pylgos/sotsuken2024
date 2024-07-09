extends Node3D

var server := VrropServer.new()
@onready var point_cloud_visualizer := $PointCloudVisualizer
@onready var camera_marker := $CameraMarker
const shader = preload("res://point_cloud.gdshader")
var material = ShaderMaterial.new()

# Called when the node enters the scene tree for the first time.
func _ready():
	material.shader = shader
	point_cloud_visualizer.set_material(material)
	server.start()
	server.images_received.connect(
		func(image: ImagesMessage):
			point_cloud_visualizer.add_image(image)
	)
	server.odometry_received.connect(
		func(odom: OdometryMessage):
			camera_marker.position = odom.translation()
			camera_marker.quaternion = odom.rotation()
	)

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass
