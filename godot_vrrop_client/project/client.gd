extends Node
class_name Client

var _client := VrropClient.new()

signal images_received(images: ImagesMessage)
signal odometry_received(odometry: OdometryMessage)
signal reset_command_sent()

func _ready() -> void:
	_client.images_received.connect(_on_images_received)
	_client.odometry_received.connect(_on_odometry_received)
	GlobalSettings.server_address.on_setting_changed.connect(_start)
	GlobalSettings.server_port.on_setting_changed.connect(_start)
	_start()

func _start() -> void:
	var address = "%s:%d" % [GlobalSettings.server_address.get_value(), GlobalSettings.server_port.get_value()]
	print(address)
	_client.start(address)

func _on_images_received(images: ImagesMessage) -> void:
	images_received.emit(images)

func _on_odometry_received(odometry: OdometryMessage) -> void:
	odometry_received.emit(odometry)

func send_reset_command() -> void:
	_client.send_reset_command()
	reset_command_sent.emit()
