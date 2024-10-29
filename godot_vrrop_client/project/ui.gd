extends Control
class_name VrropUi

signal reset_point_cloud()
signal grid_size_changed()
signal show_grid_changed()
signal camera_mode_changed()

var grid_size: float:
	set(value):
		%GridSizeSlider.value = value
	get:
		return %GridSizeSlider.value
var show_grid: bool:
	set(value):
		%ShowGridButton.button_pressed = value
	get:
		return %ShowGridButton.button_pressed
var camera_mode: int:
	set(value):
		%ViewTypeButton.select(value)
	get:
		return %CameraModeButton.selected

@onready var reset_point_cloud_button: Button = %ResetPointCloudButton
@onready var grid_size_label: Label = %GridSizeLabel
@onready var grid_size_slider: Slider = %GridSizeSlider
@onready var show_grid_button: Button = %ShowGridButton
@onready var camera_mode_button: OptionButton = %CameraModeButton

# Called when the node enters the scene tree for the first time.
func _ready():
	reset_point_cloud_button.pressed.connect(
		func():
			reset_point_cloud.emit()
	)
	grid_size_slider.value_changed.connect(
		func(_new_value: float):
			grid_size_label.text = "Grid Size: %3.1f [m]" % grid_size
			grid_size_changed.emit()
	)
	show_grid_button.pressed.connect(
		func():
			show_grid_changed.emit()
	)
	grid_size_label.text = "Grid Size: %3.1f [m]" % grid_size
	camera_mode_button.item_selected.connect(
		func(_idx: int):
			camera_mode_changed.emit()
	)
