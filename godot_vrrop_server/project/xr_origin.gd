extends XROrigin3D

@onready var left = $Left
@onready var right = $Right
@onready var camera_marker = $"../Visualizer/CameraMarker"
@onready var xr_camera_3d = $XRCamera3D

# Called when the node enters the scene tree for the first time.
func _ready():
	pass

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	var hmd_transf = XRServer.get_hmd_transform()
	var real_camera_pos: Vector3 = camera_marker.global_position
	global_position += real_camera_pos - global_transform * hmd_transf.origin
	if right.is_button_pressed("ax_button"):
		var real_camera_rot: Basis = camera_marker.basis
		basis = hmd_transf.basis.inverse() * real_camera_rot
