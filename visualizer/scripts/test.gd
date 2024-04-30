extends Node3D

var test := VisualizerTest.new()
@onready var mesh_instance_3d = $MeshInstance3D

var mesh = PlaneMesh.new()

func _process(_delta):
	test.update()
	var texture = test.get_texture()
	if texture != null:
		mesh_instance_3d.texture = TextureWrapper.new(texture)
