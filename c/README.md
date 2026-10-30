# hyperuuid for C and C++

RFC 9562 UUIDs from the HyperUuid core: v4, v5 and v7, v7 batches, and the SQL Server order.
It is the same library every HyperUuid binding calls, through the C ABI in
`include/hyperuuid.h`. C++23 has its own header over it, `include/hyperuuid.hpp`, linked
against the same library.

## Install

Each GitHub Release carries one archive per platform, `hyperuuid-<version>-<rid>.tar.gz`
(`.zip` on Windows), for linux-x64, linux-arm64, linux-musl-x64, linux-musl-arm64, osx-x64,
osx-arm64, win-x64, win-arm64, android-arm64, android-x64, ios-arm64, iossimulator-arm64,
maccatalyst-arm64, maccatalyst-x64 and wasi-wasm32. Verify one before use; the release
attestation lists every file the release shipped:

```sh
gh release download v<version> --repo SkunkWerkx/HyperUuid --pattern 'hyperuuid-*-linux-x64.tar.gz'
gh attestation verify hyperuuid-<version>-linux-x64.tar.gz --repo SkunkWerkx/HyperUuid
```

Unpack it anywhere:

```
include/hyperuuid.h, include/hyperuuid.hpp
lib/libhyperuuid.a                       static (Windows: lib/hyperuuid.lib)
lib/libhyperuuid.so | lib/libhyperuuid.dylib
                                         shared (Windows: bin/hyperuuid.dll and its
                                         import library lib/hyperuuid.dll.lib)
lib/pkgconfig/hyperuuid.pc
lib/cmake/hyperuuid/
```

The iOS, iOS simulator and Mac Catalyst archives have the static library only, linked into
the app, and so does wasi-wasm32, for wasi-sdk.

## Build against it

CMake, with the unpacked directory on `CMAKE_PREFIX_PATH`:

```cmake
find_package(hyperuuid 1.1 REQUIRED)
target_link_libraries(app PRIVATE hyperuuid::hyperuuid)          # shared
target_link_libraries(app PRIVATE hyperuuid::hyperuuid_static)   # or static
```

Or let CMake fetch the archive at configure time. Name your platform's archive and its
SHA-256, which the release's `inventory.json` lists for every file:

```cmake
include(FetchContent)
FetchContent_Declare(hyperuuid
  URL https://github.com/SkunkWerkx/HyperUuid/releases/download/v<version>/hyperuuid-<version>-linux-x64.tar.gz
  URL_HASH SHA256=<sha256>)
FetchContent_MakeAvailable(hyperuuid)
find_package(hyperuuid CONFIG REQUIRED PATHS "${hyperuuid_SOURCE_DIR}" NO_DEFAULT_PATH NO_CMAKE_FIND_ROOT_PATH)
target_link_libraries(app PRIVATE hyperuuid::hyperuuid_static)
```

The archive holds no `CMakeLists.txt`, so `FetchContent_MakeAvailable` only downloads,
checks and unpacks it; `find_package` then reads its CMake files from where it landed.
`NO_CMAKE_FIND_ROOT_PATH` keeps a cross build (iOS, Android) from looking for them inside its
SDK instead. Without FetchContent, a cross build names the archive with
`-Dhyperuuid_DIR=<dir>/lib/cmake/hyperuuid`, for the same reason.

pkg-config, with `<dir>/lib/pkgconfig` on `PKG_CONFIG_PATH`:

```sh
cc app.c $(pkg-config --cflags --libs hyperuuid)
```

The static library needs nothing past the C runtime. To link it with pkg-config, name it
directly: `cc app.c $(pkg-config --cflags hyperuuid) <dir>/lib/libhyperuuid.a`.

The shared library is found at run time by its own name: `@rpath/libhyperuuid.dylib` on
macOS and the soname `libhyperuuid.so` on Linux and Android, so a program needs an rpath
(`-Wl,-rpath,<dir>/lib`) or the library on the loader's path. On Windows the DLL goes beside
the program.

## Use

```c
#include <stdio.h>
#include "hyperuuid.h"

int main(void) {
  uint8_t id[16];
  if (uuid_new_v4(id) != 0) return 1;
  for (int i = 0; i < 16; i++) printf("%02x", id[i]);
  printf("\n");

  uint32_t v = hyperuuid_version();
  printf("hyperuuid %u.%u.%u\n", v >> 16, (v >> 8) & 0xff, v & 0xff);
  return 0;
}
```

UUIDs are sixteen bytes in, sixteen bytes out; the header documents every function and
return code. `hyperuuid_version()` is the one way to read the version at run time. A minor
release may add functions; only a major release changes or removes one.

The core takes the time from the caller for v7 (`uuid_new_v7(unix_millis, out)`), so the
program's clock is the clock. Ordering within a millisecond, the counter and the SQL Server
layout are described in the main README.

## C++

`hyperuuid.hpp` is header-only and needs a C++23 standard library with `std::expected` (it
uses none of the later monadic operations). CI builds it with GCC on Linux, Apple Clang on
macOS, iOS and Mac Catalyst, and MSVC on Windows. It links against the same targets.

```cmake
target_link_libraries(app PRIVATE hyperuuid::hyperuuid_static)
target_compile_features(app PRIVATE cxx_std_23)
```

```cpp
#include <print>
#include "hyperuuid.hpp"

int main() {
  auto id = hyperuuid::new_v7();                       // std::expected<uuid, errc>
  if (!id) {
    std::println("{}", make_error_code(id.error()).message());
    return 1;
  }
  std::println("{}", id->to_string());

  auto sql = hyperuuid::v7_to_sql_order(*id);           // the uniqueidentifier order
  auto when = hyperuuid::get_timestamp(*sql, hyperuuid::uuid_layout::sql_server);
  auto ns = hyperuuid::new_v5(hyperuuid::namespaces::dns, "example.com");
  auto batch = hyperuuid::new_v7_batch(1000);           // one call into the core
  return when && ns && batch ? 0 : 1;
}
```

The generating calls return `std::expected<T, hyperuuid::errc>`, never throw, and `errc` is a
`std::error_code` enum. The rest mirrors the other bindings: `fill_v7` into a `std::span`,
`v7_unix_millis`, `version`, `variant` and `is_rfc` in either layout, `uuid::parse` and
`to_string`, `nil_uuid` and `max_uuid` (not `nil` and `max`, which Objective-C++ and
`windows.h` define as macros), the RFC namespaces, and `native_version()`.
