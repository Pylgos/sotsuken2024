extends Node
class_name Client

var _client := VrropClient.new()
var _stats_recording := false
var _image_stamps: PackedFloat64Array
var _image_orig_sizes: PackedInt64Array
var _image_latencies: PackedFloat64Array
var _odom_stamps: PackedFloat64Array
var _odom_orig_sizes: PackedInt64Array
var _odom_latencies: PackedFloat64Array

signal images_received(images: ImagesMessage)
signal odometry_received(odometry: OdometryMessage)
signal reset_command_sent()
signal stat_recording_changed()

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
	if _stats_recording:
		var now := Time.get_unix_time_from_system()
		var stamp := images.odometry().stamp()
		var latency = now - stamp
		_image_stamps.push_back(stamp)
		_image_latencies.push_back(latency)
		_image_orig_sizes.push_back(images.original_size())
	images_received.emit(images)

func _on_odometry_received(odometry: OdometryMessage) -> void:
	if _stats_recording:
		var now := Time.get_unix_time_from_system()
		var stamp := odometry.stamp()
		var latency = now - stamp
		_odom_stamps.push_back(stamp)
		_odom_latencies.push_back(latency)
		_odom_orig_sizes.push_back(odometry.original_size())
	odometry_received.emit(odometry)

func send_reset_command() -> void:
	_client.send_reset_command()
	reset_command_sent.emit()

func start_stats_recording() -> void:
	_stats_recording = true
	stat_recording_changed.emit()

func is_stat_recording() -> bool:
	return _stats_recording

func end_stats_recording() -> void:
	_client.send_save_stats_command(
		_image_stamps,
		_image_orig_sizes,
		_image_latencies,
		_odom_stamps,
		_odom_orig_sizes,
		_odom_latencies,
	)
	_image_stamps.clear()
	_image_latencies.clear()
	_image_orig_sizes.clear()
	_odom_stamps.clear()
	_odom_latencies.clear()
	_odom_orig_sizes.clear()
	_stats_recording = false
	stat_recording_changed.emit()
