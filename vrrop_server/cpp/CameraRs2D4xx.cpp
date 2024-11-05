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
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

#include "CameraRs2D4xx.h"
#include "librealsense2/h/rs_sensor.h"
#include "librealsense2/hpp/rs_frame.hpp"
#include "librealsense2/hpp/rs_sensor.hpp"
#include <opencv2/imgproc/types_c.h>
#include <rtabmap/core/util2d.h>
#include <rtabmap/utilite/UConversion.h>
#include <rtabmap/utilite/UStl.h>
#include <rtabmap/utilite/UThreadC.h>
#include <rtabmap/utilite/UTimer.h>

#include <fstream>
#include <librealsense2/hpp/rs_processing.hpp>
#include <librealsense2/rs_advanced_mode.hpp>
#include <librealsense2/rsutil.h>

using namespace rtabmap;

// static std::optional<rs2::video_stream_profile> find_profile(rs2::sensor
// sensor, int width, int height, std::optional<int> index, int format) {
//   for (auto&& prof : sensor.get_stream_profiles()) {
//     if (prof.)
//   }
// }

bool CameraRs2D4xx::available() { return true; }

CameraRs2D4xx::CameraRs2D4xx(const std::string &device, float imageRate,
                             const rtabmap::Transform &localTransform)
    : Camera(imageRate, localTransform), deviceId_(device),
      depth_scale_meters_(1.0f), lastImuStamp_(0.0),
      clockSyncWarningShown_(false), imuGlobalSyncWarningShown_(false),
      colorWidth_(640), colorHeight_(480), colorFps_(30), irDepthWidth_(640),
      irDepthHeight_(480), irDepthFps_(30), globalTimeSync_(true),
      closing_(false) {
  UDEBUG("");
}

CameraRs2D4xx::~CameraRs2D4xx() { close(); }

void CameraRs2D4xx::close() {
  closing_ = true;
  try {
    UDEBUG("Closing device(s)...");
    for (size_t i = 0; i < dev_.size(); ++i) {
      UDEBUG("Closing %d sensor(s) from device %d...",
             (int)dev_[i].query_sensors().size(), (int)i);
      for (rs2::sensor _sensor : dev_[i].query_sensors()) {
        if (!_sensor.get_active_streams().empty()) {
          try {
            _sensor.stop();
            _sensor.close();
          } catch (const rs2::error &error) {
            UWARN("%s", error.what());
          }
        }
      }
#ifdef WIN32
      dev_[i].hardware_reset(); // To avoid freezing on some Windows computers
                                // in the following destructor
      // Don't do this on linux (tested on Ubuntu 18.04, realsense v2.41.0):
      // T265 cannot be restarted
#endif
    }
    UDEBUG("Clearing devices...");
    dev_.clear();
  } catch (const rs2::error &error) {
    UINFO("%s", error.what());
  }

  closing_ = false;
}

void CameraRs2D4xx::imu_callback(rs2::frame frame) {
  auto stream = frame.get_profile().stream_type();
  cv::Vec3f crnt_reading =
      *reinterpret_cast<const cv::Vec3f *>(frame.get_data());
  // UDEBUG("%s callback! %f (%f %f %f)",
  // 		stream == RS2_STREAM_GYRO?"GYRO":"ACC",
  // 		frame.get_timestamp(),
  // 		crnt_reading[0],
  // 		crnt_reading[1],
  // 		crnt_reading[2]);
  UScopeMutex sm(imuMutex_);
  if (stream == RS2_STREAM_GYRO) {
    gyroBuffer_.insert(gyroBuffer_.end(),
                       std::make_pair(frame.get_timestamp(), crnt_reading));
    if (gyroBuffer_.size() > 1000) {
      gyroBuffer_.erase(gyroBuffer_.begin());
    }
  } else {
    accBuffer_.insert(accBuffer_.end(),
                      std::make_pair(frame.get_timestamp(), crnt_reading));
    if (accBuffer_.size() > 1000) {
      accBuffer_.erase(accBuffer_.begin());
    }
  }
}

