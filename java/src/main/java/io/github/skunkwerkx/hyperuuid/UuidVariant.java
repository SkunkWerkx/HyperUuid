package io.github.skunkwerkx.hyperuuid;

/**
 * The variant field of a UUID (RFC 9562 §4.1), which says how the rest of its bits are laid
 * out; see {@link UuidGenerator#variant(java.util.UUID)}. Only {@link #RFC_9562} has versions.
 *
 * <p>Each constant carries the native core's code, with 0 reserved so a value nobody set is
 * recognisable as such rather than reading as a real variant.
 */
public enum UuidVariant {
    /** No variant: never returned by {@link UuidGenerator#variant(java.util.UUID)}. */
    UNSPECIFIED(0),
    /** {@code 0xxx}: reserved, Network Computing System backward compatibility. Includes Nil. */
    NCS(1),
    /** {@code 10xx}: the variant RFC 9562 (and RFC 4122 before it) specifies. */
    RFC_9562(2),
    /** {@code 110x}: reserved, Microsoft Corporation backward compatibility. */
    MICROSOFT(3),
    /** {@code 111x}: reserved for future definition. Includes Max. */
    FUTURE(4);

    private final int code;

    UuidVariant(int code) {
        this.code = code;
    }

    /**
     * The native core's code for this variant.
     *
     * @return 0 for {@link #UNSPECIFIED} through 4 for {@link #FUTURE}
     */
    public int code() {
        return code;
    }

    static UuidVariant of(int code) {
        return switch (code) {
            case 1 -> NCS;
            case 2 -> RFC_9562;
            case 3 -> MICROSOFT;
            case 4 -> FUTURE;
            default -> UNSPECIFIED;
        };
    }
}
