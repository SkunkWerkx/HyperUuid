//! The int-backed enums carrying the core's layout and variant codes.

use ext_php_rs::prelude::*;

/// Byte order a UUID is held in.
#[php_enum]
#[php(name = "HyperUuid\\UuidLayout")]
pub enum UuidLayout {
    #[php(value = 1)]
    Rfc9562,
    #[php(value = 2)]
    SqlServer,
}

/// The RFC 9562 variant field.
#[php_enum]
#[php(name = "HyperUuid\\UuidVariant")]
pub enum UuidVariant {
    #[php(value = 1)]
    Ncs,
    #[php(value = 2)]
    Rfc9562,
    #[php(value = 3)]
    Microsoft,
    #[php(value = 4)]
    Future,
}

/// A `UuidLayout` default, as reflection and the stubs show it.
impl ext_php_rs::convert::StubLiteral for UuidLayout {
    fn stub_literal(&self) -> String {
        match self {
            UuidLayout::Rfc9562 => "\\HyperUuid\\UuidLayout::Rfc9562",
            UuidLayout::SqlServer => "\\HyperUuid\\UuidLayout::SqlServer",
        }
        .to_owned()
    }
}
