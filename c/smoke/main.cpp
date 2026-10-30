// The C++ header against an unpacked release archive: the same checks as main.c, through
// hyperuuid.hpp. run.sh builds it with CMake wherever the C++ compiler has C++23's
// <expected>, and prints what it reports beside main.c's.

#include <cstdio>
#include <string_view>
#include <system_error>

#include "hyperuuid.hpp"

#define CHECK(condition)                                                                           \
  do {                                                                                             \
    if (!(condition)) {                                                                            \
      std::fprintf(stderr, "smoke: failed: %s\n", #condition);                                     \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

int main() {
  namespace hu = hyperuuid;
  constexpr std::uint64_t millis = 1700000000000ULL;

  auto v4 = hu::new_v4();
  CHECK(v4 && hu::version(*v4) == 4 && hu::variant(*v4) == hu::uuid_variant::rfc9562);
  CHECK(hu::uuid::parse(v4->to_string()) == *v4);

  auto v5 = hu::new_v5(hu::namespaces::dns, std::string_view("python.org"));
  CHECK(v5 && v5->to_string() == "886313e1-3b8a-5372-9b90-0c9aee199e5d");

  auto v7 = hu::new_v7(millis);
  CHECK(v7 && hu::is_rfc(*v7, 7) && hu::v7_unix_millis(*v7) == millis);
  CHECK(hu::v7_unix_millis(*hu::v7_to_sql_order(*v7), hu::uuid_layout::sql_server) == millis);
  auto sql = hu::v7_to_sql_order(*v7);
  CHECK(sql && hu::version(*sql, hu::uuid_layout::sql_server) == 7);
  CHECK(hu::get_timestamp(*sql, hu::uuid_layout::sql_server)->time_since_epoch().count() ==
        static_cast<long long>(millis));
  CHECK(hu::v7_from_sql_order(*sql) == *v7);
  CHECK(hu::v7_to_sql_order(*v4).error() == hu::errc::wrong_version);
  CHECK(!hu::get_timestamp(*v4));

  auto batch = hu::new_v7_batch(4, millis);
  CHECK(batch && batch->size() == 4 && (*batch)[0] < (*batch)[1]);
  CHECK(hu::new_v7_batch(hu::max_v7_batch + 1).error() == hu::errc::batch_too_large);
  CHECK(hu::new_v7(std::uint64_t{1} << 48).error() == hu::errc::timestamp_out_of_range);
  std::error_code code = hu::errc::counter_exhausted;
  CHECK(code.category().name() == std::string_view("hyperuuid"));
  // Checked by the header, not the core: a name the 32-bit C length cannot carry.
  CHECK(make_error_code(hu::errc::name_too_long).message() ==
        "the name is longer than 2^32 - 1 bytes");

  CHECK(hu::nil_uuid < hu::max_uuid && hu::is_available());
  std::printf("%s\n", hu::native_version().c_str());
  return 0;
}