void CameraRs2D4xx::frame_callback(rs2::frame frame) {
  // UDEBUG("Frame callback! %f", frame.get_timestamp());
  syncer_(frame);
}

void CameraRs2D4xx::multiple_message_callback(rs2::frame frame) {
  if (closing_) {
    return;
  }
  auto stream = frame.get_profile().stream_type();
  switch (stream) {
  case RS2_STREAM_GYRO:
  case RS2_STREAM_ACCEL:
    // UWARN("IMU : Domain=%d time=%f host=%f",
    // frame.get_frame_timestamp_domain(), frame.get_timestamp()/1000.0,
    // UTimer::now());
    imu_callback(frame);
    break;
  case RS2_STREAM_POSE:
    // UWARN("POSE : Domain=%d time=%f host=%f",
    // frame.get_frame_timestamp_domain(), frame.get_timestamp()/1000.0,
    // UTimer::now());
    break;
  default:
    // UWARN("IMG : Domain=%d time=%f host=%f",
    // frame.get_frame_timestamp_domain(), frame.get_timestamp()/1000.0,
    // UTimer::now());
    frame_callback(frame);
  }
}

void CameraRs2D4xx::getIMU(const double &stamp, IMU &imu, int maxWaitTimeMs) {
  imu = IMU();

  if (accBuffer_.empty() || gyroBuffer_.empty()) {
    return;
  }

  // Interpolate acc
  cv::Vec3d acc;
  {
    imuMutex_.lock();
    if (globalTimeSync_) {
      int waitTry = 0;
      while (maxWaitTimeMs > 0 && accBuffer_.rbegin()->first < stamp &&
             waitTry < maxWaitTimeMs) {
        imuMutex_.unlock();
        ++waitTry;
        uSleep(1);
        imuMutex_.lock();
      }
    }
    if (globalTimeSync_ && accBuffer_.rbegin()->first < stamp) {
      if (maxWaitTimeMs > 0) {
        UWARN("Could not find acc data to interpolate at image time %f after "
              "waiting %d ms (last is %f)...",
              stamp / 1000.0, maxWaitTimeMs,
              accBuffer_.rbegin()->first / 1000.0);
      }
      imuMutex_.unlock();
      return;
    } else {
      std::map<double, cv::Vec3f>::const_iterator iterB =
          accBuffer_.lower_bound(stamp);
      std::map<double, cv::Vec3f>::const_iterator iterA = iterB;
      if (iterA != accBuffer_.begin()) {
        iterA = --iterA;
      }
      if (iterB == accBuffer_.end()) {
        iterB = --iterB;
      }
      if (iterA == iterB && stamp == iterA->first) {
        acc[0] = iterA->second[0];
        acc[1] = iterA->second[1];
        acc[2] = iterA->second[2];
      } else if (stamp >= iterA->first && stamp <= iterB->first) {
        float t = (stamp - iterA->first) / (iterB->first - iterA->first);
        acc[0] = iterA->second[0] + t * (iterB->second[0] - iterA->second[0]);
        acc[1] = iterA->second[1] + t * (iterB->second[1] - iterA->second[1]);
        acc[2] = iterA->second[2] + t * (iterB->second[2] - iterA->second[2]);
      } else {
        if (!imuGlobalSyncWarningShown_) {
          if (stamp < iterA->first) {
            UWARN("Could not find acc data to interpolate at image time %f "
                  "(earliest is %f). Are sensors synchronized?",
                  stamp / 1000.0, iterA->first / 1000.0);
          } else {
            UWARN("Could not find acc data to interpolate at image time %f "
                  "(between %f and %f). Are sensors synchronized?",
                  stamp / 1000.0, iterA->first / 1000.0, iterB->first / 1000.0);
          }
        }
        if (!globalTimeSync_) {
          if (!imuGlobalSyncWarningShown_) {
            UWARN("As globalTimeSync option is off, the received gyro and "
                  "accelerometer will be re-stamped with image time. This "
                  "message is only shown once.");
            imuGlobalSyncWarningShown_ = true;
          }
          std::map<double, cv::Vec3f>::const_reverse_iterator iterC =
              accBuffer_.rbegin();
          acc[0] = iterC->second[0];
          acc[1] = iterC->second[1];
          acc[2] = iterC->second[2];
        } else {
          imuMutex_.unlock();
          return;
        }
      }
    }
    imuMutex_.unlock();
  }

  // Interpolate gyro
  cv::Vec3d gyro;
  {
    imuMutex_.lock();
    if (globalTimeSync_) {
      int waitTry = 0;
      while (maxWaitTimeMs > 0 && gyroBuffer_.rbegin()->first < stamp &&
             waitTry < maxWaitTimeMs) {
        imuMutex_.unlock();
        ++waitTry;
        uSleep(1);
        imuMutex_.lock();
      }
    }
    if (globalTimeSync_ && gyroBuffer_.rbegin()->first < stamp) {
      if (maxWaitTimeMs > 0) {
        UWARN("Could not find gyro data to interpolate at image time %f after "
              "waiting %d ms (last is %f)...",
              stamp / 1000.0, maxWaitTimeMs,
              gyroBuffer_.rbegin()->first / 1000.0);
      }
      imuMutex_.unlock();
      return;
    } else {
      std::map<double, cv::Vec3f>::const_iterator iterB =
          gyroBuffer_.lower_bound(stamp);
      std::map<double, cv::Vec3f>::const_iterator iterA = iterB;
      if (iterA != gyroBuffer_.begin()) {
        iterA = --iterA;
      }
      if (iterB == gyroBuffer_.end()) {
        iterB = --iterB;
      }
      if (iterA == iterB && stamp == iterA->first) {
        gyro[0] = iterA->second[0];
        gyro[1] = iterA->second[1];
        gyro[2] = iterA->second[2];
      } else if (stamp >= iterA->first && stamp <= iterB->first) {
        float t = (stamp - iterA->first) / (iterB->first - iterA->first);
        gyro[0] = iterA->second[0] + t * (iterB->second[0] - iterA->second[0]);
        gyro[1] = iterA->second[1] + t * (iterB->second[1] - iterA->second[1]);
        gyro[2] = iterA->second[2] + t * (iterB->second[2] - iterA->second[2]);
      } else {
        if (!imuGlobalSyncWarningShown_) {
          if (stamp < iterA->first) {
            UWARN("Could not find gyro data to interpolate at image time %f "
                  "(earliest is %f). Are sensors synchronized?",
                  stamp / 1000.0, iterA->first / 1000.0);
          } else {
            UWARN("Could not find gyro data to interpolate at image time %f "
                  "(between %f and %f). Are sensors synchronized?",
                  stamp / 1000.0, iterA->first / 1000.0, iterB->first / 1000.0);
          }
        }
        if (!globalTimeSync_) {
          if (!imuGlobalSyncWarningShown_) {
            UWARN("As globalTimeSync option is off, the latest received gyro "
                  "and accelerometer will be re-stamped with image time. This "
                  "message is only shown once.");
            imuGlobalSyncWarningShown_ = true;
          }
          std::map<double, cv::Vec3f>::const_reverse_iterator iterC =
              gyroBuffer_.rbegin();
          gyro[0] = iterC->second[0];
          gyro[1] = iterC->second[1];
          gyro[2] = iterC->second[2];
        } else {
          imuMutex_.unlock();
          return;
        }
      }
    }
    imuMutex_.unlock();
  }

  imu = IMU(gyro, cv::Mat::eye(3, 3, CV_64FC1), acc,
            cv::Mat::eye(3, 3, CV_64FC1), imuLocalTransform_);
}

