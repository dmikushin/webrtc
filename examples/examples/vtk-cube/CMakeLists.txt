# Minimal VTK Cube Example
cmake_minimum_required(VERSION 3.12)
project(VTKCubeExample)

find_package(VTK REQUIRED COMPONENTS
  CommonCore
  FiltersSources
  RenderingCore
  RenderingOpenGL2
  InteractionStyle
  RenderingFreeType
)

# Add custom cmake module path for FindVPX.cmake
list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
find_package(VPX REQUIRED)

# Find the WebRTC shared library and header
find_library(WEBRTC_LIB NAMES webrtc PATHS ${CMAKE_SOURCE_DIR}/../../../target/release REQUIRED)
find_path(WEBRTC_INCLUDE_DIR NAMES webrtc_c_api.h PATHS . REQUIRED)

find_package(PkgConfig REQUIRED)
pkg_check_modules(IXWEBSOCKET REQUIRED ixwebsocket)

add_executable(vtk_cube main.cpp)
target_include_directories(vtk_cube PRIVATE ${WEBRTC_INCLUDE_DIR} ${IXWEBSOCKET_INCLUDE_DIRS})
target_link_libraries(vtk_cube PRIVATE ${VTK_LIBRARIES} ${WEBRTC_LIB} ${IXWEBSOCKET_LIBRARIES} pthread vpx)

if(VTK_VERSION VERSION_GREATER_EQUAL "8.90.0")
  vtk_module_autoinit(
    TARGETS vtk_cube
    MODULES ${VTK_LIBRARIES}
  )
endif()
