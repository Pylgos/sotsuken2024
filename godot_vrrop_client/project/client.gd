extends Node
class_name Client

var _client := VrropClient.new()

signal images_received(images: ImagesMessage)
signal odometry_received(odometry: OdometryMessage)
signal reset_command_sent()

func _ready() -> void:
	_client.images_received.connect(images_received.emit)
	_client.odometry_received.connect(odometry_received.emit)
	GlobalSettings.server_address.on_setting_changed.connect(_start)
	GlobalSettings.server_port.on_setting_changed.connect(_start)
	_start()

func _start() -> void:
	var address = "%s:%d" % [GlobalSettings.server_address.get_value(), GlobalSettings.server_port.get_value()]
	_client.start(address)

func send_reset_command() -> void:
	_client.send_reset_command()
	reset_command_sent.emit()