bool CameraRs2D4xx::init(const std::string &calibrationFolder,
                         const std::string &cameraName) {
  UDEBUG("");

  UINFO("setupDevice...");

  close();

  clockSyncWarningShown_ = false;
  imuGlobalSyncWarningShown_ = false;

  rs2::device_list list = ctx_.query_devices();
  if (0 == list.size()) {
    UERROR("No RealSense2 devices were found!");
    return false;
  }

  bool found = false;
  try {
    for (rs2::device dev : list) {
      auto sn = dev.get_info(RS2_CAMERA_INFO_SERIAL_NUMBER);
      auto pid_str = dev.get_info(RS2_CAMERA_INFO_PRODUCT_ID);
      auto name = dev.get_info(RS2_CAMERA_INFO_NAME);

      uint16_t pid;
      std::stringstream ss;
      ss << std::hex << pid_str;
      ss >> pid;
      UINFO("Device \"%s\" with serial number %s was found with product ID=%d.",
            name, sn, (int)pid);
      if (!found && (deviceId_.empty() || deviceId_ == sn ||
                     uStrContains(name, uToUpperCase(deviceId_)))) {
        if (dev_.empty()) {
          dev_.resize(1);
        }
        dev_[0] = dev;
        found = true;
      }
    }
  } catch (const rs2::error &error) {
    UWARN("%s. Is the camera already used with another app?", error.what());
  }

  if (!found) {
    UERROR("The requested device \"%s\" is NOT found!", deviceId_.c_str());
    return false;
  }

  UASSERT(!dev_.empty());

  if (!jsonConfig_.empty()) {
    if (dev_[0].is<rs400::advanced_mode>()) {
      std::stringstream ss;
      std::ifstream in(jsonConfig_);
      if (in.is_open()) {
        ss << in.rdbuf();
        std::string json_file_content = ss.str();

        auto adv = dev_[0].as<rs400::advanced_mode>();
        adv.load_json(json_file_content);
        UINFO("JSON file is loaded! (%s)", jsonConfig_.c_str());
      } else {
        UWARN("JSON file provided doesn't exist! (%s)", jsonConfig_.c_str());
      }
    } else {
      UWARN("A json config file is provided (%s), but device does not support "
            "advanced settings!",
            jsonConfig_.c_str());
    }
  }

  ctx_.set_devices_changed_callback([this](rs2::event_information &info) {
    for (size_t i = 0; i < dev_.size(); ++i) {
      if (info.was_removed(dev_[i])) {
        if (closing_) {
          UDEBUG("The device %d has been disconnected!", i);
        } else {
          UERROR("The device %d has been disconnected!", i);
        }
      }
    }
  });

  auto sn = dev_[0].get_info(RS2_CAMERA_INFO_SERIAL_NUMBER);
  UINFO("Using device with Serial No: %s", sn);

  auto camera_name = dev_[0].get_info(RS2_CAMERA_INFO_NAME);
  UINFO("Device Name: %s", camera_name);

  auto fw_ver = dev_[0].get_info(RS2_CAMERA_INFO_FIRMWARE_VERSION);
  UINFO("Device FW version: %s", fw_ver);

  auto pid = dev_[0].get_info(RS2_CAMERA_INFO_PRODUCT_ID);
  UINFO("Device Product ID: 0x%s", pid);

  auto dev_sensors = dev_[0].query_sensors();

  UINFO("Device Sensors: ");
  rs2::sensor color_sensor;
  rs2::sensor depth_stereo_sensor;
  rs2::sensor motion_sensor;
  for (auto &&elem : dev_sensors) {
    std::string module_name = elem.get_info(RS2_CAMERA_INFO_NAME);
    if (elem.is<rs2::color_sensor>()) {
      color_sensor = elem;
    } else if (elem.is<rs2::depth_stereo_sensor>()) {
      depth_stereo_sensor = elem;
    } else if (elem.is<rs2::motion_sensor>()) {
      motion_sensor = elem;
    }
    UINFO("%s was found.", elem.get_info(RS2_CAMERA_INFO_NAME));
  }

  depth_stereo_sensor.set_option(rs2_option::RS2_OPTION_EMITTER_ENABLED, false);
  motion_sensor.set_option(rs2_option::RS2_OPTION_ENABLE_MOTION_CORRECTION,
                           true);
  std::vector<rs2::sensor> sensors = {color_sensor, depth_stereo_sensor,
                                      motion_sensor};

  UDEBUG("");

  ir_depth_model_ = CameraModel();
  std::vector<std::vector<rs2::stream_profile>> profilesPerSensor(
      sensors.size());
  for (unsigned int i = 0; i < sensors.size(); ++i) {
    UINFO("Sensor %d \"%s\"", (int)i,
          sensors[i].get_info(RS2_CAMERA_INFO_NAME));
    auto profiles = sensors[i].get_stream_profiles();
    bool added = false;
    UINFO("profiles=%d", (int)profiles.size());
    if (ULogger::level() < ULogger::kWarning) {
      for (auto &profile : profiles) {
        auto video_profile = profile.as<rs2::video_stream_profile>();
        UINFO("%s %d %d %d %d %s type=%d",
              rs2_format_to_string(video_profile.format()),
              video_profile.width(), video_profile.height(),
              video_profile.fps(), video_profile.stream_index(),
              video_profile.stream_name().c_str(), video_profile.stream_type());
      }
    }
    for (auto &profile : profiles) {
      auto video_profile = profile.as<rs2::video_stream_profile>();
      bool is_rgb = video_profile.format() == RS2_FORMAT_RGB8 &&
                    video_profile.stream_type() == RS2_STREAM_COLOR;
      bool is_ir_left = video_profile.format() == RS2_FORMAT_Y8 &&
                        (video_profile.stream_index() == 1);
      bool is_depth = video_profile.format() == RS2_FORMAT_Z16;

      if (is_rgb && video_profile.width() == colorWidth_ &&
          video_profile.height() == colorHeight_ &&
          video_profile.fps() == colorFps_) {
        auto intrinsic = video_profile.get_intrinsics();
        profilesPerSensor[i].push_back(profile);
        rgb_model_ =
            CameraModel(camera_name, intrinsic.fx, intrinsic.fy, intrinsic.ppx,
                        intrinsic.ppy, this->getLocalTransform(), 0,
                        cv::Size(intrinsic.width, intrinsic.height));
        UINFO("Model: %dx%d fx=%f fy=%f cx=%f cy=%f dist model=%d coeff=%f "
              "%f %f %f %f",
              intrinsic.width, intrinsic.height, intrinsic.fx, intrinsic.fy,
              intrinsic.ppx, intrinsic.ppy, intrinsic.model,
              intrinsic.coeffs[0], intrinsic.coeffs[1], intrinsic.coeffs[2],
              intrinsic.coeffs[3], intrinsic.coeffs[4]);
        added = true;
        break;
      } else if (is_ir_left && video_profile.width() == irDepthWidth_ &&
                 video_profile.height() == irDepthHeight_ &&
                 video_profile.fps() == irDepthFps_) {
        auto intrinsic = video_profile.get_intrinsics();
        profilesPerSensor[i].push_back(profile);
        ir_depth_model_ =
            CameraModel(camera_name, intrinsic.fx, intrinsic.fy, intrinsic.ppx,
                        intrinsic.ppy, this->getLocalTransform(), 0,
                        cv::Size(intrinsic.width, intrinsic.height));
        UINFO("Model: %dx%d fx=%f fy=%f cx=%f cy=%f dist model=%d coeff=%f "
              "%f %f %f %f",
              intrinsic.width, intrinsic.height, intrinsic.fx, intrinsic.fy,
              intrinsic.ppx, intrinsic.ppy, intrinsic.model,
              intrinsic.coeffs[0], intrinsic.coeffs[1], intrinsic.coeffs[2],
              intrinsic.coeffs[3], intrinsic.coeffs[4]);
        added = true;
        if (profilesPerSensor[i].size() == 2) {
          break;
        }
      } else if (is_depth && video_profile.width() == irDepthWidth_ &&
                 video_profile.height() == irDepthHeight_ &&
                 video_profile.fps() == irDepthFps_) {
        profilesPerSensor[i].push_back(profile);
        added = true;
        break;
      } else if (profile.format() == RS2_FORMAT_MOTION_XYZ32F) {
        // D435i:
        // MOTION_XYZ32F 0 0 200 (gyro)
        // MOTION_XYZ32F 0 0 400 (gyro)
        // MOTION_XYZ32F 0 0 63 6 (accel)
        // MOTION_XYZ32F 0 0 250 6 (accel)
        bool modified = false;
        for (size_t j = 0; j < profilesPerSensor[i].size(); ++j) {
          if (profilesPerSensor[i][j].stream_type() == profile.stream_type()) {
            if (profile.stream_type() == RS2_STREAM_ACCEL) {
              if (profile.fps() > profilesPerSensor[i][j].fps())
                profilesPerSensor[i][j] = profile;
              modified = true;
            } else if (profile.stream_type() == RS2_STREAM_GYRO) {
              if (profile.fps() < profilesPerSensor[i][j].fps())
                profilesPerSensor[i][j] = profile;
              modified = true;
            }
          }
        }
        if (!modified)
          profilesPerSensor[i].push_back(profile);
        added = true;
      }
    }
    if (!added) {
      UERROR("Given stream configuration is not supported by the device! "
             "Stream Index: %d, Width: %d, Height: %d, FPS: %d",
             i, irDepthWidth_, irDepthHeight_, irDepthFps_);
      UERROR("Available configurations:");
      for (auto &profile : profiles) {
        auto video_profile = profile.as<rs2::video_stream_profile>();
        UERROR("%s %d %d %d %d %s type=%d",
               rs2_format_to_string(video_profile.format()),
               video_profile.width(), video_profile.height(),
               video_profile.fps(), video_profile.stream_index(),
               video_profile.stream_name().c_str(),
               video_profile.stream_type());
      }
      return false;
    }
  }
  rgbBuffer_ = cv::Mat(cv::Size(colorWidth_, colorHeight_), CV_8UC3,
                       cv::Scalar(0, 0, 0));
  irBuffer_ =
      cv::Mat(cv::Size(irDepthWidth_, irDepthHeight_), CV_8UC1, cv::Scalar(0));
  depthBuffer_ =
      cv::Mat(cv::Size(irDepthWidth_, irDepthHeight_), CV_16UC1, cv::Scalar(0));
  UDEBUG("");
  if (!ir_depth_model_.isValidForProjection()) {
    UERROR("Calibration info not valid!");
    std::cout << ir_depth_model_ << std::endl;
    return false;
  }

  if (profilesPerSensor.size() == 3) {
    if (!profilesPerSensor[2].empty() && !profilesPerSensor[0].empty()) {
      rs2_extrinsics leftToIMU =
          profilesPerSensor[2][0].get_extrinsics_to(profilesPerSensor[0][0]);
      Transform leftToIMUT(leftToIMU.rotation[0], leftToIMU.rotation[1],
                           leftToIMU.rotation[2], leftToIMU.translation[0],
                           leftToIMU.rotation[3], leftToIMU.rotation[4],
                           leftToIMU.rotation[5], leftToIMU.translation[1],
                           leftToIMU.rotation[6], leftToIMU.rotation[7],
                           leftToIMU.rotation[8], leftToIMU.translation[2]);
      imuLocalTransform_ = this->getLocalTransform() * leftToIMUT;
      UINFO("imu local transform = %s",
            imuLocalTransform_.prettyPrint().c_str());
    } else if (!profilesPerSensor[2].empty() && !profilesPerSensor[1].empty()) {
      rs2_extrinsics leftToIMU =
          profilesPerSensor[2][0].get_extrinsics_to(profilesPerSensor[1][0]);
      Transform leftToIMUT(leftToIMU.rotation[0], leftToIMU.rotation[1],
                           leftToIMU.rotation[2], leftToIMU.translation[0],
                           leftToIMU.rotation[3], leftToIMU.rotation[4],
                           leftToIMU.rotation[5], leftToIMU.translation[1],
                           leftToIMU.rotation[6], leftToIMU.rotation[7],
                           leftToIMU.rotation[8], leftToIMU.translation[2]);

      imuLocalTransform_ = this->getLocalTransform() * leftToIMUT;
      UINFO("imu local transform = %s",
            imuLocalTransform_.prettyPrint().c_str());
    }
  }

  std::function<void(rs2::frame)> multiple_message_callback_function =
      [this](rs2::frame frame) { multiple_message_callback(frame); };

  for (unsigned int i = 0; i < sensors.size(); ++i) {
    if (profilesPerSensor[i].size()) {
      UINFO("Starting sensor %d with %d profiles", (int)i,
            (int)profilesPerSensor[i].size());
      for (size_t j = 0; j < profilesPerSensor[i].size(); ++j) {
        auto video_profile =
            profilesPerSensor[i][j].as<rs2::video_stream_profile>();
        UINFO("Opening: %s %d %d %d %d %s type=%d",
              rs2_format_to_string(video_profile.format()),
              video_profile.width(), video_profile.height(),
              video_profile.fps(), video_profile.stream_index(),
              video_profile.stream_name().c_str(), video_profile.stream_type());
      }
      if (globalTimeSync_ &&
          sensors[i].supports(rs2_option::RS2_OPTION_GLOBAL_TIME_ENABLED)) {
        float value =
            sensors[i].get_option(rs2_option::RS2_OPTION_GLOBAL_TIME_ENABLED);
        UINFO("Set RS2_OPTION_GLOBAL_TIME_ENABLED=1 (was %f) for sensor %d",
              value, (int)i);
        sensors[i].set_option(rs2_option::RS2_OPTION_GLOBAL_TIME_ENABLED, 1);
      }
      sensors[i].open(profilesPerSensor[i]);
      if (sensors[i].is<rs2::depth_sensor>()) {
        auto depth_sensor = sensors[i].as<rs2::depth_sensor>();
        depth_scale_meters_ = depth_sensor.get_depth_scale();
        UINFO("Depth scale %f for sensor %d", depth_scale_meters_, (int)i);
      }
      sensors[i].start(multiple_message_callback_function);
    }
  }

  uSleep(1000); // ignore the first frames
  UINFO("Enabling streams...done!");

  return true;
}

