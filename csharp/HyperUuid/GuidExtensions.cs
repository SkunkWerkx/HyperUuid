namespace HyperUuid;

/// <summary>
/// Extension members on <see cref="Guid"/> for reading what an RFC 9562 time-based UUID
/// already carries inside it.
/// </summary>
/// <remarks>
/// <para>
/// This is a C# 14 <c>extension</c> block rather than the classic
/// <c>this Guid</c> parameter form, because the classic form cannot express a property at
/// all — only methods. <c>id.Timestamp</c> is the shape this data wants: reading the
/// timestamp out of a v6/v7 UUID is a projection of bits the value already holds, not an
/// action it performs, and every other accessor of that kind on a .NET value type is a
/// property. The extension block is what makes that spelling available from outside
/// <see cref="Guid"/>, which this binding cannot modify.
/// </para>
/// <para>
/// Deliberately additive, not a replacement: <see cref="UuidGenerator.GetTimestamp"/> stays
/// the primary entry point and holds the actual logic. This block only re-spells it, so
/// there is exactly one implementation to keep correct, and a caller who prefers the static
/// call — or who wants it without importing this namespace — loses nothing by ignoring
/// these members.
/// </para>
/// <para>
/// Extension members lower to ordinary static methods, so this adds no allocation, no
/// interface dispatch, and nothing for the trimmer or Native AOT to chase — the same
/// guarantees the rest of this assembly makes.
/// </para>
/// </remarks>
public static class GuidExtensions
{
    extension(Guid uuid)
    {
        /// <summary>
        /// The UTC timestamp embedded in this UUID, or <see langword="null"/> when it isn't a
        /// version 6 or version 7 UUID and therefore carries no timestamp at all.
        /// </summary>
        /// <value>
        /// The creation time recovered from the UUID's own bits, or <see langword="null"/> for
        /// any other version — including v4, v5, <see cref="UuidGenerator.Nil"/> and
        /// <see cref="UuidGenerator.Max"/>.
        /// </value>
        /// <remarks>
        /// <para>
        /// Nullable rather than throwing, because "this Guid isn't time-based" is an ordinary,
        /// expected answer for a type that is far more often not a v6/v7 UUID than it is.
        /// Callers typically already have a <see cref="Guid"/> of unknown provenance, which is
        /// exactly the case <see cref="UuidGenerator.V6Timestamp"/> and
        /// <see cref="UuidGenerator.V7Timestamp"/> deliberately do not serve: those assume the
        /// caller already knows the version and read the timestamp field unconditionally.
        /// </para>
        /// <para>
        /// Composes with the usual null handling — <c>id.Timestamp ?? fallback</c>,
        /// <c>id.Timestamp?.ToLocalTime()</c>, or <c>if (id.Timestamp is { } created)</c>.
        /// </para>
        /// <para>
        /// This can still throw <see cref="ArgumentOutOfRangeException"/> for a spec-valid v7
        /// UUID whose embedded timestamp lands past year 9999, which
        /// <see cref="DateTimeOffset"/> cannot represent. That is inherited from
        /// <see cref="UuidGenerator.V7Timestamp"/> and is deliberately not swallowed into
        /// <see langword="null"/>: a v7 UUID from the year 12000 genuinely does carry a
        /// timestamp, so reporting "no timestamp" would be a lie about the value rather than a
        /// description of it.
        /// </para>
        /// </remarks>
        /// <example>
        /// <code>
        /// Guid id = UuidGenerator.NewV7();
        /// DateTimeOffset? created = id.Timestamp;   // the creation time
        /// DateTimeOffset? none = Guid.NewGuid().Timestamp;   // null — a v4 UUID
        /// </code>
        /// </example>
        public DateTimeOffset? Timestamp => UuidGenerator.GetTimestamp(uuid);
    }
}
