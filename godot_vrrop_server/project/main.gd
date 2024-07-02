extends Node3D

var server := VrropServer.new()
@onready var camera_marker := $Visualizer/CameraMarker

var xr_interface: XRInterface

# Called when the node enters the scene tree for the first time.
func _ready():
	xr_interface = XRServer.find_interface("OpenXR")
	if xr_interface and xr_interface.is_initialized():
		print("OpenXR initialized successfully")

		# Turn off v-sync!
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)

		# Change our main viewport to output to the HMD
		get_viewport().use_xr = true
	else:
		print("OpenXR not initialized, please check if your headset is connected")

	
	server.start()
	server.images_received.connect(
		func(image: ImagesMessage):
			print(image)
	)
	server.odometry_received.connect(
		func(odom: OdometryMessage):
			camera_marker.position = odom.translation()
			camera_marker.quaternion = odom.rotation()
	)

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass
