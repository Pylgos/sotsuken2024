extends SettingsCollection
class_name Settings

var server_address := StringSetting.new("Server Address", "Client", "Address of the server to connect to", "127.0.0.1")
var server_port := IntSetting.new("Server Port", "Client", "Port number of the server to connect to", 6677, 1, 65535)

var grid_size := FloatSetting.new("Grid Size", "Visualizer", "Grid size of the visualizer", 1.0)
var show_grid := BoolSetting.new("Show Grid", "Visualizer", "Whether to display the grid or not", false)
var view_type := MultiChoiceSetting.new("View Type", "Visualizer", "Type of the View", "Third Person", ["First Person", "Third Person"])

const _PATH = "user://global_settings.gson"

func _init():
	add_setting(server_address)
	add_setting(server_port)
	add_setting(grid_size)
	add_setting(show_grid)
	add_setting(view_type)

	if FileAccess.file_exists(_PATH):
		load_from_GSON(_PATH)

func _exit_tree():
	save_to_GSON(_PATH)
