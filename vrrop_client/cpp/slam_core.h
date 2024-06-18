#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct slam_core slam_core_t;
typedef struct slam_core slam_core_t;
typedef struct slam_core_image slam_core_image_t;

typedef struct slam_core_camera_intrinsics {
  double fx;
  double fy;
  double cx;
  double cy;
  size_t width;
  size_t height;
} slam_core_camera_intrinsics_t;

typedef struct slam_core_odometry_event {
  double translation[3];
  double rotation[4];
  slam_core_image_t* color;
  slam_core_image_t* depth;
} slam_core_odometry_event_t;

typedef void(*slam_core_event_handler_t)(void* userdata, const slam_core_odometry_event_t* event);

slam_core_t* slam_core_create();
void slam_core_delete(slam_core_t* p);
void slam_core_get_intrinstics(slam_core_t* p, slam_core_camera_intrinsics_t* color_intrinsics, slam_core_camera_intrinsics_t* depth_intrinsics);
void slam_core_register_odometry_event_handler(slam_core_t* p, void* userdata, slam_core_event_handler_t handler);

uint32_t slam_core_image_get_width(slam_core_image_t* image);
uint32_t slam_core_image_get_height(slam_core_image_t* image);
size_t slam_core_image_get_size(slam_core_image_t* image);
void* slam_core_image_get_data(slam_core_image_t* image);
void slam_core_image_destroy(slam_core_image_t* image);

#ifdef __cplusplus
}
#endif
