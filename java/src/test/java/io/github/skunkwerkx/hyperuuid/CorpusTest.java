package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

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
import java.util.function.UnaryOperator;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.junit.jupiter.api.Test;

/**
 * Replays the shared conformance corpus ({@code corpus/*.json} at the repository root), the
 * same files the Rust core's own suite replays, through this binding's public API — on
 * whichever backend this run selected, so {@code test} and {@code testWasm} replay it once
 * each. Every value crosses as the {@link UUID} a caller would hold: its two longs are the
 * hex's sixteen bytes, most significant first, in either layout, exactly as
 * {@link UuidGenerator#v7ToSqlOrder(UUID)} returns a SQL-ordered value. A vector that fails
 * here is a break in the cross-language contract, and for {@code sql_order.json} a change to
 * data already persisted in SQL Server.
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
            int version = intOf(vector, "version");
            String message = vector.toString();
            UnaryOperator<UUID> toSql = version == 6 ? UuidGenerator::v6ToSqlOrder : UuidGenerator::v7ToSqlOrder;
            UnaryOperator<UUID> toRfc = version == 6 ? UuidGenerator::v6FromSqlOrder : UuidGenerator::v7FromSqlOrder;
            assertEquals(sql, toSql.apply(rfc), message);
            assertEquals(rfc, toRfc.apply(sql), message);
            // What reaches SQL Server is the UUID's sixteen bytes, most significant first.
            assertArrayEquals(sqlBytes, RfcBytes.toRfcBytes(toSql.apply(rfc)), message);

            // The raw-byte doors, in place.
            byte[] bytes = rfcBytes.clone();
            if (version == 6) {
                UuidGenerator.v6ToSqlOrder(bytes);
            } else {
                UuidGenerator.v7ToSqlOrder(bytes);
            }
            assertArrayEquals(sqlBytes, bytes, message);
            if (version == 6) {
                UuidGenerator.v6FromSqlOrder(bytes);
            } else {
                UuidGenerator.v7FromSqlOrder(bytes);
            }
            assertArrayEquals(rfcBytes, bytes, message);
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

            // Instant reaches far past v7's 2^48 - 1 ms (the year 10889), so unlike the C#
            // and Python bindings no row needs a documented exception here.
            Optional<Instant> expected = Optional.ofNullable(millis).map(Instant::ofEpochMilli);
            assertEquals(expected, UuidGenerator.getTimestamp(id, layout), message);
            if (layout == UuidLayout.RFC_9562) {
                assertEquals(expected, UuidGenerator.getTimestamp(id), message);
            }
            if (millis == null) {
                continue;
            }
            switch (version) {
                case 6 -> {
                    assertEquals(millis, UuidGenerator.v6UnixMillis(id, layout), message);
                    assertEquals(expected.orElseThrow(), UuidGenerator.v6Timestamp(id, layout), message);
                    if (layout == UuidLayout.RFC_9562) {
                        assertEquals(millis, UuidGenerator.v6UnixMillis(id), message);
                        assertEquals(expected.orElseThrow(), UuidGenerator.v6Timestamp(id), message);
                    }
                }
                case 7 -> {
                    assertEquals(millis, UuidGenerator.v7UnixMillis(id, layout), message);
                    assertEquals(expected.orElseThrow(), UuidGenerator.v7Timestamp(id, layout), message);
                    if (layout == UuidLayout.RFC_9562) {
                        assertEquals(millis, UuidGenerator.v7UnixMillis(id), message);
                        assertEquals(expected.orElseThrow(), UuidGenerator.v7Timestamp(id), message);
                    }
                }
                default -> throw new IllegalStateException("a timestamp on a version " + version + ": " + vector);
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
