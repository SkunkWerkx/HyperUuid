package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.net.URISyntaxException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.function.Consumer;
import java.util.function.UnaryOperator;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.junit.jupiter.api.Test;

/**
 * Replays the shared conformance corpus ({@code corpus/*.json} at the repository root), the
 * same files the Rust core's own suite replays, through this binding's public API. Every
 * value crosses as the {@link UUID} a caller would hold: its two longs are the
 * hex's sixteen bytes, most significant first, in either layout, exactly as
 * {@link UuidGenerator#v7ToSqlOrder(UUID)} returns a SQL-ordered value. A vector that fails
 * here is a break in the cross-language contract, and for {@code sql_order.json} a change to
 * data already persisted in SQL Server; {@code sql_order_refused.json} pins the values the
 * conversions refuse, every one with the wrong-version exception.
 */
class CorpusTest {
    private static final Path CORPUS = findCorpus();

    private static Path findCorpus() {
        Path start;
        try {
            start = Path.of(CorpusTest.class
                            .getProtectionDomain()
                            .getCodeSource()
                            .getLocation()
                            .toURI())
                    .toAbsolutePath();
        } catch (URISyntaxException e) {
            throw new IllegalStateException(e);
        }
        for (Path dir = start; dir != null; dir = dir.getParent()) {
            Path corpus = dir.resolve("corpus");
            if (Files.isDirectory(corpus)) {
                return corpus;
            }
        }
        throw new IllegalStateException("corpus directory not found above " + start);
    }

    // The corpus is an array of flat objects whose values are strings without escapes,
    // integers, booleans and null — small enough to read without a JSON dependency.
    private static final Pattern OBJECT = Pattern.compile("\\{([^}]*)}");
    private static final Pattern MEMBER = Pattern.compile("\"([a-z_]+)\"\\s*:\\s*(\"[^\"]*\"|[^,\\s]+)");

