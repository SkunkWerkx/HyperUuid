// The C++ face of the hyperuuid core (https://github.com/SkunkWerkx/HyperUuid): header-only,
// over the C ABI in hyperuuid.h, and linked against the same shared or static library. C++23
// (std::expected).
//
// A generating call returns std::expected<T, hyperuuid::errc>: the core's failures (the random
// source, a timestamp the 48-bit field cannot hold, a batch too large, a value of the wrong
// version, an exhausted v7 stream) are values, not exceptions. errc is also a std::error_code
// enum. Nothing here throws except std::vector's allocation in new_v7_batch.
//
// The v7 calls without a timestamp read std::chrono::system_clock. Ordering within a
// millisecond, the counter and the SQL Server layout are the core's, documented in hyperuuid.h
// and the main README.

#ifndef HYPERUUID_HPP
#define HYPERUUID_HPP

#include <array>
#include <chrono>
#include <compare>
#include <cstddef>
#include <cstdint>
#include <expected>
#include <limits>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <system_error>
#include <type_traits>
#include <vector>

#include "hyperuuid.h"

namespace hyperuuid {

// The core's failures, numbered as its C return codes, and name_too_long, which this header
// checks itself before calling the core (numbered past the C codes so none can collide).
enum class errc : int {
  random_source_failure = 1,  // the operating system's random source failed
  timestamp_out_of_range = 2, // the timestamp does not fit the 48-bit millisecond field
  batch_not_addressable = 3,  // count * 16 bytes cannot be addressed (32-bit targets only)
  batch_too_large = 4,        // count is over max_v7_batch
  wrong_version = 5,          // not the version 7 (or SQL-ordered version 7) the conversion takes
  counter_exhausted = 6,      // the v7 stream cannot advance; nothing was reserved
  name_too_long = 256,        // a v5 name over 2^32 - 1 bytes, the C ABI's length limit
};

// The byte order a UUID is held in.
enum class uuid_layout : std::uint32_t {
  rfc9562 = 1,   // RFC 9562 network order
  sql_server = 2 // SQL Server order, what v7_to_sql_order writes
};

// The variant field (RFC 9562 §4.1).
enum class uuid_variant : std::uint32_t { ncs = 1, rfc9562 = 2, microsoft = 3, future = 4 };

namespace detail {
class category final : public std::error_category {
public:
  const char *name() const noexcept override { return "hyperuuid"; }
  std::string message(int code) const override {
    switch (static_cast<errc>(code)) {
    case errc::random_source_failure:
      return "the random source failed";
    case errc::timestamp_out_of_range:
      return "the timestamp does not fit the 48-bit millisecond field";
    case errc::batch_not_addressable:
      return "the batch is larger than this platform can address";
    case errc::batch_too_large:
      return "the batch is larger than max_v7_batch";
    case errc::wrong_version:
      return "the value is not of the version the conversion takes";
    case errc::counter_exhausted:
      return "the version 7 stream cannot advance";
    case errc::name_too_long:
      return "the name is longer than 2^32 - 1 bytes";
    }
    return "unknown hyperuuid error " + std::to_string(code);
  }
};
} // namespace detail

inline const std::error_category &error_category() noexcept {
  static const detail::category instance;
  return instance;
}

inline std::error_code make_error_code(errc e) noexcept {
  return {static_cast<int>(e), error_category()};
}

// Sixteen bytes in RFC 9562 order (or SQL Server order, after v7_to_sql_order). Compares and
// orders bytewise, the order RFC-ordered v7 values sort by creation.
struct uuid {
  std::array<std::uint8_t, 16> bytes{};

  friend constexpr bool operator==(const uuid &, const uuid &) = default;
  friend constexpr auto operator<=>(const uuid &, const uuid &) = default;

  // The canonical 36-character form, lowercase: 8-4-4-4-12 hex digits.
  std::string to_string() const {
    static constexpr char digits[] = "0123456789abcdef";
    std::string text;
    text.reserve(36);
    for (std::size_t i = 0; i < bytes.size(); ++i) {
      if (i == 4 || i == 6 || i == 8 || i == 10) {
        text.push_back('-');
      }
      text.push_back(digits[bytes[i] >> 4]);
      text.push_back(digits[bytes[i] & 0x0f]);
    }
    return text;
  }

