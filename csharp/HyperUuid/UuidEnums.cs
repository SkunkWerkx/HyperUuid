namespace HyperUuid;

/// <summary>
/// The variant field of a UUID (RFC 9562 §4.1), which says how the rest of its bits are laid
/// out; see <see cref="UuidGenerator.Variant(Guid)"/>. Only <see cref="Rfc9562"/> has versions.
/// </summary>
/// <remarks>
/// The values are the native core's codes, with 0 reserved so a value nobody set is
/// recognisable as such rather than reading as a real variant.
/// </remarks>
public enum UuidVariant : uint
{
	/// <summary>No variant: the default value, never returned by <see cref="UuidGenerator.Variant(Guid)"/>.</summary>
	Unspecified = 0,
	/// <summary><c>0xxx</c>: reserved, Network Computing System backward compatibility. Includes Nil.</summary>
	Ncs = 1,
	/// <summary><c>10xx</c>: the variant RFC 9562 (and RFC 4122 before it) specifies.</summary>
	Rfc9562 = 2,
	/// <summary><c>110x</c>: reserved, Microsoft Corporation backward compatibility.</summary>
	Microsoft = 3,
	/// <summary><c>111x</c>: reserved for future definition. Includes Max.</summary>
	Future = 4,
}

/// <summary>
/// The byte order a UUID is held in, for the inspection and timestamp methods that take one.
/// </summary>
/// <remarks>
/// The values are the native core's codes, with 0 reserved: passing
/// <see cref="Unspecified"/> (a <c>default</c> left unset) throws
/// <see cref="ArgumentOutOfRangeException"/> rather than guessing a layout.
/// </remarks>
public enum UuidLayout : uint
{
	/// <summary>No layout: the default value, rejected by every method that takes one.</summary>
	Unspecified = 0,
	/// <summary>RFC 9562 order, what every other method in this library takes and returns.</summary>
	Rfc9562 = 1,
	/// <summary>
	/// The order <see cref="UuidGenerator.V7ToSqlOrder(Guid)"/> and
	/// <see cref="UuidGenerator.V6ToSqlOrder(Guid)"/> return, which SQL Server's
	/// <c>uniqueidentifier</c> sorts by creation order. Defined for versions 6 and 7 only.
	/// </summary>
	SqlServer = 2,
}
