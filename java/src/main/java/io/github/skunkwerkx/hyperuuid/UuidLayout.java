package io.github.skunkwerkx.hyperuuid;

/**
 * The byte order a UUID is held in, for the inspection and timestamp methods of
 * {@link UuidGenerator} that take one.
 *
 * <p>Each constant carries the native core's code, with 0 reserved: passing
 * {@link #UNSPECIFIED} throws {@link IllegalArgumentException} rather than guessing a layout.
 */
public enum UuidLayout {
    /** No layout: rejected by every method that takes one. */
    UNSPECIFIED(0),
    /** RFC 9562 order, what every other method in this library takes and returns. */
    RFC_9562(1),
    /**
     * The order {@link UuidGenerator#v7ToSqlOrder(java.util.UUID)} and
     * {@link UuidGenerator#v6ToSqlOrder(java.util.UUID)} return, which SQL Server's
     * {@code uniqueidentifier} sorts by creation order. Defined for versions 6 and 7 only.
     */
    SQL_SERVER(2);

    private final int code;

    UuidLayout(int code) {
        this.code = code;
    }

    /**
     * The native core's code for this layout.
     *
     * @return 0 for {@link #UNSPECIFIED}, 1 for {@link #RFC_9562}, 2 for {@link #SQL_SERVER}
     */
    public int code() {
        return code;
    }
}