  // The canonical 36-character form in either case, or nullopt for anything else.
  static constexpr std::optional<uuid> parse(std::string_view text) noexcept {
    if (text.size() != 36) {
      return std::nullopt;
    }
    uuid result;
    std::size_t at = 0;
    for (std::size_t i = 0; i < 16; ++i) {
      if (at == 8 || at == 13 || at == 18 || at == 23) {
        if (text[at] != '-') {
          return std::nullopt;
        }
        ++at;
      }
      const int high = hex(text[at]);
      const int low = hex(text[at + 1]);
      if (high < 0 || low < 0) {
        return std::nullopt;
      }
      result.bytes[i] = static_cast<std::uint8_t>(high << 4 | low);
      at += 2;
    }
    return result;
  }

private:
  static constexpr int hex(char c) noexcept {
    if (c >= '0' && c <= '9') {
      return c - '0';
    }
    if (c >= 'a' && c <= 'f') {
      return c - 'a' + 10;
    }
    if (c >= 'A' && c <= 'F') {
      return c - 'A' + 10;
    }
    return -1;
  }
};

static_assert(sizeof(uuid) == 16 && std::is_trivially_copyable_v<uuid> &&
                  std::is_standard_layout_v<uuid>,
              "a uuid is its sixteen bytes, so a span of them crosses the C ABI as one buffer");

// The RFC's two special values (§5.9, §5.10). Not `nil` and `max`: Objective-C++ defines nil,
// and windows.h defines max, as macros.
inline constexpr uuid nil_uuid{};
inline constexpr uuid max_uuid{{0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                                0xff, 0xff, 0xff, 0xff, 0xff}};

// The name-based namespaces of RFC 9562 §6.6, for new_v5.
namespace namespaces {
inline constexpr uuid dns = *uuid::parse("6ba7b810-9dad-11d1-80b4-00c04fd430c8");
inline constexpr uuid url = *uuid::parse("6ba7b811-9dad-11d1-80b4-00c04fd430c8");
inline constexpr uuid oid = *uuid::parse("6ba7b812-9dad-11d1-80b4-00c04fd430c8");
inline constexpr uuid x500 = *uuid::parse("6ba7b814-9dad-11d1-80b4-00c04fd430c8");
} // namespace namespaces

// The most UUIDs one version 7 batch mints, 2^26: the largest batch takes exactly one
// millisecond of the stream.
inline constexpr std::size_t max_v7_batch = std::size_t{1} << 26;

namespace detail {
inline std::uint8_t *out(uuid &u) noexcept { return u.bytes.data(); }
inline const std::uint8_t *in(const uuid &u) noexcept { return u.bytes.data(); }

inline std::expected<std::uint64_t, errc> unix_millis(std::chrono::system_clock::time_point t) {
  const auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(t.time_since_epoch());
  if (ms.count() < 0) {
    return std::unexpected(errc::timestamp_out_of_range);
  }
  return static_cast<std::uint64_t>(ms.count());
}

inline std::expected<uuid, errc> made(std::int32_t code, const uuid &u) {
  if (code != 0) {
    return std::unexpected(static_cast<errc>(code));
  }
  return u;
}
} // namespace detail

// A random UUID version 4 (RFC 9562 §5.4).
inline std::expected<uuid, errc> new_v4() {
  uuid u;
  return detail::made(::uuid_new_v4(detail::out(u)), u);
}

// A UUID version 5 (RFC 9562 §5.5) from a namespace and a name's bytes: the same pair gives
// the same UUID in every binding, and in Python's uuid.uuid5. A name over 2^32 - 1 bytes is
// name_too_long: the C ABI takes a 32-bit length, and narrowing it would hash a prefix.
inline std::expected<uuid, errc> new_v5(const uuid &ns, std::span<const std::uint8_t> name) {
  if (name.size() > std::numeric_limits<std::uint32_t>::max()) {
    return std::unexpected(errc::name_too_long);
  }
  uuid u;
  return detail::made(::uuid_new_v5(detail::in(ns), name.data(),
                                    static_cast<std::uint32_t>(name.size()), detail::out(u)),
                      u);
}

// A UUID version 5 from a namespace and a name's UTF-8 (or any) bytes.
inline std::expected<uuid, errc> new_v5(const uuid &ns, std::string_view name) {
  return new_v5(ns, std::span(reinterpret_cast<const std::uint8_t *>(name.data()), name.size()));
}

// A time-sortable UUID version 7 (RFC 9562 §6.2) at `unix_millis`, milliseconds since the
// Unix epoch.
inline std::expected<uuid, errc> new_v7(std::uint64_t unix_millis) {
  uuid u;
  return detail::made(::uuid_new_v7(unix_millis, detail::out(u)), u);
}

// A UUID version 7 at `when`, by default now. Before 1970 is timestamp_out_of_range.
inline std::expected<uuid, errc>
new_v7(std::chrono::system_clock::time_point when = std::chrono::system_clock::now()) {
  const auto millis = detail::unix_millis(when);
  if (!millis) {
    return std::unexpected(millis.error());
  }
  return new_v7(*millis);
}

// Fills `destination` with version 7 UUIDs sharing one timestamp, in one call to the core:
// strictly increasing, the later ones stamped a millisecond on if the batch outruns one.
inline std::expected<void, errc> fill_v7(std::span<uuid> destination, std::uint64_t unix_millis) {
  if (destination.size() > max_v7_batch) {
    return std::unexpected(errc::batch_too_large);
  }
  const std::int32_t code =
      ::uuid_new_v7_batch(unix_millis, static_cast<std::uint32_t>(destination.size()),
                          reinterpret_cast<std::uint8_t *>(destination.data()));
  if (code != 0) {
    return std::unexpected(static_cast<errc>(code));
  }
  return {};
}

inline std::expected<void, errc>
fill_v7(std::span<uuid> destination,
        std::chrono::system_clock::time_point when = std::chrono::system_clock::now()) {
  const auto millis = detail::unix_millis(when);
  if (!millis) {
    return std::unexpected(millis.error());
  }
  return fill_v7(destination, *millis);
}

// `count` version 7 UUIDs from one call to the core (fill_v7 into a new vector). A count over
// max_v7_batch is batch_too_large before anything is allocated.
inline std::expected<std::vector<uuid>, errc> new_v7_batch(std::size_t count,
                                                           std::uint64_t unix_millis) {
  if (count > max_v7_batch) {
    return std::unexpected(errc::batch_too_large);
  }
  std::vector<uuid> batch(count);
  if (const auto filled = fill_v7(batch, unix_millis); !filled) {
    return std::unexpected(filled.error());
  }
  return batch;
}

inline std::expected<std::vector<uuid>, errc>
new_v7_batch(std::size_t count,
             std::chrono::system_clock::time_point when = std::chrono::system_clock::now()) {
  const auto millis = detail::unix_millis(when);
  if (!millis) {
    return std::unexpected(millis.error());
  }
  return new_v7_batch(count, *millis);
}

// The Unix-epoch milliseconds in a version 7 UUID. Bit-shifting only: meaningful for a genuine
// version 7 (is_rfc is the check; get_timestamp checks for you).
inline std::uint64_t v7_unix_millis(const uuid &u) noexcept {
  return ::uuid_v7_unix_millis(detail::in(u));
}

// The same, for a version 7 held in `layout` (a SQL-ordered one read in place).
inline std::uint64_t v7_unix_millis(const uuid &u, uuid_layout layout) noexcept {
  return ::uuid_v7_unix_millis_in(detail::in(u), static_cast<std::uint32_t>(layout));
}

// The timestamp of an RFC 9562 version 7 UUID held in `layout`, or nullopt for anything else.
inline std::optional<std::chrono::sys_time<std::chrono::milliseconds>>
get_timestamp(const uuid &u, uuid_layout layout = uuid_layout::rfc9562) noexcept {
  std::uint64_t millis = 0;
  if (::uuid_get_timestamp(detail::in(u), static_cast<std::uint32_t>(layout), &millis) == 0) {
    return std::nullopt;
  }
  return std::chrono::sys_time<std::chrono::milliseconds>(
      std::chrono::milliseconds(static_cast<std::int64_t>(millis)));
}

// An RFC 9562 version 7 in SQL Server order: the version 8 layout a uniqueidentifier column
// sorts by creation. Exact, and v7_from_sql_order inverts it; anything but an RFC 9562 version
// 7 is wrong_version.
inline std::expected<uuid, errc> v7_to_sql_order(uuid u) {
  return detail::made(::uuid_v7_to_sql_order(detail::out(u)), u);
}

// A SQL-ordered version 7 back in RFC 9562 order; anything else is wrong_version.
inline std::expected<uuid, errc> v7_from_sql_order(uuid u) {
  return detail::made(::uuid_v7_to_rfc_order(detail::out(u)), u);
}

// The version of a UUID held in `layout`: the nibble, 0-15, in RFC 9562 order; in SQL Server
// order 7 for a converted version 7 and 0 for anything else.
inline int version(const uuid &u, uuid_layout layout = uuid_layout::rfc9562) noexcept {
  return static_cast<int>(::uuid_version(detail::in(u), static_cast<std::uint32_t>(layout)));
}

// The variant of a UUID in RFC 9562 order.
inline uuid_variant variant(const uuid &u) noexcept {
  return static_cast<uuid_variant>(::uuid_variant(detail::in(u)));
}

// Whether a UUID held in `layout` is an RFC 9562 UUID of `version`.
inline bool is_rfc(const uuid &u, int version, uuid_layout layout = uuid_layout::rfc9562) noexcept {
  return version >= 0 && version <= 15 &&
         ::uuid_is_rfc(detail::in(u), static_cast<std::uint32_t>(version),
                       static_cast<std::uint32_t>(layout)) != 0;
}

// Always true: the core is linked into the program, so there is nothing to fail to load. Kept
// for parity with the bindings that load a shared library at run time.
inline constexpr bool is_available() noexcept { return true; }

// The linked library's own version, "major.minor.patch", from ::hyperuuid_version().
inline std::string native_version() {
  const std::uint32_t v = ::hyperuuid_version();
  return std::to_string(v >> 16) + "." + std::to_string((v >> 8) & 0xff) + "." +
         std::to_string(v & 0xff);
}

} // namespace hyperuuid

template <> struct std::is_error_code_enum<hyperuuid::errc> : std::true_type {};

#endif // HYPERUUID_HPP
