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

add_executable(vtk_cube main.cpp)
target_link_libraries(vtk_cube PRIVATE ${VTK_LIBRARIES})

if(VTK_VERSION VERSION_GREATER_EQUAL "8.90.0")
  vtk_module_autoinit(
    TARGETS vtk_cube
    MODULES ${VTK_LIBRARIES}
  )
endif()
