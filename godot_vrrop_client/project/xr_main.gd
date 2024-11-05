extends Node3D

var xr_interface: XRInterface

@onready var xr_origin_3d = $XROrigin3D
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

	GlobalSettings.grid_size.on_setting_changed.connect(_on_grid_size_changed)
	GlobalSettings.view_type.on_setting_changed.connect(_on_view_type_changed)
	
	_on_grid_size_changed()
	_on_view_type_changed()

	_enable_passthrough(true)

	visualizer.start(GlobalClient)

func _on_grid_size_changed() -> void:
	visualizer.grid_size = GlobalSettings.grid_size.get_value()

func _on_view_type_changed() -> void:
	match GlobalSettings.view_type.get_value():
		"First Person":
			xr_origin_3d.view_type = xr_origin_3d.ViewType.FIRST_PERSON
			visualizer.show_camera_marker = false
		"Third Person":
			xr_origin_3d.view_type = xr_origin_3d.ViewType.THIRD_PERSON
			visualizer.show_camera_marker = true
		_:
			assert(false)

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
