cmake_minimum_required(VERSION 3.21)
project(media_libtorrent_bridge LANGUAGES CXX)

find_package(LibtorrentRasterbar CONFIG REQUIRED)

add_library(media_libtorrent_bridge SHARED src/bridge.cpp)
target_compile_features(media_libtorrent_bridge PRIVATE cxx_std_17)
target_include_directories(media_libtorrent_bridge PUBLIC include)
target_link_libraries(media_libtorrent_bridge PRIVATE LibtorrentRasterbar::torrent-rasterbar)
target_compile_definitions(media_libtorrent_bridge PRIVATE MT_BUILDING_LIBRARY)

if(MSVC)
  target_compile_options(media_libtorrent_bridge PRIVATE /W4 /EHsc /utf-8)
endif()

include(CTest)
if(BUILD_TESTING)
  add_executable(media_libtorrent_smoke tests/smoke.cpp)
  target_link_libraries(media_libtorrent_smoke PRIVATE media_libtorrent_bridge)
  add_test(NAME media_libtorrent_smoke COMMAND media_libtorrent_smoke
    "${CMAKE_CURRENT_BINARY_DIR}/smoke-state"
    "${CMAKE_CURRENT_BINARY_DIR}/smoke-downloads")
endif()
