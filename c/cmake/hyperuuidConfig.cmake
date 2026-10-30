# find_package(hyperuuid) for an unpacked release archive; ships as
# lib/cmake/hyperuuid/hyperuuidConfig.cmake. Every path is relative to this file, so the
# archive works wherever it is unpacked, and a target is declared only for a library the
# archive carries (the WebAssembly archive has no shared library).
#
#   hyperuuid::hyperuuid          the shared library
#   hyperuuid::hyperuuid_static   the static library, which needs nothing past the C runtime
#
# The version is the package's, from hyperuuidConfigVersion.cmake; at run time the library
# reports its own through hyperuuid_version().

get_filename_component(_hyperuuid_prefix "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)

if(WIN32)
  set(_hyperuuid_shared "${_hyperuuid_prefix}/bin/hyperuuid.dll")
  set(_hyperuuid_implib "${_hyperuuid_prefix}/lib/hyperuuid.dll.lib")
  set(_hyperuuid_static "${_hyperuuid_prefix}/lib/hyperuuid.lib")
elseif(APPLE)
  set(_hyperuuid_shared "${_hyperuuid_prefix}/lib/libhyperuuid.dylib")
  set(_hyperuuid_static "${_hyperuuid_prefix}/lib/libhyperuuid.a")
else()
  set(_hyperuuid_shared "${_hyperuuid_prefix}/lib/libhyperuuid.so")
  set(_hyperuuid_static "${_hyperuuid_prefix}/lib/libhyperuuid.a")
endif()

if(NOT TARGET hyperuuid::hyperuuid AND EXISTS "${_hyperuuid_shared}")
  add_library(hyperuuid::hyperuuid SHARED IMPORTED)
  set_target_properties(hyperuuid::hyperuuid PROPERTIES
    IMPORTED_LOCATION "${_hyperuuid_shared}"
    INTERFACE_INCLUDE_DIRECTORIES "${_hyperuuid_prefix}/include")
  if(WIN32)
    set_target_properties(hyperuuid::hyperuuid PROPERTIES IMPORTED_IMPLIB "${_hyperuuid_implib}")
  elseif(APPLE)
    set_target_properties(hyperuuid::hyperuuid PROPERTIES IMPORTED_SONAME "@rpath/libhyperuuid.dylib")
  else()
    set_target_properties(hyperuuid::hyperuuid PROPERTIES IMPORTED_SONAME "libhyperuuid.so")
  endif()
endif()

if(NOT TARGET hyperuuid::hyperuuid_static AND EXISTS "${_hyperuuid_static}")
  add_library(hyperuuid::hyperuuid_static STATIC IMPORTED)
  set_target_properties(hyperuuid::hyperuuid_static PROPERTIES
    IMPORTED_LOCATION "${_hyperuuid_static}"
    IMPORTED_LINK_INTERFACE_LANGUAGES C
    INTERFACE_INCLUDE_DIRECTORIES "${_hyperuuid_prefix}/include")
endif()

unset(_hyperuuid_prefix)
unset(_hyperuuid_shared)
unset(_hyperuuid_implib)
unset(_hyperuuid_static)
