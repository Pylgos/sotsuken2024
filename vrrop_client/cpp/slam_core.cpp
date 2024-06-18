#include "CameraRs2D4xx.h"
#include "rtabmap/core/SensorCaptureThread.h"
#include <format>
#include <functional>
#include <memory>
#include <rtabmap/core/CameraThread.h>
#include <rtabmap/core/Odometry.h>
#include <rtabmap/core/OdometryEvent.h>
#include <rtabmap/core/OdometryThread.h>
#include <rtabmap/core/Rtabmap.h>
#include <rtabmap/core/RtabmapThread.h>
#include <rtabmap/utilite/UEventsHandler.h>
#include <rtabmap/utilite/UEventsManager.h>
#include "slam_core.h"


class EventHandler : public UEventsHandler {
public:
  EventHandler(std::function<bool(UEvent*)> f)
    : f_{f} {}

protected:
  virtual bool handleEvent(UEvent *event) override {
    return f_(event);
  }

private:
  std::function<bool(UEvent*)> f_;
};


class SlamCore {
public:
  static SlamCore* create() {
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

      auto ret = new SlamCore;
      ret->sensor_thread_ = std::make_unique<rtabmap::SensorCaptureThread>(camera);
      ret->sensor_thread_->enableIMUFiltering(1, rtabmap::ParametersMap(), true);
      ret->odom_thread_ = std::make_unique<rtabmap::OdometryThread>(rtabmap::Odometry::create());
      rtabmap::ParametersMap params;
      auto rtabmap = new rtabmap::Rtabmap{};
      rtabmap->init(params);
      ret->rtabmap_thread_ = std::make_unique<rtabmap::RtabmapThread>(rtabmap);

      ret->event_handler_ = std::make_unique<EventHandler>(std::bind(&SlamCore::handle_event, ret, std::placeholders::_1));
      ret->odom_thread_->registerToEventsManager();
      ret->rtabmap_thread_->registerToEventsManager();
      ret->event_handler_->registerToEventsManager();

      UEventsManager::createPipe(ret->sensor_thread_.get(), ret->odom_thread_.get(), "CameraEvent");
      ret->rtabmap_thread_->start();
      ret->odom_thread_->start();
      ret->sensor_thread_->start();

      return ret;
    } catch(std::exception& e) {
      UERROR("exception: %s", e.what());
      return nullptr;
    }
  }

  ~SlamCore() {
    event_handler_->unregisterFromEventsManager();
    rtabmap_thread_->unregisterFromEventsManager();
    odom_thread_->unregisterFromEventsManager();
    rtabmap_thread_->join(true);
    odom_thread_->join(true);
    sensor_thread_->join(true);
  }

  bool handle_event(UEvent* event) {
    if (event->getClassName() == "OdometryEvent") {
      rtabmap::OdometryEvent *odom_event =
          static_cast<rtabmap::OdometryEvent *>(event);
      auto pose = odom_event->pose();
      if (pose.isNull()) {
        std::cout << "pose is null" << std::endl;
      } else {
        float x, y, z;
        pose.getTranslation(x, y, z);
        std::cout << std::format("x: {:7.3f} y: {:7.3f} z: {:7.3f} color: {}", x, y, z, odom_event->data().imageRaw().data != nullptr) << std::endl;
        // std::cout << odom_event->data().userDataRaw() << std::endl;
        // cv::imencode(const String &ext, InputArray img, std::vector<uchar>
        // std::cout << odom_event->data().imageRaw() != nullptr << std::endl;
      }
    }
    return false;
  }

private:
  std::unique_ptr<rtabmap::SensorCaptureThread> sensor_thread_;
  std::unique_ptr<rtabmap::OdometryThread> odom_thread_;
  std::unique_ptr<rtabmap::RtabmapThread> rtabmap_thread_;
  std::unique_ptr<EventHandler> event_handler_;
};


extern "C" {
  typedef void* slam_core_ptr_t;
  
  slam_core_ptr_t slam_core_create() {
    return SlamCore::create();
  }
  
  void slam_core_delete(slam_core_ptr_t p) {
    delete reinterpret_cast<SlamCore*>(p);
  }
}
