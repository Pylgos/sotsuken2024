extends Node3D

var xr_interface: XRInterface

@onready var xr_origin_3d = $XROrigin3D
@onready var ui_viewport = $XROrigin3D/Left/UiViewport
@onready var visualizer = $Visualizer
@onready var world_environment = $WorldEnvironment

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
	ui.camera_mode_changed.connect(
		func():
			xr_origin_3d.camera_mode = ui.camera_mode
			visualizer.show_camera_marker = xr_origin_3d.camera_mode != xr_origin_3d.CameraMode.FIRST_PERSON
	)
	
	_enable_passthrough(true)

func _enable_passthrough(enable: bool) -> void:
	var openxr_interface: OpenXRInterface = XRServer.find_interface("OpenXR")

	# Enable passthrough if true and XR_ENV_BLEND_MODE_ALPHA_BLEND is supported.
	# Otherwise, set environment to non-passthrough settings.
	if enable and openxr_interface.get_supported_environment_blend_modes().has(XRInterface.XR_ENV_BLEND_MODE_ALPHA_BLEND):
		get_viewport().transparent_bg = true
		world_environment.environment.background_mode = Environment.BG_COLOR
		world_environment.environment.background_color = Color(0.0, 0.0, 0.0, 0.0)
		openxr_interface.environment_blend_mode = XRInterface.XR_ENV_BLEND_MODE_ALPHA_BLEND
	else:
		get_viewport().transparent_bg = false
		world_environment.environment.background_mode = Environment.BG_SKY
		openxr_interface.environment_blend_mode = XRInterface.XR_ENV_BLEND_MODE_OPAQUE
