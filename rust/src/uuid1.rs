//! Conversions to and from [`uuid::Uuid`](::uuid::Uuid) 1.x, behind the off-by-default `uuid1`
//! feature. Each is a copy of the 16 bytes, so the layout is untouched. The two types
//! do not hash alike, so do not mix them as map keys.

use crate::Uuid;

impl From<::uuid::Uuid> for Uuid {
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn from(u: ::uuid::Uuid) -> Self {
        Uuid::from_bytes(u.into_bytes())
    }
}

impl From<Uuid> for ::uuid::Uuid {
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn from(u: Uuid) -> Self {
        ::uuid::Uuid::from_bytes(u.into_bytes())
    }
}

impl From<&Uuid> for ::uuid::Uuid {
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn from(u: &Uuid) -> Self {
        ::uuid::Uuid::from_bytes(*u.as_bytes())
    }
}

impl PartialEq<::uuid::Uuid> for Uuid {
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn eq(&self, other: &::uuid::Uuid) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<Uuid> for ::uuid::Uuid {
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn eq(&self, other: &Uuid) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
