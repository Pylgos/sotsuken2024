#[compute]
#version 450

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

// inputs
layout(push_constant, std430) uniform Params {
	  int num_points;
    vec3 camera_position;
    mat3 camera_rotation;
    mat3 inv_camera_rotation;
    vec2 color_f;
    vec2 color_c;
    vec2 depth_f;
    vec2 depth_c;
    ivec2 color_image_size;
} params;
layout(rgba32f, set = 0, binding = 0) uniform restrict readonly image2D prev_point_positions;
layout(rgba8, set = 1, binding = 0) uniform restrict readonly image2D prev_point_colors;
layout(r16ui, set = 2, binding = 0) uniform restrict readonly uimage2D depth_image;

// outputs
layout(rgba32f, set = 4, binding = 0) uniform restrict writeonly image2D new_point_positions;
layout(rgba8, set = 5, binding = 0) uniform restrict writeonly image2D new_point_colors;
layout(set = 6, binding = 0) buffer new_num_points_buffer {
  int new_num_points;
};

ivec2 point_to_uv(vec3 point, vec2 f, vec2 c, mat3 inv_camera_rotation, vec3 camera_position, ivec2 size) {
    vec3 cam_point = inv_camera_rotation * (point - camera_position);
    if (cam_point.x < 0.0) {
        return ivec2(-1, -1);
    }
    int x = int(f.x * -cam_point.y / cam_point.x + c.x);
    int y = int(f.y * -cam_point.z / cam_point.x + c.y);
    if (0 <= x && x < size.x && 0 <= y && y < size.y) {
        return ivec2(x, y);
    } else {
        return ivec2(-1, -1);
    }
}

ivec2 point_to_color_uv(vec3 point) {
    return point_to_uv(point, params.color_f, params.color_c, params.inv_camera_rotation, params.camera_position, params.color_image_size);
}

ivec2 point_to_depth_uv(vec3 point) {
    return point_to_uv(point, params.depth_f, params.depth_c, params.inv_camera_rotation, params.camera_position, imageSize(depth_image));
}

ivec2 point_index_to_uv(int idx, int row_size) {
    return ivec2(idx % row_size, idx / row_size);
}

void copy_point_to_new_pointcloud(int prev_idx) {
    ivec2 prev_uv = point_index_to_uv(prev_idx, imageSize(prev_point_positions).x);
    int new_idx = atomicAdd(new_num_points, 1);
    ivec2 new_uv = point_index_to_uv(new_idx, imageSize(new_point_positions).x);
    imageStore(new_point_positions, new_uv, imageLoad(prev_point_positions, prev_uv));
    imageStore(new_point_colors, new_uv, imageLoad(prev_point_colors, prev_uv));
}

void main() {
    ivec2 uv = ivec2(gl_GlobalInvocationID.xy);
    int points_row_size = imageSize(prev_point_positions).x;
    int point_idx = uv.x + uv.y * points_row_size;

    if (point_idx >= params.num_points) {
        return;
    }
    vec3 point = imageLoad(prev_point_positions, ivec2(uv)).xyz;
    ivec2 color_uv = point_to_color_uv(point);
    ivec2 depth_uv = point_to_depth_uv(point);
    if (color_uv.x >= 0 && depth_uv.x >= 0) {
        uint depth = imageLoad(depth_image, depth_uv).x;
        if (depth == 0) {
            copy_point_to_new_pointcloud(point_idx);
        }
    } else if (color_uv.x == -1 || depth_uv.x == -1) {
        copy_point_to_new_pointcloud(point_idx);
    }

    atomicAdd(new_num_points, 1);
}

