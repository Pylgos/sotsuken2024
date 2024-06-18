cmake_minimum_required(VERSION 3.10)
project("slam_core" LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)

find_package(RTABMap)

add_library(${PROJECT_NAME}
  STATIC
    CameraRs2D4xx.cpp
    slam_core.cpp
)

target_link_libraries(${PROJECT_NAME}
  PRIVATE
    RTABMap
)

target_include_directories(${PROJECT_NAME}
  PUBLIC
    ${RTABMap_INCLUDE_DIRS}
)

install (TARGETS ${PROJECT_NAME} DESTINATION .)