bool CameraRs2D4xx::isCalibrated() const {
  return ir_depth_model_.isValidForProjection();
}

std::string CameraRs2D4xx::getSerial() const {
  if (!dev_.empty()) {
    return dev_[0].get_info(RS2_CAMERA_INFO_SERIAL_NUMBER);
  }
  return "NA";
}

bool CameraRs2D4xx::odomProvided() const { return false; }

bool CameraRs2D4xx::getPose(double stamp, Transform &pose, cv::Mat &covariance,
                            double maxWaitTime) {
  return false;
  // IMU imu;
  // unsigned int confidence = 0;
  // double rsStamp = stamp * 1000.0;
  // Transform p;
  // getPoseAndIMU(rsStamp, p, confidence, imu, maxWaitTime * 1000);

  // if (!p.isNull()) {
  //   // Transform in base frame
  //   pose = this->getLocalTransform() * p *
  //   this->getLocalTransform().inverse();

  //   covariance = cv::Mat::eye(6, 6, CV_64FC1) * 0.0001;
  //   covariance.rowRange(0, 3) *= pow(10, 3 - (int)confidence);
  //   covariance.rowRange(3, 6) *= pow(10, 1 - (int)confidence);
  //   return true;
  // }
  // return false;
}

void CameraRs2D4xx::setColorResolution(int width, int height, int fps) {
  colorWidth_ = width;
  colorHeight_ = height;
  colorFps_ = fps;
}

