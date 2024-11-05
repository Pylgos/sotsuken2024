/*
Copyright (c) 2010-2016, Mathieu Labbe - IntRoLab - Universite de Sherbrooke
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:
    * Redistributions of source code must retain the above copyright
      notice, this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above copyright
      notice, this list of conditions and the following disclaimer in the
      documentation and/or other materials provided with the distribution.
    * Neither the name of the Universite de Sherbrooke nor the
      names of its contributors may be used to endorse or promote products
      derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

#pragma once

#include "rtabmap/core/Camera.h"
#include "rtabmap/core/CameraModel.h"
#include "rtabmap/core/Version.h"

#include <pcl/pcl_config.h>

#ifdef RTABMAP_REALSENSE2
#include <librealsense2/hpp/rs_frame.hpp>
#include <librealsense2/rs.hpp>
#endif

namespace rs2 {
class context;
class device;
class syncer;
} // namespace rs2
struct rs2_intrinsics;
struct rs2_extrinsics;

class CameraRs2D4xx : public rtabmap::Camera {
public:
  static bool available();

public:
  CameraRs2D4xx(const std::string &deviceId = "", float imageRate = 0,
                const rtabmap::Transform &localTransform =
                    rtabmap::Transform::getIdentity());
  virtual ~CameraRs2D4xx();

  virtual bool init(const std::string &calibrationFolder = ".",
                    const std::string &cameraName = "");
  virtual bool isCalibrated() const;
  virtual std::string getSerial() const;
  virtual bool odomProvided() const;
  virtual bool getPose(double stamp, rtabmap::Transform &pose,
                       cv::Mat &covariance, double maxWaitTime = 0.06);

  // parameters are set during initialization
  // D400 series
  void setColorResolution(int width, int height, int fps = 30);
  void setIrDepthResolution(int width, int height, int fps = 30);
  void setGlobalTimeSync(bool enabled);

  void setJsonConfig(const std::string &json);
  rtabmap::CameraModel getIrDepthModel() { return ir_depth_model_; };
  rtabmap::CameraModel getRgbModel() { return rgb_model_; };

private:
  void close();
  void imu_callback(rs2::frame frame);
  void frame_callback(rs2::frame frame);
  void multiple_message_callback(rs2::frame frame);
  void getIMU(const double &stamp, rtabmap::IMU &imu, int maxWaitTimeMs = 35);

protected:
  virtual rtabmap::SensorData
  captureImage(rtabmap::SensorCaptureInfo *info = 0);

private:
  rs2::context ctx_;
  std::vector<rs2::device> dev_;
  std::string deviceId_;
  rs2::syncer syncer_;
  float depth_scale_meters_;
  cv::Mat depthBuffer_;
  cv::Mat irBuffer_;
  cv::Mat rgbBuffer_;
  cv::Mat prevColor_;
  rtabmap::CameraModel ir_depth_model_;
  rtabmap::CameraModel rgb_model_;
  rtabmap::Transform imuLocalTransform_;
  std::map<double, cv::Vec3f> accBuffer_;
  std::map<double, cv::Vec3f> gyroBuffer_;
  UMutex poseMutex_;
  UMutex imuMutex_;
  double lastImuStamp_;
  bool clockSyncWarningShown_;
  bool imuGlobalSyncWarningShown_;

  int colorWidth_;
  int colorHeight_;
  int colorFps_;
  int irDepthWidth_;
  int irDepthHeight_;
  int irDepthFps_;
  bool globalTimeSync_;
  std::string jsonConfig_;
  bool closing_;
};
