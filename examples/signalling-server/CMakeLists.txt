# Minimal C++ WebSocket Signaling Server using uWebSockets
cmake_minimum_required(VERSION 3.12)
project(signalling-server)

find_package(PkgConfig REQUIRED)
pkg_check_modules(UWS REQUIRED uwebsockets)

add_executable(signalling-server signalling_server.cpp)
target_include_directories(signalling-server PRIVATE ${UWS_INCLUDE_DIRS})
target_link_libraries(signalling-server PRIVATE ${UWS_LIBRARIES} usockets ssl crypto z pthread)
