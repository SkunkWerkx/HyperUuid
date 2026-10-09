<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * The variant field of a UUID (RFC 9562 §4.1), which says how the rest of its bits are laid
 * out; see {@see Uuid::variant()}. Only {@see UuidVariant::Rfc9562} has versions. The case
 * values are the native core's variant codes, so `Rfc9562`'s value is 2, the same `0b10` the
 * field's top two bits hold.
 */
enum UuidVariant: int
{
    /** `0xxx`: reserved, Network Computing System backward compatibility. Includes Nil. */
    case Ncs = 1;

    /** `10xx`: the variant RFC 9562 (and RFC 4122 before it) specifies. */
    case Rfc9562 = 2;

    /** `110x`: reserved, Microsoft Corporation backward compatibility. */
    case Microsoft = 3;

    /** `111x`: reserved for future definition. Includes Max. */
    case Future = 4;
}
