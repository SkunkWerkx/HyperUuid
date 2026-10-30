// A C program built against an unpacked release archive, the way a consumer would: through
// CMake's find_package or pkg-config, linked to the shared or the static library. run.sh
// builds it every way the platform offers and checks what it prints against the package's
// version.

#include <stdio.h>
#include <string.h>

#include "hyperuuid.h"

// uuid5(NAMESPACE_DNS, "python.org"), as Python's uuid module gives it.
static const uint8_t dns_namespace[16] = {0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1,
                                          0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8};
static const uint8_t python_org[16] = {0x88, 0x63, 0x13, 0xe1, 0x3b, 0x8a, 0x53, 0x72,
                                       0x9b, 0x90, 0x0c, 0x9a, 0xee, 0x19, 0x9e, 0x5d};

#define CHECK(condition)                                                                           \
  do {                                                                                             \
    if (!(condition)) {                                                                            \
      fprintf(stderr, "smoke: failed: %s\n", #condition);                                          \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

int main(void) {
  uint8_t id[16];
  uint8_t batch[4 * 16];
  const uint64_t millis = 1700000000000ULL;
  const char name[] = "python.org";

  CHECK(uuid_new_v4(id) == 0);
  CHECK(uuid_version(id, 1) == 4 && uuid_variant(id) == 2);

  CHECK(uuid_new_v5(dns_namespace, (const uint8_t *)name, (uint32_t)strlen(name), id) == 0);
  CHECK(memcmp(id, python_org, sizeof id) == 0);

  CHECK(uuid_new_v7(millis, id) == 0);
  CHECK(uuid_v7_unix_millis(id) == millis);
  CHECK(uuid_v7_to_sql_order(id) == 0);
  CHECK(uuid_v7_unix_millis_in(id, 2) == millis);
  CHECK(uuid_new_v7_batch(millis, 4, batch) == 0);
  CHECK(memcmp(batch, batch + 16, 16) < 0);

  uint32_t version = hyperuuid_version();
  printf("%u.%u.%u\n", version >> 16, (version >> 8) & 0xff, version & 0xff);
  return 0;
}
