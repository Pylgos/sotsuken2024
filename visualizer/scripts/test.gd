extends Node3D

var test := VisualizerTest.new()
@onready var sprite_3d = $Sprite3D

func _process(_delta):
	test.update()
	var texture = test.get_texture()
	if texture != null:
		sprite_3d.texture = TextureWrapper.new(texture)
