use std::{
    io::{Error as IoError, Write},
    process::{Command, Stdio},
    sync::Mutex,
};

static HTML_TO_PNG: HtmlToPng = HtmlToPng::new();

pub struct HtmlToPng {
    lock: Mutex<()>,
}

impl HtmlToPng {
    const fn new() -> Self {
        Self {
            lock: Mutex::new(()),
        }
    }

    pub fn convert(html: &str) -> Result<Vec<u8>, HtmlToPngError> {
        HTML_TO_PNG.convert_(html)
    }

    fn convert_(&self, html: &str) -> Result<Vec<u8>, HtmlToPngError> {
        let _lock = self.lock.lock().unwrap();

        let mut child = Command::new("wkhtmltoimage")
            .arg("--width")
            .arg("980")
            .arg("--transparent")
            .arg("-")
            .arg("-")
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(html.as_bytes())?;
        }

        let output = child.wait_with_output()?;

        output
            .status
            .success()
            .then_some(output.stdout)
            .ok_or_else(|| {
                String::from_utf8(output.stderr)
                    .map_or(HtmlToPngError::Utf8, HtmlToPngError::StdErr)
            })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HtmlToPngError {
    #[error("io error")]
    Io(#[from] IoError),
    #[error("stderr:\n{0}")]
    StdErr(String),
    #[error("stderr did not contain valid UTF-8")]
    Utf8,
}
