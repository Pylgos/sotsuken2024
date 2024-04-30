shader_type spatial;
render_mode depth_draw_always;

uniform sampler2D sampler;

uniform vec2 color_f;
uniform vec2 color_pp;

uniform vec2 depth_f;
uniform vec2 depth_pp;

uniform int height;
uniform int width;
uniform uint depth_max;
uniform uint depth_min;

varying vec4 color;

vec3 rgb_to_hsv(vec3 c) {
    vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));

    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

float decode_depth(vec3 c) {
	vec3 hsv = rgb_to_hsv(c);
	if (hsv.y < 0.5 || hsv.z < 0.5) {
		return 0.0;
	}
	float hue = rgb_to_hsv(c).x;
	uint depth = depth_min + uint(float(depth_max - depth_min) * hue);
	return float(depth) * 1e-3;
}

vec2 pixel_to_uv(vec2 pixel) {
	return pixel / vec2(ivec2(width-1, height-1));
}

vec2 id_to_pixel(int id) {
	int x = id % width;
	int y = id / height;
	return vec2(ivec2(x, y));
}

// https://github.com/IntelRealSense/librealsense/blob/5e73f7bb906a3cbec8ae43e888f182cc56c18692/include/librealsense2/rsutil.h#L46
vec3 deproject_pixel_to_point(vec2 pixel, float depth, vec2 pp, vec2 f) {
	float x = (pixel.x - pp.x) / f.x;
	float y = (pixel.y - pp.y) / f.y;
	return vec3(depth * x, depth * y, depth);
}

// https://github.com/IntelRealSense/librealsense/blob/5e73f7bb906a3cbec8ae43e888f182cc56c18692/include/librealsense2/rsutil.h#L15
vec2 project_point_to_pixel(vec3 point, vec2 pp, vec2 f) {
	return (point.xy / point.z) * f + pp;
}

void vertex() {
	POINT_SIZE = 5.0;
	vec2 depth_uv = pixel_to_uv(id_to_pixel(VERTEX_ID));
	vec2 actual_depth_uv = vec2(depth_uv.x, depth_uv.y * 0.5 + 0.5);
	vec3 encoded_depth = texture(sampler, actual_depth_uv).rgb;
	float depth = decode_depth(encoded_depth);
	vec3 point = deproject_pixel_to_point(id_to_pixel(VERTEX_ID), depth, depth_pp, depth_f);
	VERTEX += point;
	
	vec2 color_uv = pixel_to_uv(project_point_to_pixel(point, color_pp, color_f));
	vec2 actual_color_uv = color_uv / vec2(1, 2);
	
	if (depth < 0.01) {
		color = vec4(0, 0, 0, 0);
	} else if (0.0 <= actual_color_uv.x && actual_color_uv.x < 1.0 && 0.0 <= actual_color_uv.y && actual_color_uv.y < 0.5) {
		color = vec4(texture(sampler, actual_color_uv).rgb, 1);
	} else {
		color = vec4(1, 1, 1, 1);
	}
}

void fragment() {
	ALBEDO = color.rgb;
	ALPHA = color.a;
}
