use std::{
    io::Write,
    process::{Command, Stdio},
    sync::Mutex,
};

use eyre::{Report, Result, WrapErr};

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

    pub fn convert(html: &str) -> Result<Vec<u8>> {
        HTML_TO_PNG.convert_(html)
    }

    fn convert_(&self, html: &str) -> Result<Vec<u8>> {
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
            .spawn()
            .wrap_err("failed to spawn child")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(html.as_bytes())
                .wrap_err("failed writing to stdin")?;
        }

        let output = child
            .wait_with_output()
            .wrap_err("failed waiting for output")?;

        output
            .status
            .success()
            .then_some(output.stdout)
            .ok_or_else(|| {
                String::from_utf8(output.stderr)
                    .map_or_else(|_| eyre!("stderr did not contain valid UTF-8"), Report::msg)
            })
    }
}
