extends Node3D

var xr_interface: XRInterface

@onready var ui_viewport = $XROrigin3D/Left/UiViewport
@onready var visualizer = $Visualizer

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

	var ui: VrropUi = ui_viewport.get_scene_instance()

	ui.grid_size_changed.connect(
		func():
			visualizer.grid_size = ui.grid_size
	)
	ui.reset_point_cloud.connect(
		func():
			visualizer.reset_point_cloud()
	)
