use std::borrow::Cow;

pub trait IntoCacheKey<'a> {
    fn into_key(self) -> Cow<'a, [u8]>;
}

impl<'a> IntoCacheKey<'a> for &'a str {
    #[inline]
    fn into_key(self) -> Cow<'a, [u8]> {
        Cow::Borrowed(self.as_bytes())
    }
}

impl IntoCacheKey<'static> for Vec<u8> {
    #[inline]
    fn into_key(self) -> Cow<'static, [u8]> {
        Cow::Owned(self)
    }
}

impl<'a> IntoCacheKey<'a> for &'a Vec<u8> {
    #[inline]
    fn into_key(self) -> Cow<'a, [u8]> {
        Cow::Borrowed(self)
    }
}

impl IntoCacheKey<'static> for String {
    #[inline]
    fn into_key(self) -> Cow<'static, [u8]> {
        Cow::Owned(self.into_bytes())
    }
}

impl<'a> IntoCacheKey<'a> for &'a String {
    #[inline]
    fn into_key(self) -> Cow<'a, [u8]> {
        Cow::Borrowed(self.as_bytes())
    }
}
