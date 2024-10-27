extends XROrigin3D

@onready var left := $Left
@onready var right := $Right
@onready var camera_marker := $"../Visualizer/CameraMarker"
@onready var xr_camera_3d := $XRCamera3D
@onready var ui_viewport := $Left/UiViewport

func _ready():
	pass

func _process(_delta):
	var hmd_transf := XRServer.get_hmd_transform()
	var real_camera_pos: Vector3 = camera_marker.global_position
	global_position += real_camera_pos - global_transform * hmd_transf.origin

	# Reset camera yaw
	if right.is_button_pressed("ax_button"):
		var real_camera_rot: Basis = camera_marker.basis
		var rot := hmd_transf.basis.inverse() * real_camera_rot
		basis = Basis.from_euler(Vector3(0, rot.get_euler().y, 0))

	var forward: float = left.get_vector2("primary").y
	var turn: float = -right.get_vector2("primary").x
	ControlClient.target_velocity = Vector2(forward, turn)

	var ui_direction: Vector3 = (global_transform * hmd_transf.origin - ui_viewport.global_position).normalized()
	var ui_visibility: float = ui_viewport.global_basis.z.dot(ui_direction)
	var ui_visible := ui_visibility > 0.8
	ui_viewport.enabled = ui_visible
	ui_viewport.visible = ui_visible
