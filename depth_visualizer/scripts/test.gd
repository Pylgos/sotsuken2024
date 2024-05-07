extends Node3D

var test := VisualizerTest.new()
#@onready var mesh_instance_3d = $MeshInstance3D
@onready var mesh_instance_3d = $XROrigin3D/XRCamera3D/MeshInstance3D

var mesh = PlaneMesh.new()

var xr_interface: XRInterface

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


func _process(_delta):
	test.update()
	var texture = test.get_texture()
	if texture != null:
		mesh_instance_3d.texture = texture
