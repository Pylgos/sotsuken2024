shader_type spatial;
render_mode unshaded;


void vertex() {
	float size = COLOR.a * 15.0;
	vec3 vertex_pos_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	float dist = length(CAMERA_POSITION_WORLD - vertex_pos_world);
	POINT_SIZE = size / dist;
}

void fragment() {
	ALBEDO = COLOR.rgb;
}

