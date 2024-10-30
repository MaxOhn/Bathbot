pub trait ToCacheKey {
    fn to_key(&self) -> &[u8];
}

impl ToCacheKey for [u8] {
    #[inline]
    fn to_key(&self) -> &[u8] {
        self
    }
}

impl ToCacheKey for Vec<u8> {
    #[inline]
    fn to_key(&self) -> &[u8] {
        <[u8] as ToCacheKey>::to_key(self.as_slice())
    }
}

impl ToCacheKey for str {
    #[inline]
    fn to_key(&self) -> &[u8] {
        <[u8] as ToCacheKey>::to_key(self.as_bytes())
    }
}

impl ToCacheKey for String {
    #[inline]
    fn to_key(&self) -> &[u8] {
        <str as ToCacheKey>::to_key(self.as_str())
    }
}
