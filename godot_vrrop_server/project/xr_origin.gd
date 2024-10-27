extends XROrigin3D

enum CameraMode {
	FIRST_PERSON,
	THIRD_PERSON,
}

@export var camera_mode := CameraMode.FIRST_PERSON
@export var snap_turn_threshold := 0.5
@export var snap_turn_step := 0.5

@onready var left: XRController3D = $Left
@onready var right: XRController3D = $Right
@onready var camera_marker: Node3D = $"../Visualizer/CameraMarker"
@onready var xr_camera_3d: XRCamera3D = $XRCamera3D
@onready var ui_viewport: Node3D = $Left/UiViewport

var _did_snap_turn := false

func _ready():
	pass

# Base <- XROrigin3D <- XRCamera3D
# Tr = Tr_o * Tr_c
# Tr_o = Tr * Tr_c^-1
func _set_global_hmd_transform(transf: Transform3D) -> void:
	global_transform = transf * xr_camera_3d.transform.inverse()

func _set_global_hmd_position(global_pos: Vector3) -> void:
	_set_global_hmd_transform(Transform3D(xr_camera_3d.global_basis, global_pos))

func _set_global_hmd_yaw(yaw: float) -> void:
	var euler := xr_camera_3d.basis.get_euler()
	_set_global_hmd_transform(
		Transform3D(
			Basis.from_euler(Vector3(euler.x, yaw, euler.z)),
			xr_camera_3d.global_position
		)
	)

func _process(delta):
	var enable_control := true
	var is_move_mode := left.get_float("grip") > 0.5

	match camera_mode:
		CameraMode.FIRST_PERSON:
			_set_global_hmd_position(camera_marker.global_position)
			if is_move_mode:
				var real_camera_euler := camera_marker.global_basis.get_euler()
				_set_global_hmd_yaw(real_camera_euler.y - PI / 2)

		CameraMode.THIRD_PERSON:
			if is_move_mode:
				enable_control = false
				var forward_dir := -left.global_basis.z
				var right_dir := left.global_basis.x
				var primary := left.get_vector2("primary")
				position += (forward_dir * primary.y + right_dir * primary.x) * delta
			
				var right_x = right.get_vector2("primary").x
				if not _did_snap_turn:
					if right_x > snap_turn_threshold:
						_set_global_hmd_yaw(xr_camera_3d.global_basis.get_euler().y - snap_turn_step)
						_did_snap_turn = true
					elif right_x < -snap_turn_step:
						_set_global_hmd_yaw(xr_camera_3d.global_basis.get_euler().y + snap_turn_step)
						_did_snap_turn = true
				elif absf(right_x) < snap_turn_step - 0.1:
					_did_snap_turn = false

	if enable_control:
		var forward := left.get_vector2("primary").y
		var turn := -right.get_vector2("primary").x
		var leg_length := 1 - left.get_float("trigger")
		ControlClient.target_velocity = Vector2(forward, turn)
		ControlClient.leg_length = leg_length

	var hmd_transf := xr_camera_3d.transform
	var ui_direction := (global_transform * hmd_transf.origin - ui_viewport.global_position).normalized()
	var ui_visibility: float = ui_viewport.global_basis.z.dot(ui_direction)
	var ui_visible := ui_visibility > 0.8
	ui_viewport.enabled = ui_visible
	ui_viewport.visible = ui_visible
