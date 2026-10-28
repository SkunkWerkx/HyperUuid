package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;

/**
 * Every reference parameter of every public door refuses {@code null} with a {@link
 * NullPointerException} named for the parameter, before any native call, so the JNI path (where
 * a null array would be a native fault) and the FFM path answer alike.
 */
class NullArgumentTest {
    private static Object valid(Class<?> type) {
        if (type == UUID.class) {
            return UUID.randomUUID();
        }
        if (type == String.class) {
            return "name";
        }
        if (type == byte[].class) {
            return new byte[16];
        }
        if (type == UUID[].class) {
            return new UUID[1];
        }
        if (type == Instant.class) {
            return Instant.now();
        }
        if (type == java.nio.charset.Charset.class) {
            return StandardCharsets.UTF_8;
        }
        if (type == UuidLayout.class) {
            return UuidLayout.RFC_9562;
        }
        if (type == int.class) {
            return 7;
        }
        if (type == long.class) {
            return 0L;
        }
        throw new AssertionError("no valid value for " + type);
    }

    private static String expectedName(Method m, Class<?> type) {
        if (type == String.class) {
            return "name";
        }
        if (type == java.nio.charset.Charset.class) {
            return "charset";
        }
        if (type == Instant.class) {
            return "instant";
        }
        if (type == UuidLayout.class) {
            return "layout";
        }
        if (type == UUID[].class || m.getName().equals("fillV7")) {
            return "destination";
        }
        if (m.getName().equals("newV5")) {
            return type == UUID.class ? "namespace" : "name";
        }
        return "uuid";
    }

    // The one that was a native fault on the JNI path: a null name array reaching GetArrayLength.
    @Test
    void nullNameBytesIsANullPointerException() {
        NullPointerException e = org.junit.jupiter.api.Assertions.assertThrows(
                NullPointerException.class, () -> UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, (byte[]) null));
        assertEquals("name", e.getMessage());
    }

    @Test
    void everyReferenceParameterRefusesNull() throws Exception {
        int checked = 0;
        for (Method m : UuidGenerator.class.getDeclaredMethods()) {
            if (!Modifier.isPublic(m.getModifiers()) || !Modifier.isStatic(m.getModifiers())) {
                continue;
            }
            Class<?>[] types = m.getParameterTypes();
            for (int i = 0; i < types.length; i++) {
                if (types[i].isPrimitive()) {
                    continue;
                }
                Object[] args = new Object[types.length];
                for (int j = 0; j < types.length; j++) {
                    args[j] = j == i ? null : valid(types[j]);
                }
                InvocationTargetException e = org.junit.jupiter.api.Assertions.assertThrows(
                        InvocationTargetException.class,
                        () -> m.invoke(null, args),
                        m.toGenericString() + " with null at " + i);
                assertTrue(
                        e.getCause() instanceof NullPointerException,
                        m.toGenericString() + " arg " + i + " threw " + e.getCause());
                assertEquals(expectedName(m, types[i]), e.getCause().getMessage(), m.toGenericString() + " arg " + i);
                checked++;
            }
        }
        // A new public door with a reference parameter changes this count on purpose.
        assertEquals(39, checked);
    }
}
