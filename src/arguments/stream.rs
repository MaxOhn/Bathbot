#[derive(Copy, Clone, Debug, Default)]
pub struct Stream<'a> {
    src: &'a [u8],
    offset: usize,
}

impl<'a> Stream<'a> {
    #[inline]
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            offset: 0,
        }
    }

    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    #[inline]
    pub fn rest(&self) -> &'a str {
        // SAFETY: self.src is constructed from a str in the first place
        // and self.offset is being handled in a safe manner
        unsafe { std::str::from_utf8_unchecked(&self.src[self.offset..]) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.src.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.offset >= self.len()
    }

    #[inline]
    pub fn current_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    #[inline]
    pub fn current(&self) -> Option<u8> {
        self.src.get(self.offset).copied()
    }

    #[inline]
    pub fn next(&mut self) -> Option<u8> {
        let c = self.current()?;
        self.offset += 1;

        Some(c)
    }

    #[inline]
    pub fn peek_while(&self, f: impl Fn(u8) -> bool) -> &'a str {
        if self.is_empty() {
            return "";
        }

        let src = self.src;
        let start = self.offset;
        let mut end = start;

        while let Some(b) = src.get(end) {
            if !f(*b) {
                break;
            }

            end += 1;
        }

        // SAFETY: self.src is constructed from a str in the first place
        // and start & end are being handled in a safe manner
        unsafe { std::str::from_utf8_unchecked(&self.src[start..end]) }
    }

    #[inline]
    pub fn peek_while_char(&self, f: impl Fn(char) -> bool) -> &'a str {
        if self.is_empty() {
            return "";
        }

        let src = self.rest();
        let mut end = 0;

        for c in src.chars() {
            if !f(c) {
                break;
            }

            end += c.len_utf8();
        }

        &src[..end]
    }

    #[inline]
    pub fn peek_until_char(&self, f: impl Fn(char) -> bool) -> &'a str {
        self.peek_while_char(|c| !f(c))
    }

    #[inline]
    pub fn take_while(&mut self, f: impl Fn(u8) -> bool) -> &'a str {
        let s = self.peek_while(f);
        self.offset += s.len();

        s
    }

    #[inline]
    pub fn take_while_char(&mut self, f: impl Fn(char) -> bool) -> &'a str {
        let s = self.peek_while_char(f);
        self.offset += s.len();

        s
    }

    #[inline]
    pub fn take_until(&mut self, f: impl Fn(u8) -> bool) -> &'a str {
        self.take_while(|c| !f(c))
    }

    #[inline]
    pub fn take_until_char(&mut self, f: impl Fn(char) -> bool) -> &'a str {
        self.take_while_char(|c| !f(c))
    }

    #[inline]
    pub fn starts_with(&self, prefix: &str) -> bool {
        let bytes = prefix.as_bytes();

        bytes.len() <= self.len() - self.offset
            && bytes == &self.src[self.offset..self.offset + bytes.len()]
    }

    #[inline]
    pub fn increment(&mut self, amount: usize) {
        self.offset += amount;
    }
}
