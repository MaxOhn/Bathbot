use itoa::{Buffer as IntBuffer, Integer};
use rand::{Rng, distributions::Alphanumeric};
use ryu::{Buffer as FloatBuffer, Float};

const BOUNDARY_LEN: usize = 8;

pub struct Multipart {
    bytes: Vec<u8>,
    boundary: [u8; BOUNDARY_LEN],
}

impl Multipart {
    pub(super) const BOUNDARY_TERMINATOR: &'static [u8; 2] = b"--";
    pub(super) const NEWLINE: &'static [u8; 2] = b"\r\n";

    pub fn new() -> Self {
        let mut boundary = [0; BOUNDARY_LEN];
        let mut rng = rand::thread_rng();

        boundary
            .iter_mut()
            .for_each(|value| *value = rng.sample(Alphanumeric));

        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(Self::BOUNDARY_TERMINATOR);
        bytes.extend_from_slice(&boundary);

        Self { bytes, boundary }
    }

    pub fn build(mut self) -> Vec<u8> {
        self.bytes.extend_from_slice(Self::BOUNDARY_TERMINATOR);

        self.bytes
    }

    pub fn push_text<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        self.write_field_headers(key.as_ref());
        self.bytes.extend_from_slice(value.as_ref());

        self.bytes.extend_from_slice(Self::NEWLINE);
        self.bytes.extend_from_slice(Self::BOUNDARY_TERMINATOR);
        self.bytes.extend_from_slice(&self.boundary);

        self
    }

    pub fn push_int<K, I>(&mut self, key: K, value: I, buf: &mut IntBuffer) -> &mut Self
    where
        K: AsRef<[u8]>,
        I: Integer,
    {
        self.write_field_headers(key.as_ref());
        self.bytes.extend_from_slice(buf.format(value).as_bytes());

        self.bytes.extend_from_slice(Self::NEWLINE);
        self.bytes.extend_from_slice(Self::BOUNDARY_TERMINATOR);
        self.bytes.extend_from_slice(&self.boundary);

        self
    }

    pub fn push_float<K, F>(&mut self, key: K, value: F, buf: &mut FloatBuffer) -> &mut Self
    where
        K: AsRef<[u8]>,
        F: Float,
    {
        self.write_field_headers(key.as_ref());
        self.bytes.extend_from_slice(buf.format(value).as_bytes());

        self.bytes.extend_from_slice(Self::NEWLINE);
        self.bytes.extend_from_slice(Self::BOUNDARY_TERMINATOR);
        self.bytes.extend_from_slice(&self.boundary);

        self
    }

    pub fn content_type(&self) -> Vec<u8> {
        const NAME: &[u8] = b"multipart/form-data; boundary=";

        let mut content_type = Vec::with_capacity(NAME.len() + self.boundary.len());
        content_type.extend_from_slice(NAME);
        content_type.extend_from_slice(&self.boundary);

        content_type
    }

    pub(super) fn write_field_headers(&mut self, name: &[u8]) {
        self.bytes.extend_from_slice(Self::NEWLINE);
        self.bytes
            .extend_from_slice(b"Content-Disposition: form-data; name=\"");
        self.bytes.extend_from_slice(name);
        self.bytes.extend_from_slice(b"\"");

        self.bytes.extend_from_slice(Self::NEWLINE);
        self.bytes.extend_from_slice(Self::NEWLINE);
    }
}

#[cfg(test)]
mod tests {
    use std::str::from_utf8 as str_from_utf8;

    use super::*;

    #[test]
    fn test_empty() {
        let form = Multipart::new();

        let expect = format!("--{}--", str_from_utf8(&form.boundary).unwrap());

        let form = String::from_utf8(form.build()).unwrap();

        assert_eq!(form, expect);
    }

    #[test]
    fn test_filled() {
        let mut int_buf = IntBuffer::new();
        let mut float_buf = FloatBuffer::new();
        let mut form = Multipart::new();

        form.push_text("key1", "value1")
            .push_int("key2", 123, &mut int_buf)
            .push_float("key3", 456.789, &mut float_buf);

        let boundary = str_from_utf8(&form.boundary).unwrap();

        let expect = format!(
            "--{boundary}\r\n\
            Content-Disposition: form-data; name=\"key1\"\r\n\
            \r\n\
            value1\r\n\
            --{boundary}\r\n\
            Content-Disposition: form-data; name=\"key2\"\r\n\
            \r\n\
            123\r\n\
            --{boundary}\r\n\
            Content-Disposition: form-data; name=\"key3\"\r\n\
            \r\n\
            456.789\r\n\
            --{boundary}--"
        );

        let form = String::from_utf8(form.build()).unwrap();

        assert_eq!(form, expect);
    }
}
