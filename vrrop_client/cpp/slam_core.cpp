#include "slam_core.h"
#include "CameraRs2D4xx.h"
#include <cstring>
#include <functional>
#include <memory>
#include <rtabmap/core/CameraModel.h>
#include <rtabmap/core/CameraThread.h>
#include <rtabmap/core/IMUFilter.h>
#include <rtabmap/core/Odometry.h>
#include <rtabmap/core/OdometryEvent.h>
#include <rtabmap/core/OdometryThread.h>
#include <rtabmap/core/Parameters.h>
#include <rtabmap/core/Rtabmap.h>
#include <rtabmap/core/RtabmapThread.h>
#include <rtabmap/core/SensorCaptureThread.h>
#include <rtabmap/utilite/UEventsHandler.h>
#include <rtabmap/utilite/UEventsManager.h>

struct slam_core_image {
  cv::Mat mat;
};

class EventHandler : public UEventsHandler {
public:
  EventHandler(std::function<bool(UEvent *)> f) : f_{f} {}

protected:
  virtual bool handleEvent(UEvent *event) override { return f_(event); }

private:
  std::function<bool(UEvent *)> f_;
};

struct slam_core {
public:
  using OdometryCallback = std::function<void(slam_core_odometry_event_t *)>;

  static slam_core *create() {
    try {
      ULogger::setType(ULogger::kTypeConsole);
      ULogger::setLevel(ULogger::kWarning);

      auto camera = new CameraRs2D4xx{};
      camera->setColorResolution(640, 480, 60);
      camera->setIrDepthResolution(640, 480, 60);
      if (!camera->init()) {
        std::cout << "camera initialization failed" << std::endl;
        return nullptr;
      }

      auto ret = new slam_core;

      ret->color_intrinsics_ = camera->getRgbModel();
      ret->depth_intrinsics_ = camera->getIrDepthModel();

      ret->sensor_thread_ =
          std::make_unique<rtabmap::SensorCaptureThread>(camera);
      ret->sensor_thread_->enableIMUFiltering(
          rtabmap::IMUFilter::Type::kMadgwick, rtabmap::ParametersMap(), true);
      auto odometry = rtabmap::Odometry::create(rtabmap::ParametersMap());

      ret->odom_thread_ = std::make_unique<rtabmap::OdometryThread>(odometry);
      rtabmap::ParametersMap params;
      auto rtabmap = new rtabmap::Rtabmap{};
      rtabmap->init(params);
      ret->rtabmap_thread_ = std::make_unique<rtabmap::RtabmapThread>(rtabmap);

      ret->event_handler_ = std::make_unique<EventHandler>(
          std::bind(&slam_core::handle_event, ret, std::placeholders::_1));
      ret->odom_thread_->registerToEventsManager();
      ret->rtabmap_thread_->registerToEventsManager();
      ret->event_handler_->registerToEventsManager();

      UEventsManager::createPipe(ret->sensor_thread_.get(),
                                 ret->odom_thread_.get(), "CameraEvent");
      ret->rtabmap_thread_->start();
      ret->odom_thread_->start();
      ret->sensor_thread_->start();

      return ret;
    } catch (std::exception &e) {
      UERROR("exception: %s", e.what());
      return nullptr;
    }
  }

  ~slam_core() {
    event_handler_->unregisterFromEventsManager();
    rtabmap_thread_->unregisterFromEventsManager();
    odom_thread_->unregisterFromEventsManager();
    rtabmap_thread_->join(true);
    odom_thread_->join(true);
    sensor_thread_->join(true);
  }

  void register_odometry_event_handler(OdometryCallback callback) {
    odometry_callback_ = callback;
  }

