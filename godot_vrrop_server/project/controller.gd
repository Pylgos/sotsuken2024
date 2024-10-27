extends Node

@export var address := "127.0.0.1:23456"

var target_velocity := Vector2.ZERO
var leg_length := 1.0

var _client: VrropControlClient = null

func _ready():
	connect_to_server()

func connect_to_server() -> void:
	var client = VrropControlClient.new()
	var err := client.connect_to_server(address)
	if err == OK:
		_client = client
	else:
		push_warning("Failed to connect to ", address, " ", error_string(err))

func _process(_delta):
	if _client == null: return
	_client.set_target_velocity(target_velocity.x,  target_velocity.y)
	_client.set_leg_length(leg_length)
