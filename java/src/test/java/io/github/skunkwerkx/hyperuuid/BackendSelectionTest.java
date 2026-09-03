package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

/**
 * Pins that the property really is what picks the path: the plain {@code test} task sets
 * nothing and must land on FFM (this build always stages the platform's native library first),
 * while {@code testWasm} sets {@code hyperuuid.backend=wasm} and must land on GraalWasm. The
 * rest of the suite runs identically under both; this is the one assertion that would catch
 * the switch silently doing nothing.
 */
class BackendSelectionTest {

    @Test
    void backendFollowsTheProperty() {
        String expected = System.getProperty(UuidGenerator.BACKEND_PROPERTY, "native");
        assertEquals(expected, UuidGenerator.backend());
    }
}
