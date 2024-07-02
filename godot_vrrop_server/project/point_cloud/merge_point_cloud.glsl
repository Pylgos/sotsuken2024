#[compute]
#version 450

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

// inputs
layout(rgba32f, set = 0, binding = 0) uniform restrict readonly image2D prev_point_positions;
layout(rgba8, set = 1, binding = 0) uniform restrict readonly image2D prev_point_colors;
layout(push_constant, std430) uniform Params {
	uint num_points;
} params;

// outputs
layout(rgba32f, set = 2, binding = 0) uniform restrict writeonly image2D new_point_positions;
layout(rgba8, set = 3, binding = 0) uniform restrict writeonly image2D new_point_colors;
layout(set = 4, binding = 0) atomic_uint num_new_points;

void main() {
  ivec2 uv = ivec2(gl_GlobalInvocationID.xy);
  

}

