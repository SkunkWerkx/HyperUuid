# The smoke consumer, through find_package: run.sh points CMAKE_PREFIX_PATH at an unpacked
# release archive and builds it once against each library the archive has.
#
# Every program with warnings as errors (-Wall -Wextra -Wpedantic -Wshadow -Werror; /W4 /WX on
# MSVC), which reach the headers too: an imported target's includes are system headers, whose
# warnings a compiler keeps to itself, so NO_SYSTEM_FROM_IMPORTED takes that away.
cmake_minimum_required(VERSION 3.21)
project(hyperuuid_smoke C)

option(HYPERUUID_SMOKE_STATIC "Link hyperuuid::hyperuuid_static instead of hyperuuid::hyperuuid" OFF)

find_package(hyperuuid REQUIRED CONFIG)

if(MSVC)
  set(_warnings /W4 /WX)
else()
  set(_warnings -Wall -Wextra -Wpedantic -Wshadow -Werror)
endif()

# The C++ header, built wherever there is a C++ compiler whose library has C++23's
# <expected>; HYPERUUID_SMOKE_CXX=ON makes its absence an error.
option(HYPERUUID_SMOKE_CXX "Require the C++ smoke program" OFF)
include(CheckLanguage)
check_language(CXX)
set(_smoke_programs smoke)
if(CMAKE_CXX_COMPILER)
  enable_language(CXX)
  include(CheckCXXSourceCompiles)
  set(CMAKE_REQUIRED_FLAGS "${CMAKE_CXX23_STANDARD_COMPILE_OPTION}")
  check_cxx_source_compiles("#include <expected>\nint main() { std::expected<int, int> e = 1; return *e - 1; }"
    HYPERUUID_SMOKE_HAS_EXPECTED)
  unset(CMAKE_REQUIRED_FLAGS)
  if(HYPERUUID_SMOKE_HAS_EXPECTED)
    add_executable(smoke_cpp main.cpp)
    target_compile_features(smoke_cpp PRIVATE cxx_std_23)
    list(APPEND _smoke_programs smoke_cpp)
  endif()
endif()
if(HYPERUUID_SMOKE_CXX AND NOT TARGET smoke_cpp)
  message(FATAL_ERROR "HYPERUUID_SMOKE_CXX is on, but no C++ compiler here has C++23's <expected>")
endif()

add_executable(smoke main.c)
foreach(program IN LISTS _smoke_programs)
  target_compile_options(${program} PRIVATE ${_warnings})
  set_target_properties(${program} PROPERTIES NO_SYSTEM_FROM_IMPORTED ON)
  if(HYPERUUID_SMOKE_STATIC)
    target_link_libraries(${program} PRIVATE hyperuuid::hyperuuid_static)
  else()
    target_link_libraries(${program} PRIVATE hyperuuid::hyperuuid)
    # Windows has no rpath: the DLL goes beside the program.
    if(WIN32)
      add_custom_command(TARGET ${program} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_RUNTIME_DLLS:${program}> $<TARGET_FILE_DIR:${program}>
        COMMAND_EXPAND_LISTS)
    endif()
  endif()
  file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/${program}-path-$<CONFIG>.txt" CONTENT "$<TARGET_FILE:${program}>")
endforeach()

# What find_package resolved, for run.sh to hold against what the library reports. Where each
# program landed (inside an .app on iOS; under the configuration's folder in Visual Studio) is
# written above, as <program>-path-<config>.txt.
file(WRITE "${CMAKE_BINARY_DIR}/package-version.txt" "${hyperuuid_VERSION}")