void CameraRs2D4xx::setIrDepthResolution(int width, int height, int fps) {
  irDepthWidth_ = width;
  irDepthHeight_ = height;
  irDepthFps_ = fps;
}

void CameraRs2D4xx::setGlobalTimeSync(bool enabled) {
  globalTimeSync_ = enabled;
}

void CameraRs2D4xx::setJsonConfig(const std::string &json) {
  jsonConfig_ = json;
}

SensorData CameraRs2D4xx::captureImage(SensorCaptureInfo *info) {
  SensorData data;

  try {
    UTimer timer;
    rs2::frameset frameset;
    rs2::frame color_frame, ir_frame, depth_frame;
    bool required_frames_arrived = false;
    do {
      frameset = syncer_.wait_for_frames(100);
      color_frame = color_frame.get() == nullptr ? frameset.get_color_frame()
                                                 : color_frame;
      ir_frame =
          ir_frame.get() == nullptr ? frameset.get_infrared_frame() : ir_frame;
      depth_frame = depth_frame.get() == nullptr ? frameset.get_depth_frame()
                                                 : depth_frame;
      required_frames_arrived =
          ir_frame.get() != nullptr && depth_frame.get() != nullptr;
      // color_frame.get() != nullptr;
    } while (!required_frames_arrived && timer.elapsed() < 2.0);

    if (required_frames_arrived) {
      double now = UTimer::now();
      double stamp = frameset.get_timestamp();

      stamp /= 1000.0; // put in seconds
      UDEBUG("Frameset arrived. system=%fs frame=%fs", now, stamp);
      if (stamp - now > 1000000000.0) {
        if (!clockSyncWarningShown_) {
          UWARN(
              "Clocks are not sync with host computer! Detected stamps in far "
              "future %f, thus using host time instead (%f)! This message "
              "will only appear once. "
              "See https://github.com/IntelRealSense/librealsense/issues/4505 "
              "for more info",
              stamp, now);
          clockSyncWarningShown_ = true;
        }
        stamp = now;
      }

      cv::Mat depth = cv::Mat(depthBuffer_.size(), depthBuffer_.type(),
                              (void *)depth_frame.get_data())
                          .clone();
      cv::Mat ir = cv::Mat(irBuffer_.size(), irBuffer_.type(),
                           (void *)ir_frame.get_data())
                       .clone();
      if (color_frame.get() != nullptr) {
        cv::Mat color = cv::Mat(rgbBuffer_.size(), rgbBuffer_.type(),
                                (void *)color_frame.get_data())
                            .clone();
        prevColor_ = color;
      }
      data = SensorData(ir, depth, ir_depth_model_, this->getNextSeqID(), stamp,
                        prevColor_);

      IMU imu;
      double imuStamp = stamp * 1000.0;
      getIMU(imuStamp, imu);

      if (!imu.empty() && !isInterIMUPublishing()) {
        data.setIMU(imu);
      } else if (isInterIMUPublishing() && !gyroBuffer_.empty()) {
        if (lastImuStamp_ > 0.0) {
          UASSERT(imuStamp > lastImuStamp_);
          imuMutex_.lock();
          std::map<double, cv::Vec3f>::iterator iterA =
              gyroBuffer_.upper_bound(lastImuStamp_);
          std::map<double, cv::Vec3f>::iterator iterB =
              gyroBuffer_.lower_bound(imuStamp);
          if (iterA != gyroBuffer_.end()) {
            ++iterA;
          }
          if (iterB != gyroBuffer_.end()) {
            ++iterB;
          }
          std::vector<double> stamps;
          for (; iterA != iterB; ++iterA) {
            stamps.push_back(iterA->first);
          }
          imuMutex_.unlock();

          int pub = 0;
          for (size_t i = 0; i < stamps.size(); ++i) {
            IMU imuTmp;
            getIMU(stamps[i], imuTmp);
            if (!imuTmp.empty()) {
              this->postInterIMU(imuTmp, stamps[i] / 1000.0);
              pub++;
            } else {
              break;
            }
          }
          if (stamps.size()) {
            UDEBUG("inter imu published=%d (rate=%fHz), %f -> %f", pub,
                   double(pub) / ((stamps.back() - stamps.front()) / 1000.0),
                   stamps.front() / 1000.0, stamps.back() / 1000.0);
          } else {
            UWARN("No inter imu published!?");
          }
        }
        lastImuStamp_ = imuStamp;
      }
    } else {
      UERROR("Missing frames");
    }
  } catch (const std::exception &ex) {
    UERROR("An error has occurred during frame callback: %s", ex.what());
  }
  return data;
}
