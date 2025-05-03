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

# Find the WebRTC shared library and header
find_library(WEBRTC_LIB NAMES webrtc PATHS ${CMAKE_SOURCE_DIR}/../../../target/release REQUIRED)
find_path(WEBRTC_INCLUDE_DIR NAMES webrtc_c_api.h PATHS . REQUIRED)

add_executable(vtk_cube main.cpp)
target_include_directories(vtk_cube PRIVATE ${WEBRTC_INCLUDE_DIR})
target_link_libraries(vtk_cube PRIVATE ${VTK_LIBRARIES} ${WEBRTC_LIB})

if(VTK_VERSION VERSION_GREATER_EQUAL "8.90.0")
  vtk_module_autoinit(
    TARGETS vtk_cube
    MODULES ${VTK_LIBRARIES}
  )
endif()
