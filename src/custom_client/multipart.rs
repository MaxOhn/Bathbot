use std::{fmt::Display, io::Write};

use rand::{distributions::Alphanumeric, Rng};

const BOUNDARY_LEN: usize = 8;

pub struct Multipart {
    bytes: Vec<u8>,
    boundary: Box<str>,
}

impl Multipart {
    pub fn new() -> Self {
        let boundary: String = rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(BOUNDARY_LEN)
            .map(|c| c as char)
            .collect();

        Self {
            bytes: Vec::with_capacity(128),
            boundary: boundary.into_boxed_str(),
        }
    }

    pub fn push_text<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Display,
        V: Display,
    {
        self.write_field_headers(key);
        let _ = write!(self.bytes, "{value}");

        self
    }

    pub fn finish(mut self) -> Vec<u8> {
        if !self.is_empty() {
            self.bytes.extend_from_slice(b"\r\n");
        }

        let _ = write!(self.bytes, "--{}--\r\n", self.boundary);

        self.bytes
    }

    pub fn boundary(&self) -> &str {
        &self.boundary
    }

    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn write_field_headers(&mut self, name: impl Display) {
        if !self.is_empty() {
            self.bytes.extend_from_slice(b"\r\n");
        }

        let _ = write!(self.bytes, "--{}\r\n", self.boundary);

        let _ = write!(
            self.bytes,
            "Content-Disposition: form-data; name=\"{name}\""
        );

        self.bytes.extend_from_slice(b"\r\n\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::Multipart;

    #[test]
    fn empty() {
        let form = Multipart::new();

        let expect = format!("--{}--\r\n", form.boundary());

        let form = String::from_utf8(form.finish()).unwrap();

        assert_eq!(form, expect);
    }

    #[test]
    fn texts() {
        let form = Multipart::new()
            .push_text("key1", "value1")
            .push_text("key2", "value2");

        let boundary = form.boundary();

        let expect = format!(
            "--{boundary}\r\n\
            Content-Disposition: form-data; name=\"key1\"\r\n\r\n\
            value1\r\n\
            --{boundary}\r\n\
            Content-Disposition: form-data; name=\"key2\"\r\n\r\n\
            value2\r\n--{boundary}--\r\n"
        );

        let form = String::from_utf8(form.finish()).unwrap();

        assert_eq!(form, expect);
    }
}
