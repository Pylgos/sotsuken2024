class_name TextureWrapper
extends Texture2D

var _tex: ExtGLTexture

func _init(tex: ExtGLTexture):
	_tex = tex

func _get_rid():
	return _tex.__get_rid()

func _draw(to_canvas_item, pos, modulate, transpose):
	_tex._draw(to_canvas_item, pos, modulate, transpose)

func _draw_rect(to_canvas_item, rect, tile, modulate, transpose):
	_tex._draw_rect(to_canvas_item, rect, tile, modulate, transpose)

func _draw_rect_region(to_canvas_item, rect, src_rect, modulate, transpose, clip_uv):
	_tex._draw_rect_region(to_canvas_item, rect, src_rect, modulate, transpose, clip_uv)

func _get_height():
	return _tex.get_height()

func _get_width():
	return _tex.get_width()

func _has_alpha():
	return _tex.has_alpha()

func _is_pixel_opaque(x, y):
	return _tex.is_pixel_opaque(x, y)
