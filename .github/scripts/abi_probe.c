/* The ABI probe release_guard.py (in the Forge) compiles against the Swift bundle's
 * hyperuuid.h and links with one committed payload at a time, so each process loads or
 * links exactly one library. EXPECTED_VERSION arrives on the command line as the packed
 * MAJOR << 16 | MINOR << 8 | PATCH that hyperuuid_version() returns. A non-zero exit is a
 * mismatch between the header, the library and the version being released.
 *
 * The body is HyperUuid's own: the version, then the v7 <-> SQL Server reorder round trip on
 * a real v7 and its refusal (5) on a v6, which are the behaviours whose bytes are persisted
 * in other people's indexes (README "The layout is persisted data"). */
#include <string.h>
#include "hyperuuid.h"
int main(void) {
    if (hyperuuid_version() != EXPECTED_VERSION) return 1;
    uint8_t v7[16] = {0x01,0x7f,0x22,0xe2,0x79,0xb0,0x7c,0xc3,0x98,0xc4,0xdc,0x0c,0x0c,0x07,0x39,0x8f};
    uint8_t v6[16] = {0x1e,0xc9,0x41,0x4c,0x23,0x2a,0x6b,0x00,0xb3,0xc8,0x9f,0x6b,0xde,0xce,0xd8,0x46};
    uint8_t out[16];
    memcpy(out, v7, 16);
    if (uuid_v7_to_rfc_order(out) != 5 || memcmp(out, v7, 16)) return 2;
    if (uuid_v7_to_sql_order(out) != 0) return 3;
    if ((out[7] >> 4) != 8 || (out[8] >> 6) != 2) return 4;
    if (uuid_v7_to_rfc_order(out) != 0 || memcmp(out, v7, 16)) return 5;
    memcpy(out, v6, 16);
    if (uuid_v7_to_sql_order(out) != 5 || memcmp(out, v6, 16)) return 6;
    return 0;
}
