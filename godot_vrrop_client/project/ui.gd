extends Control
class_name VrropUi

@onready var reset_button: Button = %ResetButton
@onready var grid_size_label: Label = %GridSizeLabel
@onready var grid_size_slider: Slider = %GridSizeSlider
@onready var show_grid_button: Button = %ShowGridButton
@onready var view_type_button: OptionButton = %ViewTypeButton
@onready var server_address_edit = %ServerAddressEdit
@onready var server_port_edit = %ServerPortEdit

func _ready():
	reset_button.pressed.connect(
		func():
			GlobalClient.send_reset_command()
	)
	
	grid_size_slider.value_changed.connect(
		func(new_value: float):
			GlobalSettings.grid_size.set_value(new_value)
	)
	GlobalSettings.grid_size.on_setting_changed.connect(_on_grid_size_changed)
	
	show_grid_button.pressed.connect(
		func():
			GlobalSettings.show_grid.set_value(show_grid_button.button_pressed)
	)
	GlobalSettings.show_grid.on_setting_changed.connect(_on_show_grid_changed)
	
	view_type_button.item_selected.connect(_on_view_type_item_selected)
	GlobalSettings.view_type.on_setting_changed.connect(_on_view_type_changed)
	
	server_address_edit.text_submitted.connect(
		func(new_text: String):
			GlobalSettings.server_address.set_value(new_text)
	)
	GlobalSettings.server_address.on_setting_changed.connect(_on_server_address_changed)
	
	server_port_edit.text_submitted.connect(
		func(new_text: String):
			GlobalSettings.server_port.set_value(new_text.to_int())
	)
	GlobalSettings.server_port.on_setting_changed.connect(_on_server_port_changed)

	_on_grid_size_changed()
	_on_show_grid_changed()
	_on_view_type_changed()
	_on_server_address_changed()
	_on_server_port_changed()

func _on_grid_size_changed() -> void:
	var grid_size = GlobalSettings.grid_size.get_value()
	grid_size_label.text = "Grid Size: %3.1f [m]" % grid_size
	grid_size_slider.set_value_no_signal(grid_size)

func _on_show_grid_changed() -> void:
	show_grid_button.set_pressed_no_signal(GlobalSettings.show_grid.get_value())

func _on_server_address_changed() -> void:
	server_address_edit.text = GlobalSettings.server_address.get_value()

func _on_server_port_changed() -> void:
	server_port_edit.text = str(GlobalSettings.server_port.get_value())

func _on_view_type_item_selected(idx: int) -> void:
	GlobalSettings.view_type.set_value(view_type_button.get_item_text(idx))

func _on_view_type_changed() -> void:
	var item := -1
	for idx in range(view_type_button.item_count):
		if GlobalSettings.view_type.get_value() == view_type_button.get_item_text(idx):
			item = idx
	assert(item != -1)
	view_type_button.item_selected.disconnect(_on_view_type_item_selected)
	view_type_button.selected = item
	view_type_button.item_selected.connect(_on_view_type_item_selected)