    private static List<Map<String, Object>> corpus(String name) {
        String text;
        try {
            text = Files.readString(CORPUS.resolve(name), StandardCharsets.UTF_8);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        List<Map<String, Object>> vectors = new ArrayList<>();
        Matcher object = OBJECT.matcher(text);
        while (object.find()) {
            Map<String, Object> vector = new LinkedHashMap<>();
            Matcher member = MEMBER.matcher(object.group(1));
            while (member.find()) {
                String raw = member.group(2);
                Object value;
                if (raw.startsWith("\"")) {
                    value = raw.substring(1, raw.length() - 1);
                } else if (raw.equals("null")) {
                    value = null;
                } else if (raw.equals("true") || raw.equals("false")) {
                    value = Boolean.parseBoolean(raw);
                } else {
                    value = Long.parseLong(raw);
                }
                vector.put(member.group(1), value);
            }
            vectors.add(vector);
        }
        if (vectors.isEmpty()) {
            throw new IllegalStateException(name + " has no vectors");
        }
        return vectors;
    }

    private static byte[] hex(Map<String, Object> vector, String field) {
        return HexFormat.of().parseHex((String) vector.get(field));
    }

    private static UuidLayout layoutOf(Map<String, Object> vector) {
        return switch ((String) vector.get("layout")) {
            case "rfc9562" -> UuidLayout.RFC_9562;
            case "sql_server" -> UuidLayout.SQL_SERVER;
            default -> throw new IllegalStateException("unknown layout in " + vector);
        };
    }

    private static int intOf(Map<String, Object> vector, String field) {
        return ((Long) vector.get(field)).intValue();
    }

    @Test
    void v5Corpus() {
        for (Map<String, Object> vector : corpus("v5.json")) {
            UUID namespace = switch ((String) vector.get("namespace")) {
                case "dns" -> UuidGenerator.Namespaces.DNS;
                case "url" -> UuidGenerator.Namespaces.URL;
                case "oid" -> UuidGenerator.Namespaces.OID;
                case "x500" -> UuidGenerator.Namespaces.X500;
                default -> throw new IllegalStateException("unknown namespace in " + vector);
            };
            UUID expected = RfcBytes.fromRfcBytes(hex(vector, "expect"));
            String message = vector.toString();
            assertEquals(expected, UuidGenerator.newV5(namespace, hex(vector, "name_hex")), message);
            if (vector.containsKey("name")) {
                assertEquals(expected, UuidGenerator.newV5(namespace, (String) vector.get("name")), message);
            }
        }
    }

    @Test
    void sqlOrderCorpus() {
        for (Map<String, Object> vector : corpus("sql_order.json")) {
            byte[] rfcBytes = hex(vector, "rfc");
            byte[] sqlBytes = hex(vector, "sql");
            UUID rfc = RfcBytes.fromRfcBytes(rfcBytes);
            UUID sql = RfcBytes.fromRfcBytes(sqlBytes);
            String message = vector.toString();
            assertEquals(sql, UuidGenerator.v7ToSqlOrder(rfc), message);
            assertEquals(rfc, UuidGenerator.v7FromSqlOrder(sql), message);
            // What reaches SQL Server is the UUID's sixteen bytes, most significant first.
            assertArrayEquals(sqlBytes, RfcBytes.toRfcBytes(UuidGenerator.v7ToSqlOrder(rfc)), message);

            // The raw-byte doors, in place.
            byte[] bytes = rfcBytes.clone();
            UuidGenerator.v7ToSqlOrder(bytes);
            assertArrayEquals(sqlBytes, bytes, message);
            UuidGenerator.v7FromSqlOrder(bytes);
            assertArrayEquals(rfcBytes, bytes, message);
        }
    }

    // The one refusal, exactly as the binding throws it.
    private static final String WRONG_VERSION = "the value is not a UUID of the version this conversion takes";

    @Test
    void sqlOrderRefusedCorpus() {
        for (Map<String, Object> vector : corpus("sql_order_refused.json")) {
            byte[] bytes = hex(vector, "uuid");
            UUID id = RfcBytes.fromRfcBytes(bytes);
            boolean toSql = switch ((String) vector.get("direction")) {
                case "to_sql" -> true;
                case "from_sql" -> false;
                default -> throw new IllegalStateException("unknown direction in " + vector);
            };
            String message = vector.toString();
            UnaryOperator<UUID> convert = toSql ? UuidGenerator::v7ToSqlOrder : UuidGenerator::v7FromSqlOrder;
            Consumer<byte[]> convertBytes = toSql ? UuidGenerator::v7ToSqlOrder : UuidGenerator::v7FromSqlOrder;

            IllegalArgumentException thrown =
                    assertThrows(IllegalArgumentException.class, () -> convert.apply(id), message);
            assertEquals(WRONG_VERSION, thrown.getMessage(), message);

            // The raw-byte door refuses the same way and leaves the caller's bytes untouched.
            byte[] inPlace = bytes.clone();
            thrown = assertThrows(IllegalArgumentException.class, () -> convertBytes.accept(inPlace), message);
            assertEquals(WRONG_VERSION, thrown.getMessage(), message);
            assertArrayEquals(bytes, inPlace, message);
        }
    }

    @Test
    void timestampCorpus() {
        for (Map<String, Object> vector : corpus("timestamp.json")) {
            UuidLayout layout = layoutOf(vector);
            UUID id = RfcBytes.fromRfcBytes(hex(vector, "uuid"));
            int version = intOf(vector, "version");
            Long millis = (Long) vector.get("unix_millis");
            String message = vector.toString();
            assertEquals(version, UuidGenerator.version(id, layout), message);

            // The version-agnostic doors answer only for a timestamp the core would mint: a
            // version 7 dated past the generation cap, 253402300799999 ms, has none there, as a
            // version 6 has none. The per-version reads below read the 48-bit field as it is
            // (an Instant reaches far past its 2^48 - 1 ms, the year 10889).
            Optional<Instant> expected = Optional.ofNullable(millis)
                    .filter(ms -> ms <= 253_402_300_799_999L)
                    .map(Instant::ofEpochMilli);
            assertEquals(expected, UuidGenerator.getTimestamp(id, layout), message);
            if (layout == UuidLayout.RFC_9562) {
                assertEquals(expected, UuidGenerator.getTimestamp(id), message);
            }
            if (millis == null) {
                continue;
            }
            if (version != 7) {
                throw new IllegalStateException("a timestamp on a version " + version + ": " + vector);
            }
            Instant field = Instant.ofEpochMilli(millis);
            assertEquals(millis, UuidGenerator.v7UnixMillis(id, layout), message);
            assertEquals(field, UuidGenerator.v7Timestamp(id, layout), message);
            if (layout == UuidLayout.RFC_9562) {
                assertEquals(millis, UuidGenerator.v7UnixMillis(id), message);
                assertEquals(field, UuidGenerator.v7Timestamp(id), message);
            }
        }
    }

    @Test
    void inspectCorpus() {
        for (Map<String, Object> vector : corpus("inspect.json")) {
            UuidLayout layout = layoutOf(vector);
            byte[] bytes = hex(vector, "uuid");
            UUID id = RfcBytes.fromRfcBytes(bytes);
            int version = intOf(vector, "version");
            boolean isRfc = (Boolean) vector.get("is_rfc");
            String message = vector.toString();

            assertEquals(version, UuidGenerator.version(id, layout), message);
            assertEquals(version, UuidGenerator.version(bytes, layout), message);
            assertEquals(isRfc, UuidGenerator.isRfc(id, version, layout), message);
            assertEquals(isRfc, UuidGenerator.isRfc(bytes, version, layout), message);
            for (int other = 0; other <= 15; other++) {
                if (other != version) {
                    assertFalse(UuidGenerator.isRfc(id, other, layout), message + " isRfc(" + other + ")");
                    assertFalse(UuidGenerator.isRfc(bytes, other, layout), message + " isRfc(byte[], " + other + ")");
                }
            }

            if (!vector.containsKey("variant")) {
                continue;
            }
            UuidVariant expected = switch ((String) vector.get("variant")) {
                case "ncs" -> UuidVariant.NCS;
                case "rfc9562" -> UuidVariant.RFC_9562;
                case "microsoft" -> UuidVariant.MICROSOFT;
                case "future" -> UuidVariant.FUTURE;
                default -> throw new IllegalStateException("unknown variant in " + vector);
            };
            assertEquals(expected, UuidGenerator.variant(id), message);
            assertEquals(expected, UuidGenerator.variant(bytes), message);
            assertEquals(version, UuidGenerator.version(id), message);
            assertEquals(version, UuidGenerator.version(bytes), message);
            assertEquals(version, id.version(), message + " (the JDK agrees)");
            assertEquals(isRfc, UuidGenerator.isRfc(id, version), message);
            assertEquals(isRfc, UuidGenerator.isRfc(bytes, version), message);
        }
    }
}
