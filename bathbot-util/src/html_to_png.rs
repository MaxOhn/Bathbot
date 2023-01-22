use std::{process::Stdio, time::Duration};

use eyre::{Report, Result, WrapErr};
use tokio::{io::AsyncWriteExt, process::Command, sync::Mutex, time::timeout};

static HTML_TO_PNG: HtmlToPng = HtmlToPng::new();

pub struct HtmlToPng {
    lock: Mutex<()>,
}

impl HtmlToPng {
    const fn new() -> Self {
        Self {
            lock: Mutex::const_new(()),
        }
    }

    pub async fn convert(html: &str) -> Result<Vec<u8>> {
        HTML_TO_PNG.convert_(html).await
    }

    async fn convert_(&self, html: &str) -> Result<Vec<u8>> {
        let _lock = self.lock.lock().await;

        let mut child = Command::new("wkhtmltoimage")
            .arg("--width")
            .arg("980")
            .arg("-f")
            .arg("png")
            .arg("--custom-header")
            .arg("Connection Upgrade")
            .arg("--custom-header-propagation")
            .arg("--enable-local-file-access")
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
                .await
                .wrap_err("failed writing to stdin")?;
        }

        const DEADLINE: Duration = Duration::from_secs(10);

        match timeout(DEADLINE, child.wait_with_output()).await {
            Ok(Ok(output)) => output
                .status
                .success()
                .then_some(output.stdout)
                .ok_or_else(|| {
                    String::from_utf8(output.stderr)
                        .map_or_else(|_| eyre!("stderr did not contain valid UTF-8"), Report::msg)
                }),
            Ok(Err(err)) => Err(Report::new(err).wrap_err("Failed waiting for output")),
            Err(_) => Err(Report::msg("Timed out while waiting for child process")),
        }
    }
}
