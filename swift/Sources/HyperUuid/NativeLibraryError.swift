import Foundation

/// Never thrown. This was the error for a shared library that couldn't be found, opened or
/// resolved, back when macOS and Windows loaded the native core at run time. The core is
/// now linked into the executable on every platform, so there is no load left to fail and
/// the type has no cases. It stays so that an existing `catch let error as NativeLibraryError`
/// still compiles (with a deprecation warning) instead of breaking the build.
@available(
    *, deprecated,
    message: "Never thrown: the native core is linked in on every platform, so nothing is loaded at run time."
)
public enum NativeLibraryError: Error, CustomStringConvertible, LocalizedError {
    /// Unreachable: the type has no values.
    public var description: String {
        switch self {}
    }

    /// Unreachable: the type has no values.
    public var errorDescription: String? { description }
}