  rtabmap::CameraModel get_color_intrinsics() { return color_intrinsics_; }
  rtabmap::CameraModel get_depth_intrinsics() { return depth_intrinsics_; }

private:
  bool handle_event(UEvent *event) {
    if (event->getClassName() == "OdometryEvent") {
      rtabmap::OdometryEvent *odom_event =
          static_cast<rtabmap::OdometryEvent *>(event);
      if (odometry_callback_ == nullptr)
        return false;
      slam_core_odometry_event_t ev;
      memset(&ev, 0, sizeof(ev));
      auto pose = odom_event->pose();
      if (pose.isNull()) {
        std::cout << "pose is null" << std::endl;
        ev.translation[0] = std::numeric_limits<double>::quiet_NaN();
        ev.translation[1] = std::numeric_limits<double>::quiet_NaN();
        ev.translation[2] = std::numeric_limits<double>::quiet_NaN();
        ev.rotation[0] = std::numeric_limits<double>::quiet_NaN();
        ev.rotation[1] = std::numeric_limits<double>::quiet_NaN();
        ev.rotation[2] = std::numeric_limits<double>::quiet_NaN();
        ev.rotation[3] = std::numeric_limits<double>::quiet_NaN();
      } else {
        ev.translation[0] = pose.x();
        ev.translation[1] = pose.y();
        ev.translation[2] = pose.z();
        auto q = pose.getQuaterniond();
        ev.rotation[0] = q.w();
        ev.rotation[1] = q.x();
        ev.rotation[2] = q.y();
        ev.rotation[3] = q.z();
      }
      ev.color = new slam_core_image_t{odom_event->data().userDataRaw()};
      ev.depth = new slam_core_image_t{odom_event->data().depthRaw()};
      odometry_callback_(&ev);
    }
    return false;
  }

  OdometryCallback odometry_callback_;
  rtabmap::CameraModel color_intrinsics_;
  rtabmap::CameraModel depth_intrinsics_;
  std::unique_ptr<rtabmap::SensorCaptureThread> sensor_thread_;
  std::unique_ptr<rtabmap::OdometryThread> odom_thread_;
  std::unique_ptr<rtabmap::RtabmapThread> rtabmap_thread_;
  std::unique_ptr<EventHandler> event_handler_;
};

extern "C" {

slam_core_t *slam_core_create() { return slam_core::create(); }
void slam_core_delete(slam_core_t *p) { delete p; }

void slam_core_get_intrinstics(
    slam_core_t *p, slam_core_camera_intrinsics_t *color_intrinsics,
    slam_core_camera_intrinsics_t *depth_intrinsics) {
  auto color = p->get_color_intrinsics();
  auto depth = p->get_depth_intrinsics();
  if (color_intrinsics) {
    color_intrinsics->fx = color.fx();
    color_intrinsics->fy = color.fy();
    color_intrinsics->cx = color.cx();
    color_intrinsics->cy = color.cy();
    color_intrinsics->width = color.imageWidth();
    color_intrinsics->height = color.imageHeight();
  }
  if (depth_intrinsics) {
    depth_intrinsics->fx = depth.fx();
    depth_intrinsics->fy = depth.fy();
    depth_intrinsics->cx = depth.cx();
    depth_intrinsics->cy = depth.cy();
    depth_intrinsics->width = depth.imageWidth();
    depth_intrinsics->height = depth.imageHeight();
  }
}

void slam_core_register_odometry_event_handler(
    slam_core_t *p, void *userdata, slam_core_event_handler_t handler) {
  p->register_odometry_event_handler(
      [=](slam_core_odometry_event_t *ev) { handler(userdata, ev); });
}

uint32_t slam_core_image_get_width(slam_core_image_t *image) {
  return image->mat.cols;
}

uint32_t slam_core_image_get_height(slam_core_image_t *image) {
  return image->mat.rows;
}

size_t slam_core_image_get_size(slam_core_image_t *image) {
  return image->mat.step[0] * image->mat.rows;
}

void *slam_core_image_get_data(slam_core_image_t *image) {
  return image->mat.data;
}

void slam_core_image_destroy(slam_core_image_t *image) { delete image; }
}
#undef THIS
